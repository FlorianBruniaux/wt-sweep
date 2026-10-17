//! Current, bounded HEAD/index/disk observation. No snapshot or cleanup decision.
use super::*;
use crate::model::{LocalCount, LocalSource, LocalSummary};
use std::ffi::OsStr;
fn text(bytes: &[u8]) -> io::Result<&str> {
    std::str::from_utf8(bytes).map_err(|e| io::Error::other(e.to_string()))
}
#[derive(Clone, Debug)]
pub struct SummaryLimits {
    pub max_index_entries: usize,
    pub max_index_bytes: u64,
    pub max_paths: usize,
    pub max_path_bytes: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_objects: usize,
    pub max_depth: usize,
    pub max_summary_bytes: usize,
    pub timeout: Duration,
}
impl Default for SummaryLimits {
    fn default() -> Self {
        Self {
            max_index_entries: 100_000,
            max_index_bytes: 32 * 1024 * 1024,
            max_paths: 100_000,
            max_path_bytes: 32 * 1024 * 1024,
            max_total_bytes: 128 * 1024 * 1024,
            max_file_bytes: 32 * 1024 * 1024,
            max_objects: 4096,
            max_depth: 256,
            max_summary_bytes: 64 * 1024,
            timeout: Duration::from_secs(30),
        }
    }
}
impl SummaryLimits {
    /// Explicit larger collection allowance; standard review retains its defaults.
    pub fn thorough() -> Self {
        Self {
            max_objects: 65_536,
            max_total_bytes: 512 * 1024 * 1024,
            timeout: Duration::from_secs(120),
            ..Self::default()
        }
    }
}
#[derive(Debug, Default)]
pub(crate) struct SummaryMetrics {
    /// Decoded payloads offered to object verification, including refused payloads.
    pub object_reads: u64,
    /// Raw payload bytes; excludes batch framing and metadata responses.
    pub object_bytes: u64,
    /// Children actually spawned, including failed reads.
    pub git_processes: u64,
    /// Same-run blob results reused by HEAD, index or disk comparisons.
    pub cache_hits: u64,
    /// Main disk metadata attempts, excluding administration markers.
    pub paths_checked: u64,
}
pub(crate) struct SummaryOutcome {
    pub summary: LocalSummary,
    pub first_git_timeout: Option<GitRead>,
    pub metrics: SummaryMetrics,
    pub comparison: Option<comparison::SourceCapture>,
}
pub(crate) fn collect_summary_for_review(
    w: &Worktree,
    l: &SummaryLimits,
) -> Result<crate::local::SummaryOutcome, SweepError> {
    collect_outcome(w, l, &NativeHooks)
}
pub fn collect_summary(w: &Worktree, l: &SummaryLimits) -> Result<LocalSummary, SweepError> {
    collect_summary_with(w, l, &NativeHooks)
}
fn observe(count: &mut LocalCount, path: &BytePath) {
    count.observed += 1;
    let used = count.samples.iter().map(|p| p.hex.len() / 2).sum::<usize>();
    if count.samples.len() < 8 && used.saturating_add(path.hex.len() / 2) <= 1024 {
        count.samples.push(path.clone());
    } else {
        count.samples_omitted += 1;
    }
}
type TreePosition = (String, Vec<u8>, usize);
struct Collector<'a> {
    s: LocalSummary,
    metrics: SummaryMetrics,
    limits: &'a SummaryLimits,
    hooks: &'a dyn LocalHooks,
    started: Instant,
    bytes: u64,
    objects: usize,
    paths: usize,
    path_bytes: usize,
    stopped: bool,
    git_failed: bool,
    first_git_timeout: Option<GitRead>,
    diagnostic_bytes: usize,
    blobs: BTreeMap<String, Result<String, String>>,
    comparison: Option<comparison::SourceCapture>,
}
impl Collector<'_> {
    fn capture(
        &mut self,
        path: &BytePath,
        category: comparison::Category,
        layer: comparison::Layer,
        stage: Option<u8>,
        source: comparison::Value,
        source_oid: Option<String>,
    ) {
        if let Some(capture) = &mut self.comparison {
            capture.push(comparison::SourceRow {
                path: path.clone(),
                category,
                layer,
                stage,
                source,
                source_oid,
            });
        }
    }
    fn gap(&mut self, path: &Path, reason: impl Into<String>) {
        let reason = reason.into();
        let bp = BytePath::from_path(path);
        if self.s.gaps.len() < 32
            && reason.len() <= 1024
            && self
                .diagnostic_bytes
                .saturating_add(bp.hex.len() + reason.len())
                <= 8192
        {
            self.diagnostic_bytes += bp.hex.len() + reason.len();
            self.s.gaps.push(Gap { path: bp, reason });
        } else {
            self.s.details_truncated = true;
        }
    }
    fn check(&mut self) -> io::Result<()> {
        if self.stopped {
            return Err(io::Error::other(
                "summary collection already stopped; see earlier gaps",
            ));
        }
        if self.hooks.cancelled() || self.started.elapsed() >= self.limits.timeout {
            self.stopped = true;
            return Err(io::Error::other("summary cancelled or deadline reached"));
        }
        Ok(())
    }
    fn charge(&mut self, n: usize) -> io::Result<()> {
        self.bytes = self.bytes.saturating_add(n as u64);
        if self.bytes > self.limits.max_total_bytes {
            self.stopped = true;
            Err(io::Error::other("summary total byte limit reached"))
        } else {
            Ok(())
        }
    }
    fn reserve_path(&mut self, n: usize) -> io::Result<()> {
        self.path_bytes = self.path_bytes.saturating_add(n);
        if self.path_bytes > self.limits.max_path_bytes {
            Err(io::Error::other("summary path byte limit reached"))
        } else {
            Ok(())
        }
    }
    fn git(&mut self, fd: &OwnedFd, path: &Path, op: GitRead) -> io::Result<Vec<u8>> {
        self.check()?;
        if self.git_failed {
            return Err(io::Error::other(
                "earlier Git read failed; dependent reads refused",
            ));
        }
        let remaining = self.limits.timeout.saturating_sub(self.started.elapsed());
        let backend_cap = ControlledGit::default().timeout_limit();
        let git = ControlledGit::with_timeout(remaining);
        let facet_limited = git.timeout_limit() < backend_cap;
        let result = git.read_from_directory(fd, path, op.clone());
        self.metrics.git_processes = self
            .metrics
            .git_processes
            .saturating_add(git.spawned_processes());
        let bytes = match result {
            Ok(bytes) => bytes,
            Err(e) => {
                if e.code == "git_timeout"
                    && facet_limited
                    && self.started.elapsed() >= self.limits.timeout
                {
                    // A shortened facet allowance is not evidence of a blocked
                    // backend. Stop this collector without quarantining its family.
                    self.stopped = true;
                    return Err(io::Error::other(
                        "summary time budget exhausted during Git read; dependent facet reads refused",
                    ));
                }
                self.git_failed = e.code == "git_timeout";
                if self.git_failed && self.first_git_timeout.is_none() {
                    self.first_git_timeout = Some(op);
                }
                return Err(io::Error::other(e.to_string()));
            }
        };
        self.charge(bytes.len())?;
        self.check()?;
        Ok(bytes)
    }
    fn object(&mut self, fd: &OwnedFd, path: &Path, kind: &str, oid: &str) -> io::Result<Vec<u8>> {
        self.check()?;
        if self.objects >= self.limits.max_objects {
            return Err(io::Error::other("summary object limit reached"));
        }
        self.objects += 1;
        let op = match kind {
            "commit" => GitRead::Commit { oid: oid.into() },
            "tree" => GitRead::Tree { oid: oid.into() },
            _ => GitRead::Blob { oid: oid.into() },
        };
        let bytes = self.git(fd, path, op)?;
        self.note_object(&bytes);
        if bytes.len() as u64 > self.limits.max_file_bytes {
            return Err(io::Error::other("summary object byte limit reached"));
        }
        crate::review::verify_object(kind, oid, &bytes)
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(bytes)
    }
    fn note_object(&mut self, raw: &[u8]) {
        self.metrics.object_reads = self.metrics.object_reads.saturating_add(1);
        self.metrics.object_bytes = self.metrics.object_bytes.saturating_add(raw.len() as u64);
    }
    fn blob_digest(&mut self, oid: &str, kind: &str, raw: &[u8]) -> Result<String, String> {
        self.note_object(raw);
        if kind != "blob" || raw.len() as u64 > self.limits.max_file_bytes {
            return Err("expected blob within summary object byte limit".into());
        }
        crate::review::verify_object("blob", oid, raw).map_err(|e| e.to_string())?;
        Ok(format!("{:x}", Sha256::digest(raw)))
    }
    fn blob_batch(&mut self, fd: &OwnedFd, path: &Path, oids: &[String]) {
        if oids.is_empty() {
            return;
        }
        let result = self
            .git(
                fd,
                path,
                GitRead::Objects {
                    oids: oids.to_vec(),
                },
            )
            .and_then(|raw| {
                crate::git::parse_objects(&raw, oids).map_err(|e| io::Error::other(e.to_string()))
            });
        match result {
            Ok(objects) => {
                for (oid, object) in oids.iter().zip(objects) {
                    let digest = object
                        .and_then(|object| self.blob_digest(oid, &object.kind, &object.bytes));
                    self.blobs.insert(oid.clone(), digest);
                }
            }
            Err(error) => {
                for oid in oids {
                    self.blobs.insert(oid.clone(), Err(error.to_string()));
                }
            }
        }
    }
    fn prefetch_blobs<'a>(
        &mut self,
        fd: &OwnedFd,
        path: &Path,
        oids: impl Iterator<Item = &'a str>,
    ) {
        // The cache and selection are bounded by the existing object budget.
        // Reserve each distinct OID once, including unavailable objects.
        let remaining = self.limits.max_objects.saturating_sub(self.objects);
        let mut selected = BTreeSet::new();
        for oid in oids {
            if let Err(error) = self.check() {
                self.gap(path, error.to_string());
                break;
            }
            if selected.len() < remaining && !self.blobs.contains_key(oid) {
                selected.insert(oid.to_owned());
            }
        }
        self.objects += selected.len();
        let selected: Vec<_> = selected.into_iter().collect();
        for oid in &selected {
            self.blobs
                .insert(oid.clone(), Err("blob batch not completed".into()));
        }
        const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
        for oids in selected.chunks(128) {
            let metadata = self
                .git(
                    fd,
                    path,
                    GitRead::ObjectInfo {
                        oids: oids.to_vec(),
                    },
                )
                .and_then(|raw| {
                    crate::git::parse_object_info(&raw, oids)
                        .map_err(|e| io::Error::other(e.to_string()))
                });
            let metadata = match metadata {
                Ok(metadata) => metadata,
                Err(error) => {
                    for oid in oids {
                        self.blobs.insert(oid.clone(), Err(error.to_string()));
                    }
                    if self.git_failed || self.stopped {
                        break;
                    }
                    continue;
                }
            };
            let mut payload = Vec::new();
            let mut payload_bytes = 0usize;
            for (oid, info) in oids.iter().zip(metadata) {
                let info = match info {
                    Ok(info)
                        if info.kind == "blob"
                            && info.size as u64 <= self.limits.max_file_bytes =>
                    {
                        info
                    }
                    Ok(_) => {
                        self.blobs.insert(
                            oid.clone(),
                            Err("expected blob within summary object byte limit".into()),
                        );
                        continue;
                    }
                    Err(error) => {
                        self.blobs.insert(oid.clone(), Err(error));
                        continue;
                    }
                };
                // Exact batch header, payload and terminator. A scalar read
                // retains objects fitting the old output cap without framing.
                let framed = info
                    .size
                    .saturating_add(oid.len() + " blob ".len() + info.size.to_string().len() + 2);
                if framed > OUTPUT_LIMIT {
                    let digest = if info.size <= OUTPUT_LIMIT {
                        self.git(fd, path, GitRead::Blob { oid: oid.clone() })
                            .map_err(|e| e.to_string())
                            .and_then(|raw| self.blob_digest(oid, "blob", &raw))
                    } else {
                        Err("summary object exceeds Git output byte limit".into())
                    };
                    self.blobs.insert(oid.clone(), digest);
                    continue;
                }
                if payload_bytes.saturating_add(framed) > OUTPUT_LIMIT {
                    self.blob_batch(fd, path, &payload);
                    payload.clear();
                    payload_bytes = 0;
                }
                payload.push(oid.clone());
                payload_bytes += framed;
            }
            self.blob_batch(fd, path, &payload);
            if self.git_failed || self.stopped {
                break;
            }
        }
    }
    fn blob(&mut self, fd: &OwnedFd, path: &Path, oid: &str) -> io::Result<String> {
        if let Some(result) = self.blobs.get(oid) {
            self.metrics.cache_hits = self.metrics.cache_hits.saturating_add(1);
            return result.clone().map_err(io::Error::other);
        }
        let value = self
            .object(fd, path, "blob", oid)
            .map(|raw| format!("{:x}", Sha256::digest(&raw)))
            .map_err(|e| e.to_string());
        if self.blobs.len() < self.limits.max_objects {
            self.blobs.insert(oid.into(), value.clone());
        }
        value.map_err(io::Error::other)
    }
    fn read(
        &mut self,
        parent: &OwnedFd,
        name: &OsStr,
        path: &Path,
        max: u64,
        retain: bool,
    ) -> io::Result<(String, Vec<u8>)> {
        self.check()?;
        let before = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?;
        let bp = BytePath::from_path(path);
        self.hooks.before_entry(&bp)?;
        if self.hooks.is_dataless(&bp, &before) {
            return Err(io::Error::other("dataless entry; content refused"));
        }
        if before.st_mode & libc::S_IFMT != libc::S_IFREG
            || before.st_size < 0
            || before.st_size as u64 > max
        {
            return Err(io::Error::other("expected regular file within byte limit"));
        }
        let fd = openat(
            parent,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        if !same_stat(&before, &fstat(&fd)?) {
            self.s.temporal_consistency = "change_observed".into();
            return Err(io::Error::other("file changed before open"));
        }
        self.hooks.after_file_open(&bp)?;
        let mut file = File::from(fd);
        let mut hash = Sha256::new();
        let mut raw = Vec::new();
        let mut size = 0u64;
        let mut buffer = [0u8; 16384];
        loop {
            self.check()?;
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            self.charge(n)?;
            size += n as u64;
            if size > max {
                return Err(io::Error::other("file grew past byte limit"));
            }
            hash.update(&buffer[..n]);
            if retain {
                raw.extend_from_slice(&buffer[..n]);
            }
        }
        if !same_stat(&before, &fstat(&file)?)
            || !same_stat(&before, &statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)?)
        {
            self.s.temporal_consistency = "change_observed".into();
            return Err(io::Error::other("file changed during read"));
        }
        Ok((format!("{:x}", hash.finalize()), raw))
    }
    fn source(
        &mut self,
        fd: &OwnedFd,
        admin: &Path,
        name: &str,
        max: u64,
    ) -> io::Result<(String, Vec<u8>)> {
        let path = admin.join(name);
        let value = self.read(fd, OsStr::new(name), &path, max, true)?;
        self.s.sources.push(LocalSource {
            kind: name.to_ascii_lowercase(),
            path: BytePath::from_path(&path),
            sha256: value.0.clone(),
        });
        Ok(value)
    }
    fn tree_entries(
        &mut self,
        raw: &[u8],
        position: &TreePosition,
        hash_size: usize,
        records: &mut usize,
        result: &mut BTreeMap<BytePath, IndexEntry>,
        stack: &mut Vec<TreePosition>,
    ) -> io::Result<()> {
        let (_, prefix, depth) = position;
        let mut offset = 0;
        while offset < raw.len() {
            self.check()?;
            *records += 1;
            if *records > self.limits.max_paths {
                return Err(io::Error::other("HEAD tree path limit reached"));
            }
            let space = raw[offset..]
                .iter()
                .position(|b| *b == b' ')
                .ok_or_else(|| io::Error::other("invalid tree mode"))?
                + offset;
            let mode = u32::from_str_radix(text(&raw[offset..space])?, 8)
                .map_err(|_| io::Error::other("invalid tree mode"))?;
            let start = space + 1;
            let end = raw[start..]
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| io::Error::other("invalid tree path"))?
                + start;
            let name = &raw[start..end];
            if !valid_relative(name) || name.contains(&b'/') || name == b".git" {
                return Err(io::Error::other("unsafe HEAD tree path"));
            }
            let bytes = raw
                .get(end + 1..end + 1 + hash_size)
                .ok_or_else(|| io::Error::other("short tree OID"))?;
            let oid = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            offset = end + 1 + hash_size;
            self.reserve_path(prefix.len().saturating_add(name.len()))?;
            let mut full = prefix.to_vec();
            full.extend_from_slice(name);
            let bp = BytePath::from_bytes(&full);
            if mode == 0o40000 {
                full.push(b'/');
                stack.push((oid, full, depth + 1));
            } else if matches!(mode, 0o100644 | 0o100755 | 0o120000) {
                if result
                    .insert(
                        bp.clone(),
                        IndexEntry {
                            path: bp,
                            mode,
                            oid,
                            stage: 0,
                        },
                    )
                    .is_some()
                {
                    return Err(io::Error::other("duplicate tree path"));
                }
            } else {
                return Err(io::Error::other("gitlink or unsupported HEAD tree type"));
            }
        }
        Ok(())
    }
    fn tree_payload(
        &mut self,
        fd: &OwnedFd,
        path: &Path,
        positions: &[TreePosition],
    ) -> io::Result<Vec<(String, Vec<u8>)>> {
        let oids: Vec<_> = positions
            .iter()
            .map(|position| position.0.clone())
            .collect();
        let raw = self.git(fd, path, GitRead::Objects { oids: oids.clone() })?;
        crate::git::parse_objects(&raw, &oids)
            .map_err(|e| io::Error::other(e.to_string()))?
            .into_iter()
            .map(|object| {
                object
                    .map(|object| {
                        self.note_object(&object.bytes);
                        (object.kind, object.bytes)
                    })
                    .map_err(io::Error::other)
            })
            .collect()
    }
    fn trees(
        &mut self,
        fd: &OwnedFd,
        path: &Path,
        tree: &str,
        hash_size: usize,
    ) -> io::Result<BTreeMap<BytePath, IndexEntry>> {
        let mut result = BTreeMap::new();
        let mut stack = vec![(tree.to_owned(), Vec::new(), 0usize)];
        let mut records = 0usize;
        const OUTPUT_LIMIT: usize = 8 * 1024 * 1024;
        while !stack.is_empty() {
            self.check()?;
            let count = stack
                .len()
                .min(128)
                .min(self.limits.max_objects.saturating_sub(self.objects));
            if count == 0 {
                return Err(io::Error::other("summary object limit reached"));
            }
            let mut frontier = Vec::with_capacity(count);
            for _ in 0..count {
                let position = stack
                    .pop()
                    .ok_or_else(|| io::Error::other("missing tree frontier"))?;
                if position.2 > self.limits.max_depth {
                    return Err(io::Error::other("HEAD tree depth limit reached"));
                }
                frontier.push(position);
            }
            self.objects += count;
            let oids: Vec<_> = frontier.iter().map(|position| position.0.clone()).collect();
            let metadata = self.git(fd, path, GitRead::ObjectInfo { oids: oids.clone() })?;
            let metadata = crate::git::parse_object_info(&metadata, &oids)
                .map_err(|e| io::Error::other(e.to_string()))?;
            let mut start = 0usize;
            while start < frontier.len() {
                let mut end = start;
                let mut bytes = 0usize;
                while end < frontier.len() {
                    let info = metadata[end]
                        .as_ref()
                        .map_err(|e| io::Error::other(e.clone()))?;
                    if info.kind != "tree" || info.size as u64 > self.limits.max_file_bytes {
                        return Err(io::Error::other(
                            "expected tree within summary object byte limit",
                        ));
                    }
                    let framed = info.size.saturating_add(
                        frontier[end].0.len() + " tree ".len() + info.size.to_string().len() + 2,
                    );
                    if bytes.saturating_add(framed) > OUTPUT_LIMIT {
                        break;
                    }
                    bytes += framed;
                    end += 1;
                }
                if end == start {
                    let info = metadata[start]
                        .as_ref()
                        .map_err(|e| io::Error::other(e.clone()))?;
                    if info.size > OUTPUT_LIMIT {
                        return Err(io::Error::other(
                            "summary object exceeds Git output byte limit",
                        ));
                    }
                    let raw = self.git(
                        fd,
                        path,
                        GitRead::Tree {
                            oid: frontier[start].0.clone(),
                        },
                    )?;
                    self.note_object(&raw);
                    crate::review::verify_object("tree", &frontier[start].0, &raw)
                        .map_err(|e| io::Error::other(e.to_string()))?;
                    self.tree_entries(
                        &raw,
                        &frontier[start],
                        hash_size,
                        &mut records,
                        &mut result,
                        &mut stack,
                    )?;
                    end += 1;
                } else {
                    for (position, object) in frontier[start..end].iter().zip(self.tree_payload(
                        fd,
                        path,
                        &frontier[start..end],
                    )?) {
                        if object.0 != "tree" || object.1.len() as u64 > self.limits.max_file_bytes
                        {
                            return Err(io::Error::other(
                                "expected tree within summary object byte limit",
                            ));
                        }
                        crate::review::verify_object("tree", &position.0, &object.1)
                            .map_err(|e| io::Error::other(e.to_string()))?;
                        self.tree_entries(
                            &object.1,
                            position,
                            hash_size,
                            &mut records,
                            &mut result,
                            &mut stack,
                        )?;
                    }
                }
                start = end;
            }
        }
        Ok(result)
    }
    fn disk(
        &mut self,
        root: &OwnedFd,
        path: &Path,
        index: &BTreeMap<BytePath, IndexEntry>,
        conflicts: &BTreeSet<BytePath>,
        intent_to_add: &BTreeSet<BytePath>,
        known: bool,
    ) {
        self.s.disk_complete = true;
        self.s.unstaged.complete = known;
        self.s.untracked.complete = known;
        self.s.ignored.complete = known;
        let initial = match openat(root, ".", DIRECTORY, Mode::empty()) {
            Ok(v) => v,
            Err(e) => {
                self.disk_gap(Path::new("."), e.to_string());
                return;
            }
        };
        let mut stack = vec![(initial, PathBuf::new(), 0usize)];
        let mut seen = BTreeSet::new();
        while let Some((fd, relative, depth)) = stack.pop() {
            if let Err(e) = self.check() {
                self.disk_gap(&relative, e.to_string());
                break;
            }
            let before = match fstat(&fd) {
                Ok(v) => v,
                Err(e) => {
                    self.disk_gap(&relative, e.to_string());
                    continue;
                }
            };
            let mut dir = match Dir::read_from(&fd) {
                Ok(v) => v,
                Err(e) => {
                    self.disk_gap(&relative, e.to_string());
                    continue;
                }
            };
            let mut names = Vec::new();
            for entry in &mut dir {
                if let Err(e) = self.check() {
                    self.disk_gap(&relative, e.to_string());
                    break;
                }
                match entry {
                    Ok(entry) => {
                        let n = entry.file_name().to_bytes();
                        if n == b"." || n == b".." {
                            continue;
                        }
                        if self.paths >= self.limits.max_paths {
                            self.disk_gap(&relative, "disk path limit reached");
                            self.stopped = true;
                            break;
                        }
                        if let Err(e) = self.reserve_path(
                            relative
                                .as_os_str()
                                .as_encoded_bytes()
                                .len()
                                .saturating_add(n.len() + 1),
                        ) {
                            self.disk_gap(&relative, e.to_string());
                            break;
                        }
                        self.paths += 1;
                        names.push(n.to_vec());
                    }
                    Err(e) => self.disk_gap(&relative, e.to_string()),
                }
            }
            names.sort();
            let mut candidates = Vec::new();
            for n in &names {
                let p = relative.join(OsString::from_vec(n.clone()));
                let bp = BytePath::from_path(&p);
                if n != b".git" && !index.contains_key(&bp) && !conflicts.contains(&bp) {
                    match statat(
                        &fd,
                        OsStr::new(&OsString::from_vec(n.clone())),
                        AtFlags::SYMLINK_NOFOLLOW,
                    ) {
                        Ok(st) if st.st_mode & libc::S_IFMT != libc::S_IFDIR => candidates.push(bp),
                        Ok(_) => {}
                        Err(e) => self.disk_gap(&p, e.to_string()),
                    }
                }
            }
            let mut additional_categories = BTreeMap::new();
            for batch in candidates.chunks(256) {
                match self
                    .git(
                        root,
                        path,
                        GitRead::IgnorePaths {
                            paths: batch.to_vec(),
                        },
                    )
                    .and_then(|raw| parse_paths(&raw))
                {
                    Ok(ignored) => {
                        // Candidates already have safe positive metadata observations.
                        // Publish each successful batch before a later batch can stop.
                        if known {
                            for bp in batch {
                                if ignored.contains(bp) {
                                    observe(&mut self.s.ignored, bp);
                                    if self.comparison.is_some() {
                                        additional_categories
                                            .insert(bp.clone(), comparison::Category::Ignored);
                                    }
                                } else {
                                    observe(&mut self.s.untracked, bp);
                                    if self.comparison.is_some() {
                                        additional_categories
                                            .insert(bp.clone(), comparison::Category::Untracked);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        self.disk_gap(&relative, format!("ignore classification unavailable: {e}"));
                        break;
                    }
                }
            }
            for n in names {
                let name = OsString::from_vec(n);
                let p = relative.join(&name);
                let bp = BytePath::from_path(&p);
                if name == ".git" {
                    if !relative.as_os_str().is_empty() {
                        self.disk_gap(&p, "nested repository administration marker");
                    }
                    continue;
                }
                if let Err(e) = self.check() {
                    self.disk_gap(&p, e.to_string());
                    break;
                }
                self.metrics.paths_checked = self.metrics.paths_checked.saturating_add(1);
                let st = match statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(v) => v,
                    Err(e) => {
                        self.disk_gap(&p, e.to_string());
                        continue;
                    }
                };
                let kind = st.st_mode & libc::S_IFMT;
                let tracked = index.get(&bp);
                if tracked.is_some() || conflicts.contains(&bp) {
                    seen.insert(bp.clone());
                }
                if let Err(e) = self.hooks.before_entry(&bp) {
                    self.disk_gap(&p, format!("entry refused: {e}"));
                    continue;
                }
                if self.hooks.is_dataless(&bp, &st) {
                    self.disk_gap(&p, "dataless entry; content refused");
                    continue;
                }
                if kind == libc::S_IFDIR {
                    if tracked.is_some() {
                        observe(&mut self.s.unstaged, &bp);
                        self.capture(
                            &bp,
                            comparison::Category::Unstaged,
                            comparison::Layer::Disk,
                            None,
                            comparison::Value::unknown("directory replaced indexed file"),
                            None,
                        );
                    }
                    if depth >= self.limits.max_depth {
                        self.disk_gap(&p, "disk depth limit reached");
                        continue;
                    }
                    match openat(&fd, &name, DIRECTORY, Mode::empty()) {
                        Ok(child) => match fstat(&child) {
                            Ok(after) if same_stat(&st, &after) => {
                                stack.push((child, p, depth + 1))
                            }
                            Ok(_) => self.disk_change(&p, "directory changed before open"),
                            Err(e) => {
                                self.disk_gap(&p, format!("directory metadata unavailable: {e}"))
                            }
                        },
                        Err(e) => self.disk_gap(&p, e.to_string()),
                    }
                    continue;
                }
                let (mode, digest) = if kind == libc::S_IFREG {
                    let mode = if st.st_mode & 0o100 != 0 {
                        0o100755
                    } else {
                        0o100644
                    };
                    match self.read(&fd, &name, &p, self.limits.max_file_bytes, false) {
                        Ok((d, _)) => (mode, Some(d)),
                        Err(e) => {
                            self.disk_gap(&p, e.to_string());
                            (mode, None)
                        }
                    }
                } else if kind == libc::S_IFLNK {
                    let mut buffer = [0u8; 4096];
                    let digest = match rustix::fs::readlinkat_raw(&fd, &name, &mut buffer[..]) {
                        Ok(n) if n < buffer.len() => {
                            match self.charge(n).and_then(|_| {
                                statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW).map_err(Into::into)
                            }) {
                                Ok(after) if same_stat(&st, &after) => {
                                    Some(format!("{:x}", Sha256::digest(&buffer[..n])))
                                }
                                Ok(_) => {
                                    self.disk_change(&p, "symlink changed during observation");
                                    None
                                }
                                Err(e) => {
                                    self.disk_gap(
                                        &p,
                                        format!("symlink revalidation unavailable: {e}"),
                                    );
                                    None
                                }
                            }
                        }
                        _ => {
                            self.disk_gap(&p, "symlink target unavailable or exceeds limit");
                            None
                        }
                    };
                    (0o120000, digest)
                } else {
                    self.disk_gap(&p, "unsupported special file");
                    (0, None)
                };
                let stable = statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW)
                    .is_ok_and(|after| same_stat(&st, &after));
                match statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(after) if same_stat(&st, &after) => {}
                    Ok(_) => self
                        .disk_change(&p, "entry changed between metadata and content observation"),
                    Err(e) => self.disk_gap(&p, format!("entry revalidation unavailable: {e}")),
                }
                if let Some(entry) = tracked {
                    let cached = self.blobs.get(&entry.oid);
                    if cached.is_some() {
                        self.metrics.cache_hits = self.metrics.cache_hits.saturating_add(1);
                    }
                    let expected = cached.and_then(|r| r.as_ref().ok()).cloned();
                    if intent_to_add.contains(&bp)
                        || mode != entry.mode
                        || digest
                            .as_ref()
                            .zip(expected.as_ref())
                            .is_some_and(|(a, b)| a != b)
                    {
                        observe(&mut self.s.unstaged, &bp);
                        let value = if stable && !intent_to_add.contains(&bp) {
                            comparison::Value::disk(digest.clone(), mode, &st)
                        } else {
                            comparison::Value::unknown("unstable or intent-to-add source")
                        };
                        self.capture(
                            &bp,
                            comparison::Category::Unstaged,
                            comparison::Layer::Disk,
                            None,
                            value,
                            None,
                        );
                    }
                    if digest.is_none() || (expected.is_none() && !intent_to_add.contains(&bp)) {
                        self.s.unstaged.complete = false;
                    }
                } else if let Some(category) = additional_categories.get(&bp) {
                    let value = if stable {
                        comparison::Value::disk(digest, mode, &st)
                    } else {
                        comparison::Value::unknown("source changed during read")
                    };
                    self.capture(
                        &bp,
                        category.clone(),
                        comparison::Layer::Disk,
                        None,
                        value,
                        None,
                    );
                }
            }
            match fstat(&fd) {
                Ok(after) if same_stat(&before, &after) => {}
                Ok(_) => self.disk_change(&relative, "directory changed during enumeration"),
                Err(e) => self.disk_gap(
                    &relative,
                    format!("directory revalidation unavailable: {e}"),
                ),
            }
        }
        if self.s.disk_complete {
            for bp in index.keys() {
                if !seen.contains(bp) {
                    observe(&mut self.s.unstaged, bp);
                    self.capture(
                        bp,
                        comparison::Category::Unstaged,
                        comparison::Layer::Disk,
                        None,
                        comparison::Value::Absent { parent: None },
                        None,
                    );
                }
            }
        }
    }
    fn disk_gap(&mut self, p: &Path, reason: impl Into<String>) {
        self.s.disk_complete = false;
        self.s.unstaged.complete = false;
        self.s.untracked.complete = false;
        self.s.ignored.complete = false;
        self.gap(p, reason);
    }
    fn disk_change(&mut self, p: &Path, reason: &str) {
        self.s.temporal_consistency = "change_observed".into();
        self.disk_gap(p, reason);
    }
    fn drift(&mut self, p: &Path, reason: &str) {
        self.s.temporal_consistency = "change_observed".into();
        self.invalidate(p, reason);
    }
    fn invalidate(&mut self, p: &Path, reason: &str) {
        self.s.head_complete = false;
        self.s.index_complete = false;
        self.s.disk_complete = false;
        self.s.staged.complete = false;
        self.s.unstaged.complete = false;
        self.s.conflicts.complete = false;
        self.s.untracked.complete = false;
        self.s.ignored.complete = false;
        self.gap(p, reason);
    }
}
pub fn collect_summary_with(
    w: &Worktree,
    l: &SummaryLimits,
    h: &dyn LocalHooks,
) -> Result<LocalSummary, SweepError> {
    collect_outcome(w, l, h).map(|outcome| outcome.summary)
}
fn collect_outcome(
    w: &Worktree,
    l: &SummaryLimits,
    h: &dyn LocalHooks,
) -> Result<SummaryOutcome, SweepError> {
    collect_outcome_with_projection(w, l, h, None)
}
fn collect_outcome_with_projection(
    w: &Worktree,
    l: &SummaryLimits,
    h: &dyn LocalHooks,
    projection: Option<(usize, usize)>,
) -> Result<SummaryOutcome, SweepError> {
    crate::residency::ensure_no_materialization()?;
    let path = w.path.to_path()?;
    let ceiling = SummaryLimits::thorough();
    if l.max_index_entries > ceiling.max_index_entries
        || l.max_index_bytes > ceiling.max_index_bytes
        || l.max_paths > ceiling.max_paths
        || l.max_path_bytes > ceiling.max_path_bytes
        || l.max_total_bytes > ceiling.max_total_bytes
        || l.max_file_bytes > ceiling.max_file_bytes
        || l.max_objects > ceiling.max_objects
        || l.max_depth > ceiling.max_depth
        || l.max_summary_bytes > ceiling.max_summary_bytes
        || l.timeout > ceiling.timeout
    {
        return Err(SweepError::new(
            "local_limits",
            "local_summary",
            &path,
            "summary limits exceed hard ceilings",
        ));
    }
    let mut c = Collector {
        metrics: SummaryMetrics::default(),
        s: LocalSummary {
            started_unix_ms: now_ms(),
            temporal_consistency: "not_guaranteed".into(),
            ..LocalSummary::default()
        },
        limits: l,
        hooks: h,
        started: Instant::now(),
        bytes: 0,
        objects: 0,
        paths: 0,
        path_bytes: 0,
        stopped: false,
        git_failed: false,
        first_git_timeout: None,
        diagnostic_bytes: 0,
        blobs: BTreeMap::new(),
        comparison: projection.map(|(rows, bytes)| comparison::SourceCapture::new(l, rows, bytes)),
    };
    if let Err(e) = run(&mut c, w, &path) {
        c.gap(&path, e.to_string());
    }
    if c.stopped {
        c.s.head_complete = false;
        c.s.index_complete = false;
        c.s.disk_complete = false;
        for count in [
            &mut c.s.staged,
            &mut c.s.unstaged,
            &mut c.s.conflicts,
            &mut c.s.untracked,
            &mut c.s.ignored,
        ] {
            count.complete = false;
        }
    }
    c.s.finished_unix_ms = now_ms();
    c.s.details_truncated |= [
        &c.s.staged,
        &c.s.unstaged,
        &c.s.conflicts,
        &c.s.untracked,
        &c.s.ignored,
    ]
    .iter()
    .any(|count| count.samples_omitted > 0);
    c.s.digest_sha256 = crate::output::canonical_digest(&c.s)?;
    if serde_json::to_vec(&c.s)
        .map_err(|e| SweepError::new("local_serialize", "local_summary", &path, e.to_string()))?
        .len()
        > l.max_summary_bytes
    {
        c.s.details_truncated = true;
        for count in [
            &mut c.s.staged,
            &mut c.s.unstaged,
            &mut c.s.conflicts,
            &mut c.s.untracked,
            &mut c.s.ignored,
        ] {
            count.samples_omitted += count.samples.len();
            count.samples.clear();
        }
        c.s.gaps.clear();
        c.diagnostic_bytes = 0;
        c.gap(&path, "summary details omitted by output byte limit");
        c.s.digest_sha256 = crate::output::canonical_digest(&c.s)?;
        if serde_json::to_vec(&c.s)
            .map_err(|e| SweepError::new("local_serialize", "local_summary", &path, e.to_string()))?
            .len()
            > l.max_summary_bytes
        {
            return Err(SweepError::new(
                "local_summary_limit",
                "local_summary",
                &path,
                "compact summary exceeds output byte limit",
            ));
        }
    }
    if let Some(capture) = &mut c.comparison {
        capture.identity = c.s.worktree_identity.clone();
        capture.sources = c.s.sources.clone();
        capture.summary_sha256 = c.s.digest_sha256.clone();
        capture.remaining.max_total_bytes = l.max_total_bytes.saturating_sub(c.bytes);
        capture.remaining.max_objects = l.max_objects.saturating_sub(c.objects);
        capture.remaining.timeout = l.timeout.saturating_sub(c.started.elapsed());
    }
    Ok(SummaryOutcome {
        comparison: c.comparison,
        summary: c.s,
        first_git_timeout: c.first_git_timeout,
        metrics: c.metrics,
    })
}
fn resolve(c: &mut Collector<'_>, root: &OwnedFd, path: &Path) -> io::Result<String> {
    let raw = c.git(
        root,
        path,
        GitRead::ResolveCommit {
            reference: BytePath::from_bytes(b"HEAD"),
        },
    )?;
    let oid = text(
        raw.strip_suffix(b"\n")
            .ok_or_else(|| io::Error::other("unterminated HEAD OID"))?,
    )?;
    if !matches!(oid.len(), 40 | 64)
        || !oid.bytes().all(|b| b.is_ascii_hexdigit())
        || oid.bytes().all(|b| b == b'0')
    {
        return Err(io::Error::other("invalid HEAD OID"));
    }
    Ok(oid.to_ascii_lowercase())
}
fn run(c: &mut Collector<'_>, w: &Worktree, path: &Path) -> io::Result<()> {
    c.check()?;
    if w.bare {
        return Err(io::Error::other("bare repository has no worktree disk"));
    }
    let root = open_absolute_directory(path)?;
    let before = fstat(&root)?;
    c.s.worktree_identity = Some(identity(&before));
    if w.identity.as_ref() != c.s.worktree_identity.as_ref() {
        c.drift(path, "worktree identity differs from supplied observation");
        return Ok(());
    }
    c.hooks.before_entry(&w.path)?;
    if c.hooks.is_dataless(&w.path, &before) {
        return Err(io::Error::other("dataless worktree; reads refused"));
    }
    let marker_before = statat(&root, ".git", AtFlags::SYMLINK_NOFOLLOW)?;
    if !matches!(
        marker_before.st_mode & libc::S_IFMT,
        libc::S_IFREG | libc::S_IFDIR
    ) {
        return Err(io::Error::other("unsafe worktree administration marker"));
    }
    let marker_path = BytePath::from_path(&path.join(".git"));
    c.hooks.before_entry(&marker_path)?;
    if c.hooks.is_dataless(&marker_path, &marker_before) {
        return Err(io::Error::other(
            "dataless administration marker; Git refused",
        ));
    }
    let admin_path = path_from_output(&c.git(&root, path, GitRead::GitDir)?)?;
    let admin = open_absolute_directory(&admin_path)?;
    let admin_before = fstat(&admin)?;
    let bp = BytePath::from_path(&admin_path);
    c.hooks.before_entry(&bp)?;
    if c.hooks.is_dataless(&bp, &admin_before) {
        return Err(io::Error::other("dataless administration; reads refused"));
    }
    c.s.git_dir = Some(bp);
    let hash_size = match c.git(&root, path, GitRead::ObjectFormat)?.as_slice() {
        b"sha1\n" => 20,
        b"sha256\n" => 32,
        _ => return Err(io::Error::other("unsupported Git object format")),
    };
    let head_raw = match c.source(&admin, &admin_path, "HEAD", 256 * 1024) {
        Ok(v) => Some(v),
        Err(e) => {
            c.gap(
                &admin_path.join("HEAD"),
                format!("raw HEAD unavailable: {e}"),
            );
            None
        }
    };
    let index_raw = match c.source(&admin, &admin_path, "index", c.limits.max_index_bytes) {
        Ok(v) => Some(v),
        Err(e) => {
            c.gap(
                &admin_path.join("index"),
                format!("raw index unavailable: {e}"),
            );
            None
        }
    };
    let mut index = BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    let mut intent_to_add = BTreeSet::new();
    if let Some((_, raw)) = &index_raw {
        match parse_raw_index(raw, hash_size, c.limits.max_index_entries) {
            Ok(parsed) if parsed.shared.is_none() => {
                intent_to_add = parsed.intent_to_add;
                let mut valid = true;
                for e in parsed.entries {
                    if let Err(error) = c.reserve_path(e.path.hex.len() / 2) {
                        valid = false;
                        c.gap(&admin_path.join("index"), error.to_string());
                        break;
                    }
                    if e.mode == 0o160000 {
                        valid = false;
                        c.gap(
                            &e.path
                                .to_path()
                                .map_err(|e| io::Error::other(e.to_string()))?,
                            "gitlink index entry is unsupported",
                        );
                    }
                    if e.stage > 0 {
                        conflicts.insert(e.path.clone());
                        c.capture(
                            &e.path,
                            comparison::Category::Conflicts,
                            comparison::Layer::Index,
                            Some(e.stage),
                            comparison::Value::unknown("unmerged index stage"),
                            Some(e.oid),
                        );
                    } else {
                        index.insert(e.path.clone(), e);
                    }
                }
                // Ordinary indexes cannot contain both a stage-zero entry and conflicts
                // for the same path, or a file whose ancestor is another entry.
                for bp in index.keys().chain(conflicts.iter()) {
                    let b = bp.bytes().map_err(|e| io::Error::other(e.to_string()))?;
                    for (n, v) in b.iter().enumerate() {
                        if *v == b'/' {
                            let parent = BytePath::from_bytes(&b[..n]);
                            if index.contains_key(&parent) || conflicts.contains(&parent) {
                                valid = false;
                            }
                        }
                    }
                    if index.contains_key(bp) && conflicts.contains(bp) {
                        valid = false;
                    }
                }
                // The raw checksum/stages are verified even when another path's
                // semantics are unsupported. Preserve distinct observed conflicts.
                for bp in &conflicts {
                    observe(&mut c.s.conflicts, bp);
                }
                if valid {
                    c.s.index_complete = true;
                    c.s.conflicts.complete = true;
                } else {
                    c.gap(
                        &admin_path.join("index"),
                        "unsupported or contradictory index paths",
                    );
                }
            }
            Ok(_) => c.gap(
                &admin_path.join("index"),
                "split index refused without mutation",
            ),
            Err(e) => c.gap(&admin_path.join("index"), e.to_string()),
        }
    }
    let mut head_tree = BTreeMap::new();
    let mut head_paths_known = false;
    match resolve(c, &root, path) {
        Ok(oid) => {
            c.s.head = Some(oid.clone());
            let tree = c
                .object(&root, path, "commit", &oid)
                .and_then(|raw| {
                    let first = raw
                        .split(|b| *b == b'\n')
                        .next()
                        .ok_or_else(|| io::Error::other("missing commit tree"))?;
                    let oid = text(
                        first
                            .strip_prefix(b"tree ")
                            .ok_or_else(|| io::Error::other("missing commit tree"))?,
                    )?;
                    if oid.len() != hash_size * 2 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
                        return Err(io::Error::other("invalid commit tree OID"));
                    }
                    Ok(oid.to_owned())
                })
                .and_then(|oid| c.trees(&root, path, &oid, hash_size));
            match tree {
                Ok(tree) => {
                    head_tree = tree;
                    head_paths_known = true;
                    c.s.head_complete = head_raw.is_some();
                }
                Err(e) => c.gap(path, format!("HEAD tree unavailable: {e}")),
            }
        }
        Err(e) => c.gap(path, format!("unborn or unavailable HEAD: {e}")),
    }
    if c.s.index_complete && head_paths_known {
        c.s.staged.complete = true;
        let paths = head_tree
            .keys()
            .chain(index.keys())
            .chain(conflicts.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        for bp in paths {
            // An intent entry records a future addition, not staged content.
            let staged = index.get(&bp).filter(|_| !intent_to_add.contains(&bp));
            let different = match (head_tree.get(&bp), staged) {
                (Some(a), Some(b)) => a.mode != b.mode || a.oid != b.oid,
                (None, None) => conflicts.contains(&bp),
                _ => true,
            };
            if different {
                observe(&mut c.s.staged, &bp);
            }
        }
    }
    c.prefetch_blobs(
        &root,
        path,
        head_tree
            .values()
            .chain(index.values())
            .filter(|entry| entry.mode != 0o160000 && !intent_to_add.contains(&entry.path))
            .map(|entry| entry.oid.as_str()),
    );
    for entry in head_tree.values() {
        if let Err(e) = c.blob(&root, path, &entry.oid) {
            c.s.head_complete = false;
            c.gap(
                &entry
                    .path
                    .to_path()
                    .map_err(|e| io::Error::other(e.to_string()))?,
                format!("HEAD blob unavailable: {e}"),
            );
            if c.git_failed || c.stopped {
                break;
            }
        }
    }
    let mut index_blobs_complete = c.s.index_complete;
    for entry in index.values() {
        if intent_to_add.contains(&entry.path) {
            continue;
        }
        if entry.mode == 0o160000 {
            index_blobs_complete = false;
            continue;
        }
        if let Err(e) = c.blob(&root, path, &entry.oid) {
            index_blobs_complete = false;
            c.gap(
                &entry
                    .path
                    .to_path()
                    .map_err(|e| io::Error::other(e.to_string()))?,
                format!("index blob unavailable: {e}"),
            );
            if c.git_failed || c.stopped {
                break;
            }
        }
    }
    if c.comparison.is_some() {
        for bp in head_tree
            .keys()
            .chain(index.keys())
            .chain(conflicts.iter())
            .cloned()
            .collect::<BTreeSet<_>>()
        {
            let staged = index.get(&bp).filter(|_| !intent_to_add.contains(&bp));
            let different = match (head_tree.get(&bp), staged) {
                (Some(a), Some(b)) => a.mode != b.mode || a.oid != b.oid,
                (None, None) => conflicts.contains(&bp),
                _ => true,
            };
            if different && c.s.staged.observed > 0 {
                let value = if conflicts.contains(&bp) {
                    comparison::Value::unknown("unmerged index stages")
                } else if let Some(entry) = staged {
                    match c.blobs.get(&entry.oid) {
                        Some(Ok(d)) => comparison::Value::git(d.clone(), entry.mode),
                        _ => comparison::Value::unknown("index blob unavailable"),
                    }
                } else {
                    comparison::Value::Absent { parent: None }
                };
                c.capture(
                    &bp,
                    comparison::Category::Staged,
                    comparison::Layer::Index,
                    Some(0),
                    value,
                    staged.map(|e| e.oid.clone()),
                );
            }
        }
    }
    c.disk(
        &root,
        path,
        &index,
        &conflicts,
        &intent_to_add,
        c.s.index_complete,
    );
    c.s.unstaged.complete &= index_blobs_complete;
    // Revalidate bytes rather than trusting size/mtime or cached inventory HEAD.
    for (name, original, max) in [
        ("HEAD", head_raw.as_ref(), 256 * 1024),
        ("index", index_raw.as_ref(), c.limits.max_index_bytes),
    ] {
        if let Some((digest, _)) = original {
            match c.read(&admin, OsStr::new(name), &admin_path.join(name), max, false) {
                Ok((after, _)) if &after == digest => {}
                Ok(_) => c.drift(
                    &admin_path.join(name),
                    "administration bytes changed during collection",
                ),
                Err(e) => {
                    c.s.head_complete = false;
                    c.s.index_complete = false;
                    c.s.staged.complete = false;
                    c.s.unstaged.complete = false;
                    c.s.conflicts.complete = false;
                    c.gap(
                        &admin_path.join(name),
                        format!("administration revalidation failed: {e}"),
                    );
                }
            }
        }
    }
    if let Some(original) = c.s.head.clone() {
        match resolve(c, &root, path) {
            Ok(after) if after == original => {}
            Ok(_) => c.drift(path, "HEAD changed during collection"),
            Err(e) => {
                c.s.head_complete = false;
                c.s.staged.complete = false;
                c.gap(path, format!("HEAD revalidation failed: {e}"));
            }
        }
    }
    for (observed, target) in [(&before, path), (&admin_before, admin_path.as_path())] {
        let result = c
            .hooks
            .before_entry(&BytePath::from_path(target))
            .and_then(|_| open_absolute_directory(target))
            .and_then(|fd| fstat(&fd).map_err(Into::into));
        match result {
            Ok(after) if same_stat(observed, &after) => {}
            Ok(_) => c.drift(
                target,
                "worktree or administration identity/metadata changed",
            ),
            Err(e) => c.invalidate(target, &format!("directory revalidation unavailable: {e}")),
        }
    }
    match statat(&root, ".git", AtFlags::SYMLINK_NOFOLLOW) {
        Ok(after) if same_stat(&marker_before, &after) => {}
        Ok(_) => c.drift(path, "administration marker changed"),
        Err(e) => c.invalidate(
            path,
            &format!("administration marker revalidation unavailable: {e}"),
        ),
    }
    Ok(())
}

pub(crate) fn collect_for_comparison(
    w: &Worktree,
    l: &SummaryLimits,
    max_rows: usize,
    max_bytes: usize,
) -> Result<SummaryOutcome, SweepError> {
    collect_outcome_with_projection(w, l, &NativeHooks, Some((max_rows, max_bytes)))
}
pub(crate) struct CheckoutBinding {
    pub path: BytePath,
    pub identity: Identity,
    pub common_dir: BytePath,
    pub common_dir_identity: Identity,
}
pub(crate) fn validate_checkout(
    path: &Path,
    family: &crate::model::Family,
    timeout: Duration,
) -> Result<CheckoutBinding, SweepError> {
    let check = || -> io::Result<CheckoutBinding> {
        let root = open_absolute_directory(path)?;
        let st = fstat(&root)?;
        if NativeHooks.is_dataless(&BytePath::from_path(path), &st) {
            return Err(io::Error::other("dataless reference root"));
        }
        let marker = statat(&root, ".git", AtFlags::SYMLINK_NOFOLLOW)?;
        if !matches!(marker.st_mode & libc::S_IFMT, libc::S_IFREG | libc::S_IFDIR)
            || flags(&marker) & 0x4000_0000 != 0
        {
            return Err(io::Error::other("unsafe reference Git marker"));
        }
        let common = path_from_output(
            &ControlledGit::with_output_limit(timeout, 64 * 1024)
                .read_from_directory(&root, path, GitRead::CommonDir)
                .map_err(|e| io::Error::other(e.to_string()))?,
        )?;
        let common_fd = open_absolute_directory(&common)?;
        if identity(&fstat(&common_fd)?) != family.identity
            || BytePath::from_path(&common) != family.common_dir
        {
            return Err(io::Error::other(
                "reference checkout belongs to another physical Git family",
            ));
        }
        Ok(CheckoutBinding {
            path: BytePath::from_path(path),
            identity: identity(&st),
            common_dir: family.common_dir.clone(),
            common_dir_identity: family.identity.clone(),
        })
    };
    check().map_err(|e| {
        SweepError::new(
            "local_comparison",
            "reference_checkout",
            path,
            e.to_string(),
        )
    })
}
fn comparison_collector<'a>(limits: &'a SummaryLimits, hooks: &'a dyn LocalHooks) -> Collector<'a> {
    Collector {
        s: LocalSummary::default(),
        metrics: SummaryMetrics::default(),
        limits,
        hooks,
        started: Instant::now(),
        bytes: 0,
        objects: 0,
        paths: 0,
        path_bytes: 0,
        stopped: false,
        git_failed: false,
        first_git_timeout: None,
        diagnostic_bytes: 0,
        blobs: BTreeMap::new(),
        comparison: None,
    }
}
fn checkout_administration(
    c: &mut Collector<'_>,
    root: &OwnedFd,
    binding: &CheckoutBinding,
) -> io::Result<comparison::CheckoutAdministration> {
    c.check()?;
    let path = binding
        .path
        .to_path()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let root_stat = fstat(root)?;
    if identity(&root_stat) != binding.identity || c.hooks.is_dataless(&binding.path, &root_stat) {
        return Err(io::Error::other("reference root changed or dataless"));
    }
    let marker = statat(root, ".git", AtFlags::SYMLINK_NOFOLLOW)?;
    let marker_path = path.join(".git");
    if c.hooks
        .is_dataless(&BytePath::from_path(&marker_path), &marker)
    {
        return Err(io::Error::other("dataless reference marker"));
    }
    let marker_sha256 = match marker.st_mode & libc::S_IFMT {
        libc::S_IFDIR => None,
        libc::S_IFREG => Some(
            c.read(root, OsStr::new(".git"), &marker_path, 256 * 1024, false)?
                .0,
        ),
        _ => return Err(io::Error::other("unsafe reference Git marker")),
    };
    let git_dir = path_from_output(&c.git(root, &path, GitRead::GitDir)?)?;
    let common_dir = path_from_output(&c.git(root, &path, GitRead::CommonDir)?)?;
    let admin = open_absolute_directory(&git_dir)?;
    let common = open_absolute_directory(&common_dir)?;
    let admin_stat = fstat(&admin)?;
    let common_stat = fstat(&common)?;
    if identity(&common_stat) != binding.common_dir_identity
        || BytePath::from_path(&common_dir) != binding.common_dir
        || c.hooks
            .is_dataless(&BytePath::from_path(&git_dir), &admin_stat)
        || c.hooks
            .is_dataless(&BytePath::from_path(&common_dir), &common_stat)
    {
        return Err(io::Error::other(
            "reference physical Git family changed or dataless",
        ));
    }
    let commondir = checkout_value(c, &admin, &BytePath::from_bytes(b"commondir"))?;
    if matches!(commondir, comparison::Value::Unknown { .. }) {
        return Err(io::Error::other("reference commondir unreadable"));
    }
    if !same_stat(&marker, &statat(root, ".git", AtFlags::SYMLINK_NOFOLLOW)?)
        || !same_stat(&root_stat, &fstat(root)?)
    {
        return Err(io::Error::other(
            "reference administration changed during capture",
        ));
    }
    Ok(comparison::CheckoutAdministration {
        root: comparison::FsWitness::from_stat(&root_stat),
        marker: comparison::FsWitness::from_stat(&marker),
        marker_sha256,
        git_dir: BytePath::from_path(&git_dir),
        git_dir_identity: identity(&admin_stat),
        common_dir: BytePath::from_path(&common_dir),
        common_dir_identity: identity(&common_stat),
        commondir,
    })
}
fn checkout_value(
    c: &mut Collector<'_>,
    root: &OwnedFd,
    relative: &BytePath,
) -> io::Result<comparison::Value> {
    let bytes = relative
        .bytes()
        .map_err(|e| io::Error::other(e.to_string()))?;
    if !valid_relative(&bytes) {
        return Err(io::Error::other("unsafe reference-relative path"));
    }
    let path = relative
        .to_path()
        .map_err(|e| io::Error::other(e.to_string()))?;
    let mut fd = openat(root, ".", DIRECTORY, Mode::empty())?;
    let components = path.components().collect::<Vec<_>>();
    let mut parents = Vec::new();
    let mut value = comparison::Value::unknown("no final path component");
    for (i, component) in components.iter().enumerate() {
        c.check()?;
        let Component::Normal(name) = *component else {
            return Err(io::Error::other("unsafe reference component"));
        };
        let before = fstat(&fd)?;
        let child = statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW);
        let st = match child {
            Err(rustix::io::Errno::NOENT) => {
                if !same_stat(&before, &fstat(&fd)?) {
                    return Err(io::Error::other("absence parent changed"));
                }
                value = comparison::Value::Absent {
                    parent: Some(comparison::FsWitness::from_stat(&before)),
                };
                break;
            }
            Err(e) => return Err(e.into()),
            Ok(st) => st,
        };
        if c.hooks.is_dataless(relative, &st) {
            return Err(io::Error::other("dataless reference entry"));
        }
        if i + 1 < components.len() {
            let next = openat(&fd, name, DIRECTORY, Mode::empty())?;
            if !same_stat(&st, &fstat(&next)?) {
                return Err(io::Error::other("reference directory changed"));
            }
            match statat(&next, ".git", AtFlags::SYMLINK_NOFOLLOW) {
                Ok(_) => return Err(io::Error::other("nested reference repository")),
                Err(rustix::io::Errno::NOENT) => (),
                Err(e) => return Err(e.into()),
            }
            let mut bare = true;
            // The bounded discovery signature also protects reference reads.
            for admin_name in ["HEAD", "objects", "refs"] {
                match statat(&next, admin_name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => (),
                    Err(rustix::io::Errno::NOENT) => bare = false,
                    Err(e) => return Err(e.into()),
                }
            }
            if bare {
                return Err(io::Error::other("nested bare reference administration"));
            }
            parents.push((fd, (*name).to_os_string(), st));
            fd = next;
            continue;
        }
        let kind = st.st_mode & libc::S_IFMT;
        let (mode, digest) = if kind == libc::S_IFREG {
            let mode = if st.st_mode & 0o100 != 0 {
                0o100755
            } else {
                0o100644
            };
            (
                mode,
                Some(c.read(&fd, name, &path, c.limits.max_file_bytes, false)?.0),
            )
        } else if kind == libc::S_IFLNK {
            let mut buffer = [0u8; 4096];
            let n = rustix::fs::readlinkat_raw(&fd, name, &mut buffer[..])?;
            if n == buffer.len() {
                return Err(io::Error::other("reference symlink exceeds limit"));
            }
            c.charge(n)?;
            (
                0o120000,
                Some(format!("{:x}", Sha256::digest(&buffer[..n]))),
            )
        } else {
            return Err(io::Error::other("unsupported reference file type"));
        };
        if !same_stat(&st, &statat(&fd, name, AtFlags::SYMLINK_NOFOLLOW)?)
            || !same_stat(&before, &fstat(&fd)?)
        {
            return Err(io::Error::other("reference entry or parent changed"));
        }
        value = comparison::Value::disk(digest, mode, &st);
    }
    for (parent, name, before) in parents {
        if !same_stat(&before, &statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW)?) {
            return Err(io::Error::other("reference path namespace changed"));
        }
    }
    Ok(value)
}
type ComparisonOutput = (
    Vec<comparison::Reference>,
    Vec<comparison::Row>,
    Vec<String>,
);
pub(crate) fn compare_capture(
    path: &BytePath,
    capture: comparison::SourceCapture,
    targets: &[BytePath],
    checkout: Option<&CheckoutBinding>,
    source_valid: bool,
    capacity: usize,
) -> Result<ComparisonOutput, SweepError> {
    compare_capture_with(
        path,
        capture,
        targets,
        checkout,
        source_valid,
        capacity,
        &NativeHooks,
    )
}
fn compare_capture_with(
    path: &BytePath,
    capture: comparison::SourceCapture,
    targets: &[BytePath],
    checkout: Option<&CheckoutBinding>,
    source_valid: bool,
    capacity: usize,
    hooks: &dyn LocalHooks,
) -> Result<ComparisonOutput, SweepError> {
    use comparison::{Lookup, Reference, Row, Value};
    let started = now_ms();
    let source_path = path.to_path()?;
    let root = open_absolute_directory(&source_path);
    let mut valid = source_valid
        && root
            .as_ref()
            .ok()
            .and_then(|fd| fstat(fd).ok())
            .is_some_and(|s| Some(identity(&s)) == capture.identity);
    let root = root.ok();
    let mut c = comparison_collector(&capture.remaining, hooks);
    if checkout.is_some_and(|binding| capture.identity.as_ref() == Some(&binding.identity)) {
        return Err(SweepError::new(
            "local_comparison",
            "reference_checkout",
            &source_path,
            "reference checkout cannot be the source physical worktree",
        ));
    }
    let mut gaps = Vec::new();
    if capture.omitted > 0 {
        gaps.push("source details omitted by bounded projection".into());
    }
    let mut refs = Vec::new();
    let mut trees = Vec::new();
    for name in targets {
        let read = || -> io::Result<(String, String, BTreeMap<BytePath, IndexEntry>)> {
            let root = root
                .as_ref()
                .ok_or_else(|| io::Error::other("source root unavailable"))?;
            let raw = c.git(
                root,
                &source_path,
                GitRead::ResolveCommit {
                    reference: name.clone(),
                },
            )?;
            let oid = text(
                raw.strip_suffix(b"\n")
                    .ok_or_else(|| io::Error::other("unterminated ref OID"))?,
            )?
            .to_owned();
            let raw = c.object(root, &source_path, "commit", &oid)?;
            let tree = text(
                raw.split(|b| *b == b'\n')
                    .next()
                    .and_then(|line| line.strip_prefix(b"tree "))
                    .ok_or_else(|| io::Error::other("missing target tree"))?,
            )?
            .to_owned();
            let entries = c.trees(root, &source_path, &tree, oid.len() / 2)?;
            Ok((oid, tree, entries))
        };
        let mut read = read;
        let result = read();
        let (commit_oid, tree_oid, entries, gap) = match result {
            Ok((oid, tree, entries)) => (Some(oid), Some(tree), Some(entries), None),
            Err(_) => (
                None,
                None,
                None,
                Some("Git target unreadable, changed or budget exhausted".into()),
            ),
        };
        refs.push(Reference {
            origin: "git".into(),
            name: name.clone(),
            commit_oid,
            tree_oid,
            identity: None,
            administration: None,
            started_unix_ms: started,
            finished_unix_ms: now_ms(),
            checks: if gap.is_none() { "stable" } else { "unknown" }.into(),
            gap,
        });
        trees.push(entries);
    }
    let checkout_root = checkout
        .and_then(|binding| binding.path.to_path().ok())
        .and_then(|p| open_absolute_directory(&p).ok());
    let checkout_before = checkout_root
        .as_ref()
        .zip(checkout)
        .and_then(|(fd, binding)| checkout_administration(&mut c, fd, binding).ok());
    if let Some(binding) = checkout {
        refs.push(Reference {
            origin: "checkout".into(),
            name: binding.path.clone(),
            commit_oid: None,
            tree_oid: None,
            identity: checkout_before.as_ref().map(|w| w.root.identity.clone()),
            administration: checkout_before.clone(),
            started_unix_ms: started,
            finished_unix_ms: now_ms(),
            checks: if checkout_before.is_some() {
                "stable"
            } else {
                "unknown"
            }
            .into(),
            gap: checkout_before
                .is_none()
                .then(|| "reference checkout unavailable".into()),
        });
    }
    let mut rows = Vec::new();
    let mut bytes = crate::output::bounded_json_size(&refs, capacity)?;
    for mut change in capture.rows {
        if !valid {
            change.source =
                Value::unknown("source final identity or administration checks unavailable");
        }
        let mut lookups = Vec::new();
        for (i, reference) in refs.iter().enumerate() {
            let mut object_oid = None;
            let value = if reference.origin == "checkout" {
                checkout_root
                    .as_ref()
                    .filter(|_| checkout_before.is_some())
                    .map_or_else(
                        || Value::unknown("checkout unavailable"),
                        |fd| {
                            checkout_value(&mut c, fd, &change.path).unwrap_or_else(|_| {
                            Value::unknown(
                                "checkout content unavailable, unsafe, changed or budget exhausted",
                            )
                        })
                        },
                    )
            } else if let Some(entries) = &trees[i] {
                if let Some(entry) = entries.get(&change.path) {
                    object_oid = Some(entry.oid.clone());
                    root.as_ref().map_or_else(
                        || Value::unknown("source root unavailable"),
                        |fd| {
                            c.blob(fd, &source_path, &entry.oid)
                                .map(|d| Value::git(d, entry.mode))
                                .unwrap_or_else(|_| {
                                    Value::unknown("target blob unavailable or budget exhausted")
                                })
                        },
                    )
                } else {
                    Value::Absent { parent: None }
                }
            } else {
                Value::unknown("target tree unavailable")
            };
            lookups.push(Lookup {
                reference: i,
                value,
                object_oid,
            });
        }
        let result = comparison::classify(&change, &lookups, &refs);
        let row = Row {
            change,
            lookups,
            result,
        };
        let n = crate::output::bounded_json_size(&row, capacity)
            .ok()
            .map(|n| n.saturating_add(512));
        if n.is_none_or(|n| n > capacity.saturating_sub(bytes)) {
            gaps.push("comparison details omitted by run byte limit".into());
            break;
        }
        if let Some(n) = n {
            bytes += n;
            rows.push(row);
        }
    }
    // Fresh final checks apply even when blob digests came from a cache.
    for (i, reference) in refs.iter_mut().enumerate() {
        let stable = if reference.origin == "git" {
            root.as_ref()
                .zip(reference.commit_oid.as_ref())
                .is_some_and(|(fd, oid)| {
                    c.git(
                        fd,
                        &source_path,
                        GitRead::ResolveCommit {
                            reference: reference.name.clone(),
                        },
                    )
                    .is_ok_and(|raw| raw == format!("{oid}\n").as_bytes())
                })
        } else {
            checkout
                .zip(checkout_before.as_ref())
                .is_some_and(|(binding, before)| {
                    binding
                        .path
                        .to_path()
                        .ok()
                        .and_then(|p| open_absolute_directory(&p).ok())
                        .is_some_and(|fd| {
                            checkout_administration(&mut c, &fd, binding)
                                .is_ok_and(|after| before == &after)
                        })
                })
        };
        reference.finished_unix_ms = now_ms();
        if !stable {
            reference.checks = "unknown".into();
            reference.gap = Some("reference final validation unavailable or changed".into());
            for row in &mut rows {
                row.lookups[i].value =
                    Value::unknown("reference final validation unavailable or changed");
            }
        }
    }
    for source in &capture.sources {
        let check = source
            .path
            .to_path()
            .ok()
            .and_then(|p| p.parent().map(|parent| (p.clone(), parent.to_path_buf())))
            .and_then(|(p, parent)| open_absolute_directory(&parent).ok().map(|fd| (p, fd)));
        valid &= check.is_some_and(|(p, fd)| {
            p.file_name().is_some_and(|name| {
                c.read(&fd, name, &p, c.limits.max_index_bytes, false)
                    .is_ok_and(|(digest, _)| digest == source.sha256)
            })
        });
    }
    // Re-observe changed disk entries under the remaining allowance. Saved
    // fingerprints are observations, never permission to trust a later payload.
    for row in &mut rows {
        if row.change.layer == comparison::Layer::Disk {
            let current = root
                .as_ref()
                .and_then(|fd| checkout_value(&mut c, fd, &row.change.path).ok());
            if current
                .as_ref()
                .is_some_and(|current| comparison::same_disk_value(&row.change.source, current))
            {
                if matches!(row.change.source, Value::Absent { .. }) {
                    row.change.source =
                        current.unwrap_or_else(|| Value::unknown("source absence unavailable"));
                }
            } else {
                row.change.source = Value::unknown("source file changed or final read unavailable");
            }
        }
    }
    valid &= source_path.as_path().is_absolute()
        && open_absolute_directory(&source_path)
            .ok()
            .and_then(|fd| fstat(&fd).ok())
            .is_some_and(|s| Some(identity(&s)) == capture.identity);
    if !valid {
        gaps.push("source final checks unavailable, changed or budget exhausted".into());
        for row in &mut rows {
            row.change.source = Value::unknown("source final checks unavailable or changed");
        }
    }
    for row in &mut rows {
        row.result = comparison::classify(&row.change, &row.lookups, &refs);
    }
    Ok((refs, rows, gaps))
}

#[cfg(test)]
mod comparison_drift_tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    struct Redirect<'a> {
        marker: &'a Path,
        foreign: &'a Path,
        changed: AtomicBool,
    }
    impl LocalHooks for Redirect<'_> {
        fn after_file_open(&self, path: &BytePath) -> io::Result<()> {
            if path.bytes().is_ok_and(|p| p == b"payload")
                && !self.changed.swap(true, Ordering::Relaxed)
            {
                // Rewrite the existing inode: the parent directory does not change.
                fs::write(self.marker, format!("gitdir: {}\n", self.foreign.display()))?;
            }
            Ok(())
        }
    }
    #[test]
    fn reference_git_marker_rewrite_invalidates_only_checkout_proofs() {
        let f = Fixture(PathBuf::from(format!(
            "/private/tmp/wt-sweep-comparison-drift-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )));
        fs::create_dir(&f.0).unwrap_or_else(|e| panic!("{e}"));
        let source = f.0.join("source");
        let foreign = f.0.join("foreign");
        for root in [&source, &foreign] {
            let status = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args(["init", "-q", "-b", "main"])
                .arg(root)
                .status()
                .unwrap_or_else(|e| panic!("{e}"));
            assert!(status.success());
        }
        let reference = f.0.join("reference");
        fs::create_dir(&reference).unwrap_or_else(|e| panic!("{e}"));
        let marker = reference.join(".git");
        fs::write(
            &marker,
            format!("gitdir: {}\n", source.join(".git").display()),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        for root in [&source, &reference] {
            fs::write(root.join("payload"), b"same\n").unwrap_or_else(|e| panic!("{e}"));
        }
        let limits = SummaryLimits::default();
        let source_fd = open_absolute_directory(&source).unwrap_or_else(|e| panic!("{e}"));
        let mut c = comparison_collector(&limits, &NativeHooks);
        let path = BytePath::from_bytes(b"payload");
        let value = checkout_value(&mut c, &source_fd, &path).unwrap_or_else(|e| panic!("{e}"));
        let capture = || {
            let mut capture = comparison::SourceCapture::new(&limits, 16, 65536);
            capture.identity = Some(identity(
                &fstat(&source_fd).unwrap_or_else(|e| panic!("{e}")),
            ));
            capture.push(comparison::SourceRow {
                path: path.clone(),
                category: comparison::Category::Untracked,
                layer: comparison::Layer::Disk,
                stage: None,
                source: value.clone(),
                source_oid: None,
            });
            capture
        };
        let source_bp = BytePath::from_path(&source);
        let common = source.join(".git");
        let binding = CheckoutBinding {
            path: BytePath::from_path(&reference),
            identity: identity(
                &fstat(open_absolute_directory(&reference).unwrap_or_else(|e| panic!("{e}")))
                    .unwrap_or_else(|e| panic!("{e}")),
            ),
            common_dir: BytePath::from_path(&common),
            common_dir_identity: identity(
                &fstat(open_absolute_directory(&common).unwrap_or_else(|e| panic!("{e}")))
                    .unwrap_or_else(|e| panic!("{e}")),
            ),
        };
        let (_, positive, _) = compare_capture_with(
            &source_bp,
            capture(),
            &[],
            Some(&binding),
            true,
            65536,
            &NativeHooks,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(positive[0].result, comparison::ResultKind::MatchesReference);
        let hooks = Redirect {
            marker: &marker,
            foreign: &foreign.join(".git"),
            changed: AtomicBool::new(false),
        };
        let (refs, rows, _) = compare_capture_with(
            &source_bp,
            capture(),
            &[],
            Some(&binding),
            true,
            65536,
            &hooks,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert!(hooks.changed.load(Ordering::Relaxed));
        assert_eq!(refs[0].checks, "unknown");
        assert_eq!(rows[0].result, comparison::ResultKind::Unknown);
        assert!(matches!(
            rows[0].change.source,
            comparison::Value::Present { .. }
        ));
    }
}

#[cfg(test)]
mod timeout_tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(format!(
                "/private/tmp/wt-sweep-summary-timeout-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap_or_else(|e| panic!("{e}"));
            let status = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .args(["init", "-q", "-b", "main"])
                .arg(&path)
                .status()
                .unwrap_or_else(|e| panic!("{e}"));
            assert!(status.success());
            let fifo = path.join("config-fifo");
            let status = Command::new("/usr/bin/mkfifo")
                .arg(&fifo)
                .status()
                .unwrap_or_else(|e| panic!("{e}"));
            assert!(status.success());
            let config = path.join(".git/config");
            let mut bytes = fs::read(&config).unwrap_or_else(|e| panic!("{e}"));
            bytes
                .extend_from_slice(format!("\n[include]\n path = {}\n", fifo.display()).as_bytes());
            fs::write(config, bytes).unwrap_or_else(|e| panic!("{e}"));
            Self(path)
        }
        fn worktree(&self) -> Worktree {
            use std::os::unix::fs::MetadataExt;
            let m = fs::metadata(&self.0).unwrap_or_else(|e| panic!("{e}"));
            Worktree {
                path: BytePath::from_path(&self.0),
                identity: Some(Identity {
                    device: m.dev(),
                    inode: m.ino(),
                }),
                head: None,
                branch: None,
                detached: false,
                bare: false,
                locked: None,
                prunable: None,
                path_state: crate::model::DirState::Present,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn near_summary_deadline_child_does_not_quarantine_family_or_spawn_dependents() {
        let f = Fixture::new();
        let fd = open_absolute_directory(&f.0).unwrap_or_else(|e| panic!("{e}"));
        let hooks = NativeHooks;
        let limits = SummaryLimits::default();
        let mut c = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now() - (limits.timeout - Duration::from_millis(100)),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        crate::git::take_child_read_journal();
        let error = match c.git(&fd, &f.0, GitRead::GitDir) {
            Err(error) => error,
            Ok(_) => panic!("owned FIFO did not reach shortened child deadline"),
        };
        let reads = crate::git::take_child_read_journal();
        assert_eq!(
            reads.len(),
            1,
            "deadline fixture must launch an actual Git child"
        );
        assert!(matches!(reads[0].1, GitRead::GitDir));
        assert!(
            c.first_git_timeout.is_none(),
            "facet deadline became family backend quarantine: {:?}",
            c.first_git_timeout
        );
        assert!(c.stopped, "exhausted facet must stop");
        assert!(error.to_string().contains("summary time budget"), "{error}");
        assert!(c.git(&fd, &f.0, GitRead::ObjectFormat).is_err());
        assert!(
            crate::git::take_child_read_journal().is_empty(),
            "stopped facet launched dependent read"
        );
        // Another collector retains its independent native Git evidence.
        let healthy = Fixture::new();
        fs::write(
            healthy.0.join(".git/config"),
            b"[core]\nrepositoryformatversion = 0\nbare = false\n",
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let healthy_fd = open_absolute_directory(&healthy.0).unwrap_or_else(|e| panic!("{e}"));
        let mut other = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now(),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        let bytes = other
            .git(&healthy_fd, &healthy.0, GitRead::ObjectFormat)
            .unwrap_or_else(|e| panic!("healthy independent collector: {e}"));
        assert_eq!(bytes, b"sha1\n");
        assert_eq!(crate::git::take_child_read_journal().len(), 1);
        assert!(!other.stopped && other.first_git_timeout.is_none());
    }
    #[test]
    fn first_runner_timeout_operation_is_exposed_for_review() {
        let f = Fixture::new();
        // Leave the default 30s facet allowance so the unchanged 10s child cap
        // proves a genuine backend timeout, independently of summary exhaustion.
        let limits = SummaryLimits::default();
        let outcome = crate::local::collect_summary_for_review(&f.worktree(), &limits)
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(
            matches!(outcome.first_git_timeout, Some(GitRead::GitDir)),
            "{:?}",
            outcome.first_git_timeout
        );
        assert!(!outcome.summary.head_complete && !outcome.summary.index_complete);
        assert!(!outcome.summary.gaps.is_empty());
        assert_eq!(outcome.metrics.git_processes, 1);
        assert_eq!(outcome.metrics.object_reads, 0);
    }
    #[test]
    fn timeout_keeps_full_blob_oid_and_ignore_paths_without_reclassifying_budgets() {
        let f = Fixture::new();
        let fd = open_absolute_directory(&f.0).unwrap_or_else(|e| panic!("{e}"));
        let hooks = NativeHooks;
        let oid = "ab".repeat(20);
        let probes = vec![BytePath::from_bytes(b"ignored\nname")];
        for op in [
            GitRead::Blob { oid: oid.clone() },
            GitRead::IgnorePaths {
                paths: probes.clone(),
            },
        ] {
            // The facet has 30s; the controlled child still has only 10s.
            let limits = SummaryLimits::default();
            let mut c = Collector {
                s: LocalSummary::default(),
                metrics: SummaryMetrics::default(),
                limits: &limits,
                hooks: &hooks,
                started: Instant::now(),
                bytes: 0,
                objects: 0,
                paths: 0,
                path_bytes: 0,
                stopped: false,
                git_failed: false,
                first_git_timeout: None,
                diagnostic_bytes: 0,
                blobs: BTreeMap::new(),
                comparison: None,
            };
            assert!(c.git(&fd, &f.0, op.clone()).is_err());
            match (&op, &c.first_git_timeout) {
                (GitRead::Blob { oid: a }, Some(GitRead::Blob { oid: b })) => assert_eq!(a, b),
                (GitRead::IgnorePaths { paths: a }, Some(GitRead::IgnorePaths { paths: b })) => {
                    assert_eq!(a, b)
                }
                _ => panic!("first operation lost: {:?}", c.first_git_timeout),
            }
            assert!(c.git(&fd, &f.0, GitRead::GitDir).is_err());
            assert!(matches!(
                (&op, &c.first_git_timeout),
                (GitRead::Blob { .. }, Some(GitRead::Blob { .. }))
                    | (
                        GitRead::IgnorePaths { .. },
                        Some(GitRead::IgnorePaths { .. })
                    )
            ));
        }
        let limits = SummaryLimits {
            max_objects: 0,
            ..SummaryLimits::default()
        };
        let mut c = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now(),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        assert!(c.object(&fd, &f.0, "blob", &oid).is_err());
        assert!(c.first_git_timeout.is_none());
    }
    #[test]
    fn cancelled_prefetch_records_initial_cause_without_spawning_git() {
        struct CancelAfterOne(std::cell::Cell<usize>);
        impl LocalHooks for CancelAfterOne {
            fn cancelled(&self) -> bool {
                let checks = self.0.get();
                self.0.set(checks + 1);
                checks > 0
            }
        }
        let f = Fixture::new();
        let fd = open_absolute_directory(&f.0).unwrap_or_else(|e| panic!("{e}"));
        let hooks = CancelAfterOne(std::cell::Cell::new(0));
        let limits = SummaryLimits::default();
        let mut c = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now(),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        let oids = ["ab".repeat(20), "cd".repeat(20)];
        crate::git::take_child_read_journal();
        c.prefetch_blobs(&fd, &f.0, oids.iter().map(String::as_str));
        assert!(c.stopped);
        assert_eq!(c.metrics.git_processes, 0);
        assert!(crate::git::take_child_read_journal().is_empty());
        assert!(
            c.s.gaps
                .iter()
                .any(|gap| gap.reason.contains("summary cancelled")),
            "initial cancellation was lost: {:?}",
            c.s.gaps
        );
        assert!(c.first_git_timeout.is_none());
    }

    #[test]
    fn byte_budget_stop_does_not_invent_cancellation_or_deadline() {
        let limits = SummaryLimits {
            max_total_bytes: 1,
            ..SummaryLimits::default()
        };
        let hooks = NativeHooks;
        let mut c = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now(),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        let error = c
            .charge(2)
            .err()
            .unwrap_or_else(|| panic!("byte ceiling was accepted"));
        c.gap(Path::new("resident"), error.to_string());
        observe(&mut c.s.staged, &BytePath::from_bytes(b"known-staged"));
        let later = c
            .check()
            .err()
            .unwrap_or_else(|| panic!("stopped collector resumed"));
        for invented in ["cancel", "deadline", "timeout"] {
            assert!(!later.to_string().contains(invented), "{later}");
        }
        assert_eq!(c.s.gaps[0].reason, "summary total byte limit reached");
        assert_eq!(c.s.staged.observed, 1);
        assert!(c.first_git_timeout.is_none());
    }

    #[test]
    fn completed_ignore_batch_positives_survive_later_byte_budget_failure() {
        let f = Fixture::new();
        fs::write(
            f.0.join(".git/config"),
            b"[core]\nrepositoryformatversion = 0\nbare = false\n",
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let status = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .arg("-C")
            .arg(&f.0)
            .args(["read-tree", "--empty"])
            .status()
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(status.success());
        fs::remove_file(f.0.join("config-fifo")).unwrap_or_else(|e| panic!("{e}"));
        fs::write(f.0.join(".gitignore"), b"ignored*\n").unwrap_or_else(|e| panic!("{e}"));
        fs::write(f.0.join("a-untracked"), b"resident").unwrap_or_else(|e| panic!("{e}"));
        for n in 0..300 {
            fs::write(f.0.join(format!("ignored{n:03}")), b"resident")
                .unwrap_or_else(|e| panic!("{e}"));
        }
        let limits = SummaryLimits {
            max_total_bytes: 3400,
            ..SummaryLimits::default()
        };
        let hooks = NativeHooks;
        let mut c = Collector {
            s: LocalSummary::default(),
            metrics: SummaryMetrics::default(),
            limits: &limits,
            hooks: &hooks,
            started: Instant::now(),
            bytes: 0,
            objects: 0,
            paths: 0,
            path_bytes: 0,
            stopped: false,
            git_failed: false,
            first_git_timeout: None,
            diagnostic_bytes: 0,
            blobs: BTreeMap::new(),
            comparison: None,
        };
        let fd = open_absolute_directory(&f.0).unwrap_or_else(|e| panic!("{e}"));
        c.disk(
            &fd,
            &f.0,
            &BTreeMap::new(),
            &BTreeSet::new(),
            &BTreeSet::new(),
            true,
        );
        assert_eq!(c.s.ignored.observed, 254, "{:?}", c.s);
        assert_eq!(c.s.untracked.observed, 2, "{:?}", c.s);
        assert!(!c.s.ignored.complete && !c.s.untracked.complete && !c.s.disk_complete);
        assert!(c.s.gaps.iter().any(|g| g.reason.contains("byte limit")));
        assert!(c.first_git_timeout.is_none());
    }
}

#[cfg(test)]
#[path = "summary_batch_tests.rs"]
mod batch_tests;
