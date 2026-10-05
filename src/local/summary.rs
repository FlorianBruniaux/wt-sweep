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
pub(crate) struct SummaryOutcome {
    pub summary: LocalSummary,
    pub first_git_timeout: Option<GitRead>,
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
struct Collector<'a> {
    s: LocalSummary,
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
}
impl Collector<'_> {
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
        if self.stopped || self.hooks.cancelled() || self.started.elapsed() >= self.limits.timeout {
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
        if bytes.len() as u64 > self.limits.max_file_bytes {
            return Err(io::Error::other("summary object byte limit reached"));
        }
        crate::review::verify_object(kind, oid, &bytes)
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(bytes)
    }
    fn blob(&mut self, fd: &OwnedFd, path: &Path, oid: &str) -> io::Result<String> {
        if let Some(result) = self.blobs.get(oid) {
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
        while let Some((oid, prefix, depth)) = stack.pop() {
            if depth > self.limits.max_depth {
                return Err(io::Error::other("HEAD tree depth limit reached"));
            }
            let raw = self.object(fd, path, "tree", &oid)?;
            let mut offset = 0;
            while offset < raw.len() {
                self.check()?;
                records += 1;
                if records > self.limits.max_paths {
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
                let mut full = prefix.clone();
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
        }
        Ok(result)
    }
    fn disk(
        &mut self,
        root: &OwnedFd,
        path: &Path,
        index: &BTreeMap<BytePath, IndexEntry>,
        conflicts: &BTreeSet<BytePath>,
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
                                } else {
                                    observe(&mut self.s.untracked, bp);
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
                match statat(&fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(after) if same_stat(&st, &after) => {}
                    Ok(_) => self
                        .disk_change(&p, "entry changed between metadata and content observation"),
                    Err(e) => self.disk_gap(&p, format!("entry revalidation unavailable: {e}")),
                }
                if let Some(entry) = tracked {
                    let expected = self.blobs.get(&entry.oid).and_then(|r| r.as_ref().ok());
                    if mode != entry.mode
                        || digest.as_ref().zip(expected).is_some_and(|(a, b)| a != b)
                    {
                        observe(&mut self.s.unstaged, &bp);
                    }
                    if digest.is_none() || expected.is_none() {
                        self.s.unstaged.complete = false;
                    }
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
    crate::residency::ensure_no_materialization()?;
    let path = w.path.to_path()?;
    let ceiling = SummaryLimits::default();
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
    Ok(SummaryOutcome {
        summary: c.s,
        first_git_timeout: c.first_git_timeout,
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
    if let Some((_, raw)) = &index_raw {
        match parse_raw_index(raw, hash_size, c.limits.max_index_entries) {
            Ok(parsed) if parsed.shared.is_none() && !parsed.intent_to_add => {
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
            Ok(parsed) => c.gap(
                &admin_path.join("index"),
                if parsed.shared.is_some() {
                    "split index refused without mutation"
                } else {
                    "intent-to-add index semantics unsupported"
                },
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
            let different = match (head_tree.get(&bp), index.get(&bp)) {
                (Some(a), Some(b)) => a.mode != b.mode || a.oid != b.oid,
                (None, None) => conflicts.contains(&bp),
                _ => true,
            };
            if different {
                observe(&mut c.s.staged, &bp);
            }
        }
    }
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
    c.disk(&root, path, &index, &conflicts, c.s.index_complete);
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
        };
        assert!(c.object(&fd, &f.0, "blob", &oid).is_err());
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
        };
        let fd = open_absolute_directory(&f.0).unwrap_or_else(|e| panic!("{e}"));
        c.disk(&fd, &f.0, &BTreeMap::new(), &BTreeSet::new(), true);
        assert_eq!(c.s.ignored.observed, 254, "{:?}", c.s);
        assert_eq!(c.s.untracked.observed, 2, "{:?}", c.s);
        assert!(!c.s.ignored.complete && !c.s.untracked.complete && !c.s.disk_complete);
        assert!(c.s.gaps.iter().any(|g| g.reason.contains("byte limit")));
        assert!(c.first_git_timeout.is_none());
    }
}
