//! Commit-only private history and repository-wide stash context.
use super::*;
use crate::local::{
    LocalHooks, NativeHooks,
    history::{Sources, Validation},
};
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::{fs, io, path::PathBuf};

#[derive(Clone, Debug)]
pub struct PrivateHistoryLimits {
    pub source_entries: usize,
    pub source_bytes: usize,
    pub file_bytes: usize,
    pub depth: usize,
    pub roots: usize,
    pub anchors: usize,
    pub pending_oids: usize,
    pub object_reads: usize,
    pub object_bytes: usize,
    pub traversal_steps: usize,
    pub per_root_steps: usize,
    pub git_processes: usize,
    pub stash_entries: usize,
    pub stash_paths: usize,
    pub message_bytes: usize,
    pub output_bytes: usize,
    pub timeout: Duration,
}
impl Default for PrivateHistoryLimits {
    fn default() -> Self {
        Self {
            source_entries: 4096,
            source_bytes: 32 * 1024 * 1024,
            file_bytes: 8 * 1024 * 1024,
            depth: 256,
            roots: 4096,
            anchors: 4096,
            pending_oids: 4096,
            object_reads: 2048,
            object_bytes: 32 * 1024 * 1024,
            traversal_steps: 65536,
            per_root_steps: 512,
            git_processes: 4096,
            stash_entries: 256,
            stash_paths: 256,
            message_bytes: 240,
            output_bytes: 4 * 1024 * 1024,
            timeout: Duration::from_secs(60),
        }
    }
}
impl PrivateHistoryLimits {
    pub fn thorough() -> Self {
        Self {
            roots: 4096,
            anchors: 4096,
            pending_oids: 65536,
            object_reads: 65536,
            object_bytes: 128 * 1024 * 1024,
            traversal_steps: 262144,
            per_root_steps: 65536,
            git_processes: 65536,
            timeout: Duration::from_secs(300),
            ..Self::default()
        }
    }
    fn validate(&self) -> Result<(), SweepError> {
        let d = Self::thorough();
        for (v, max) in [
            (self.source_entries, d.source_entries),
            (self.source_bytes, d.source_bytes),
            (self.file_bytes, d.file_bytes),
            (self.depth, d.depth),
            (self.roots, d.roots),
            (self.anchors, d.anchors),
            (self.pending_oids, d.pending_oids),
            (self.object_reads, d.object_reads),
            (self.object_bytes, d.object_bytes),
            (self.traversal_steps, d.traversal_steps),
            (self.per_root_steps, d.per_root_steps),
            (self.git_processes, d.git_processes),
            (self.stash_entries, d.stash_entries),
            (self.stash_paths, d.stash_paths),
            (self.message_bytes, d.message_bytes),
            (self.output_bytes, d.output_bytes),
        ] {
            if v == 0 || v > max {
                return Err(error("invalid private history work-unit limit"));
            }
        }
        if self.timeout.is_zero() || self.timeout > d.timeout {
            return Err(error("invalid private history timeout"));
        }
        Ok(())
    }
}
pub trait PrivateHistoryHooks: LocalHooks {
    fn before_final_validation(&self) -> io::Result<()> {
        Ok(())
    }
}
impl PrivateHistoryHooks for NativeHooks {}
pub struct PrivateHistoryCollection {
    pub histories: BTreeMap<BytePath, PrivateHistory>,
    pub shared_stashes: Vec<FamilyStashes>,
}
pub fn collect_with_limits(
    inventory: &Inventory,
    targets: &[BytePath],
    limits: &PrivateHistoryLimits,
) -> Result<PrivateHistoryCollection, SweepError> {
    collect_with(inventory, targets, limits, &NativeHooks)
}
pub fn collect_with(
    inventory: &Inventory,
    targets: &[BytePath],
    limits: &PrivateHistoryLimits,
    hooks: &dyn PrivateHistoryHooks,
) -> Result<PrivateHistoryCollection, SweepError> {
    collect_with_target_input(inventory, TargetInput::Simple(targets), limits, hooks)
}
pub fn collect_with_target_input(
    inventory: &Inventory,
    input: TargetInput<'_>,
    limits: &PrivateHistoryLimits,
    hooks: &dyn PrivateHistoryHooks,
) -> Result<PrivateHistoryCollection, SweepError> {
    limits.validate()?;
    let targets = FamilyTargets::bind_with(inventory, input, hooks)?;
    let mut run = PrivateHistoryRun::collect(inventory, &targets, limits, hooks, &BTreeMap::new())?;
    run.finish(inventory, &BTreeMap::new())?;
    Ok(run.output)
}
type FamilyId = (u64, u64);
type ObjectKey = (FamilyId, String, String);
type ObjectResult = Result<Rc<Vec<u8>>, SweepError>;
type ReflogEntry = (Option<String>, Option<String>);
#[derive(Clone)]
enum RefValue {
    Oid(String),
    Symbol(Vec<u8>),
}
struct FamilySources<'a> {
    family: usize,
    sources: Sources<'a>,
    anchor_sources: Vec<usize>,
    anchor_proofs: BTreeMap<BytePath, Vec<usize>>,
    histories: Vec<(BytePath, PathBuf, Vec<usize>)>,
    stash_sources: Vec<usize>,
}
#[derive(Clone, Debug, Default)]
pub(super) struct PrivateHistoryMetrics {
    pub(super) object_read_attempts: u64,
    pub(super) object_bytes: u64,
    pub(super) git_processes: u64,
    pub(super) object_cache_hits: u64,
    pub(super) traversal_steps: u64,
    pub(super) anchors_verified: u64,
    pub(super) roots_enumerated: u64,
    pub(super) output_serializations: u64,
}
pub(super) struct PrivateHistoryRun<'a> {
    pub(super) output: PrivateHistoryCollection,
    families: Vec<FamilySources<'a>>,
    limits: PrivateHistoryLimits,
    hooks: &'a dyn PrivateHistoryHooks,
    started: Instant,
    pub(super) stopped_families: BTreeMap<FamilyId, GitRead>,
    objects: BTreeMap<ObjectKey, ObjectResult>,
    reads: usize,
    cache_hits: usize,
    anchors_verified: usize,
    bytes: usize,
    steps: usize,
    processes: usize,
    roots: usize,
    anchors: usize,
    source_bytes: usize,
    source_output: usize,
    output_serializations: u64,
}
fn private_name(name: &[u8]) -> bool {
    [
        b"refs/bisect".as_slice(),
        b"refs/worktree",
        b"refs/rewritten",
    ]
    .iter()
    .any(|p| name == *p || name.strip_prefix(*p).is_some_and(|r| r.starts_with(b"/")))
}
fn root_name(name: &[u8]) -> bool {
    name == b"HEAD"
        || (name.ends_with(b"_HEAD") && name.iter().all(|b| b.is_ascii_uppercase() || *b == b'_'))
        || [
            b"AUTO_MERGE".as_slice(),
            b"BISECT_EXPECTED_REV",
            b"NOTES_MERGE_PARTIAL",
            b"NOTES_MERGE_REF",
            b"MERGE_AUTOSTASH",
        ]
        .contains(&name)
}
fn history_ref(name: &[u8]) -> Result<(), String> {
    if !name.starts_with(b"refs/")
        || name.ends_with(b"/")
        || name.ends_with(b".")
        || name
            .windows(2)
            .any(|w| w == b".." || w == b"@{" || w == b"//")
        || name
            .iter()
            .any(|b| *b <= b' ' || *b == 127 || b"~^:?*[\\".contains(b))
        || name
            .split(|b| *b == b'/')
            .any(|p| p.is_empty() || p.starts_with(b".") || p.ends_with(b".lock"))
    {
        Err("unsupported full history ref name".into())
    } else {
        Ok(())
    }
}
pub(super) fn retained_anchor_name(name: &[u8]) -> bool {
    history_ref(name).is_ok() && !private_name(name)
}
fn oid_bytes(bytes: &[u8]) -> Result<Option<String>, String> {
    let value = std::str::from_utf8(bytes).map_err(|_| "invalid object ID bytes")?;
    if !matches!(value.len(), 40 | 64) || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid full object ID".into());
    }
    if value.bytes().all(|b| b == b'0') {
        Ok(None)
    } else {
        Ok(Some(value.to_ascii_lowercase()))
    }
}
fn ref_value(raw: &[u8]) -> Result<RefValue, String> {
    let raw = raw.strip_suffix(b"\n").ok_or("unterminated ref source")?;
    if let Some(name) = raw.strip_prefix(b"ref: ") {
        history_ref(name)?;
        Ok(RefValue::Symbol(name.to_vec()))
    } else {
        Ok(RefValue::Oid(oid_bytes(raw)?.ok_or("zero ref object ID")?))
    }
}
fn resolve_ref(
    value: &RefValue,
    refs: &BTreeMap<Vec<u8>, RefValue>,
    max: usize,
    anchor: bool,
) -> Result<String, String> {
    let mut value = value.clone();
    let mut seen = BTreeSet::new();
    for _ in 0..max {
        match value {
            RefValue::Oid(oid) => return Ok(oid),
            RefValue::Symbol(name) => {
                if anchor && private_name(&name) {
                    return Err("shared symbolic anchor points into a private namespace".into());
                }
                if !seen.insert(name.clone()) {
                    return Err("symbolic ref cycle".into());
                }
                value = refs
                    .get(&name)
                    .cloned()
                    .ok_or("symbolic ref target absent or unsupported")?;
            }
        }
    }
    Err("symbolic ref depth budget reached".into())
}
fn pseudo_roots(raw: &[u8], name: &[u8]) -> Result<Vec<String>, String> {
    if !raw.is_empty() && !raw.ends_with(b"\n") {
        return Err("unterminated root ref".into());
    }
    raw.split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .map(|line| {
            let token = if name == b"FETCH_HEAD" {
                line.split(|b| *b == b'\t')
                    .next()
                    .ok_or("invalid FETCH_HEAD")?
            } else {
                line
            };
            oid_bytes(token)?.ok_or_else(|| "zero root ref OID".into())
        })
        .collect()
}
fn reflog(raw: &[u8]) -> Result<Vec<ReflogEntry>, String> {
    if !raw.is_empty() && !raw.ends_with(b"\n") {
        return Err("unterminated reflog".into());
    }
    let mut entries = Vec::new();
    for line in raw.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
        let mut fields = line.splitn(3, |b| *b == b' ');
        let old = oid_bytes(fields.next().ok_or("reflog old OID absent")?)?;
        let new = oid_bytes(fields.next().ok_or("reflog new OID absent")?)?;
        let rest = fields.next().ok_or("reflog metadata absent")?;
        let header = rest
            .split(|b| *b == b'\t')
            .next()
            .ok_or("reflog header absent")?;
        let tail = header.rsplit(|b| *b == b' ').take(2).collect::<Vec<_>>();
        if tail.len() != 2
            || tail[0].len() != 5
            || !matches!(tail[0][0], b'+' | b'-')
            || !tail[0][1..].iter().all(u8::is_ascii_digit)
            || std::str::from_utf8(tail[1])
                .ok()
                .and_then(|v| v.parse::<i64>().ok())
                .is_none()
        {
            return Err("invalid reflog timestamp or timezone".into());
        }
        entries.push((old, new));
    }
    Ok(entries)
}
fn partial_reflog(raw: &[u8], cap: usize) -> (Vec<ReflogEntry>, Option<String>) {
    let mut entries = Vec::new();
    let mut gap = if !raw.is_empty() && !raw.ends_with(b"\n") {
        Some("unterminated reflog".into())
    } else {
        None
    };
    for (i, line) in raw
        .split(|b| *b == b'\n')
        .filter(|l| !l.is_empty())
        .enumerate()
    {
        if i >= cap {
            gap = Some("reflog record budget reached".into());
            break;
        }
        let mut record = line.to_vec();
        record.push(b'\n');
        match reflog(&record) {
            Ok(v) => entries.extend(v),
            Err(e) => {
                gap.get_or_insert(e);
            }
        }
    }
    (entries, gap)
}
fn parse_path(raw: &[u8]) -> Result<PathBuf, SweepError> {
    let bytes = raw
        .strip_suffix(b"\n")
        .ok_or_else(|| error("Git admin path lacks final LF"))?;
    let p = BytePath::from_bytes(bytes).to_path()?;
    if !p.is_absolute() || bytes.is_empty() || bytes.contains(&0) {
        return Err(error("invalid absolute administration path"));
    }
    Ok(p)
}
impl<'a> PrivateHistoryRun<'a> {
    pub(super) fn metrics(&self) -> PrivateHistoryMetrics {
        PrivateHistoryMetrics {
            object_read_attempts: self.reads as u64,
            object_bytes: self.bytes as u64,
            git_processes: self.processes as u64,
            object_cache_hits: self.cache_hits as u64,
            traversal_steps: self.steps as u64,
            anchors_verified: self.anchors_verified as u64,
            roots_enumerated: self.roots as u64,
            output_serializations: self.output_serializations,
        }
    }
    fn id(f: &Family) -> FamilyId {
        (f.identity.device, f.identity.inode)
    }
    fn read(&mut self, f: &Family, path: &Path, op: GitRead) -> Result<Vec<u8>, SweepError> {
        if let Some(old) = self.stopped_families.get(&Self::id(f)) {
            return Err(error(family_timeout_reason(old)));
        }
        if self.started.elapsed() >= self.limits.timeout
            || self.processes >= self.limits.git_processes
        {
            return Err(error("private history Git process or time budget reached"));
        }
        let backend_cap = ControlledGit::default().timeout_limit();
        let git = ControlledGit::with_output_limit(
            self.limits.timeout.saturating_sub(self.started.elapsed()),
            self.limits
                .object_bytes
                .saturating_sub(self.bytes)
                .min(8 * 1024 * 1024),
        );
        let facet_limited = git.timeout_limit() < backend_cap;
        let result = git.read(path, op.clone());
        self.processes += git.spawned_processes() as usize;
        if result.as_ref().is_err_and(|e| e.code == "git_timeout") {
            if facet_limited && self.started.elapsed() >= self.limits.timeout {
                // A shortened phase allowance cannot establish backend failure.
                // The exhausted phase refuses its own remaining dependent reads.
                return Err(error(
                    "private history time budget exhausted during Git read; dependent private reads refused",
                ));
            }
            self.stopped_families.entry(Self::id(f)).or_insert(op);
        }
        result
    }
    fn object(&mut self, f: &Family, kind: &str, oid: &str) -> Result<Rc<Vec<u8>>, SweepError> {
        if let Some(old) = self.stopped_families.get(&Self::id(f)) {
            return Err(error(family_timeout_reason(old)));
        }
        if !valid_oid(oid) {
            return Err(error("invalid history object OID"));
        }
        let canonical = oid.to_ascii_lowercase();
        let oid = canonical.as_str();
        let key = (Self::id(f), kind.into(), oid.into());
        if let Some(raw) = self.objects.get(&key) {
            self.cache_hits += 1;
            return raw.clone();
        }
        if self.reads >= self.limits.object_reads || self.bytes >= self.limits.object_bytes {
            return Err(error("private history object or byte budget reached"));
        }
        self.reads += 1;
        let op = match kind {
            "commit" => GitRead::Commit { oid: oid.into() },
            "tree" => GitRead::Tree { oid: oid.into() },
            _ => return Err(error("unsupported history object type")),
        };
        let raw = self.read(f, &f.common_dir.to_path()?, op).and_then(|raw| {
            self.bytes += raw.len();
            verify_object(kind, oid, &raw)?;
            Ok(Rc::new(raw))
        });
        self.objects.insert(key, raw.clone());
        raw
    }
    fn commit(&mut self, f: &Family, oid: &str) -> Result<Commit, SweepError> {
        parse_commit(&self.object(f, "commit", oid)?, oid.len())
    }
    fn staged_commit(
        &mut self,
        f: &Family,
        oid: &str,
        staged: &mut BTreeMap<ObjectKey, ObjectResult>,
        session: &mut Option<(crate::git::CommitSession, Duration)>,
    ) -> Result<Commit, SweepError> {
        if let Some(old) = self.stopped_families.get(&Self::id(f)) {
            return Err(error(family_timeout_reason(old)));
        }
        if !valid_oid(oid) {
            return Err(error("invalid history object OID"));
        }
        let oid = oid.to_ascii_lowercase();
        let key = (Self::id(f), "commit".into(), oid.clone());
        if let Some(raw) = staged.get(&key).or_else(|| self.objects.get(&key)) {
            self.cache_hits += 1;
            return parse_commit(&raw.clone()?, oid.len());
        }
        if self.reads >= self.limits.object_reads || self.bytes >= self.limits.object_bytes {
            return Err(error("private history object or byte budget reached"));
        }
        if self.started.elapsed() >= self.limits.timeout {
            return Err(error("private history time budget reached"));
        }
        if session.is_none() {
            if self.processes >= self.limits.git_processes {
                return Err(error("private history Git process or time budget reached"));
            }
            let git = ControlledGit::with_output_limit(
                self.limits.timeout.saturating_sub(self.started.elapsed()),
                self.limits.object_bytes.saturating_sub(self.bytes),
            );
            let created = git.commit_session(
                &f.common_dir.to_path()?,
                32,
                self.limits
                    .object_bytes
                    .saturating_sub(self.bytes)
                    .min(128 * 1024 * 1024),
            );
            self.processes += git.spawned_processes() as usize;
            *session = Some((created?, git.timeout_limit()));
        }
        self.reads += 1;
        let raw = session
            .as_mut()
            .ok_or_else(|| error("commit session unavailable"))?
            .0
            .read(
                &oid,
                self.limits
                    .object_bytes
                    .saturating_sub(self.bytes)
                    .min(8 * 1024 * 1024),
                self.limits.timeout.saturating_sub(self.started.elapsed()),
            )
            .and_then(|raw| {
                self.bytes += raw.len();
                verify_object("commit", &oid, &raw)?;
                Ok(Rc::new(raw))
            });
        staged.insert(key, raw.clone());
        parse_commit(&raw?, oid.len())
    }
    fn admin(
        &mut self,
        f: &Family,
        cwd: &Path,
        name: &[u8],
        expected: &Path,
    ) -> Result<PathBuf, SweepError> {
        let path = parse_path(&self.read(
            f,
            cwd,
            GitRead::AdminPath {
                name: BytePath::from_bytes(name),
            },
        )?)?;
        if path != expected {
            return Err(error(
                "Git administration path disagrees with selected family/worktree",
            ));
        }
        Ok(path)
    }
    fn add_root(&mut self, h: &mut PrivateHistory, oid: String, source: &Path, kind: &str) {
        let prov = RootProvenance {
            source: BytePath::from_path(source),
            kind: kind.into(),
        };
        if let Some(root) = h.roots.iter_mut().find(|r| r.oid == oid) {
            if !root
                .provenance
                .iter()
                .any(|p| p.source == prov.source && p.kind == prov.kind)
            {
                if h.sources.len() + h.roots.iter().map(|r| r.provenance.len()).sum::<usize>()
                    >= self.limits.source_entries
                {
                    h.enumeration_complete = false;
                    h.gaps.push("root provenance budget reached".into());
                    return;
                }
                // Look up again after the bounded immutable count above.
                if let Some(root) = h.roots.iter_mut().find(|r| r.oid == oid) {
                    root.provenance.push(prov);
                }
            }
            return;
        }
        if self.roots >= self.limits.roots {
            h.enumeration_complete = false;
            if !h.gaps.iter().any(|g| g == "private root budget reached") {
                h.gaps.push("private root budget reached".into());
            }
            return;
        }
        self.roots += 1;
        h.roots.push(PrivateRoot {
            oid,
            provenance: vec![prov],
            result: "unknown".into(),
            anchor: None,
            observed_result: None,
            reason: None,
        });
    }
    fn refs_tree(
        &mut self,
        sources: &mut Sources<'a>,
        path: &Path,
        prefix: &[u8],
        depth: usize,
        refs: &mut BTreeMap<Vec<u8>, RefValue>,
        gaps: &mut Vec<String>,
    ) {
        if depth >= self.limits.depth {
            gaps.push("ref enumeration depth budget reached".into());
            return;
        }
        let names = match sources.directory(path, "ref_directory") {
            Ok((_, n)) => n,
            Err(e) => {
                gaps.push(e);
                return;
            }
        };
        for name in names {
            let child = path.join(&name);
            let mut reference = prefix.to_vec();
            reference.push(b'/');
            reference.extend_from_slice(name.as_bytes());
            let metadata = fs::symlink_metadata(&child);
            if metadata.as_ref().is_ok_and(|m| m.is_dir()) {
                self.refs_tree(sources, &child, &reference, depth + 1, refs, gaps);
            } else {
                match sources.file(&child, "ref") {
                    Ok((_, Some(raw))) => match ref_value(&raw) {
                        Ok(value) => {
                            if let Err(e) = history_ref(&reference) {
                                gaps.push(e.to_string());
                            } else {
                                refs.insert(reference, value);
                            }
                        }
                        Err(e) => gaps.push(format!(
                            "ref {}: {e}",
                            BytePath::from_bytes(&reference).display()
                        )),
                    },
                    Ok((_, None)) => {
                        gaps.push("ref disappeared after directory enumeration".into())
                    }
                    Err(e) => gaps.push(e),
                }
            }
        }
    }
    fn log_tree(
        &mut self,
        sources: &mut Sources<'a>,
        path: &Path,
        depth: usize,
        h: &mut PrivateHistory,
    ) {
        if depth >= self.limits.depth {
            h.enumeration_complete = false;
            h.gaps
                .push("reflog enumeration depth budget reached".into());
            return;
        }
        let names = match sources.directory(path, "private_reflog_directory") {
            Ok((_, n)) => n,
            Err(e) => {
                h.enumeration_complete = false;
                h.gaps.push(e);
                return;
            }
        };
        for name in names {
            let child = path.join(&name);
            if fs::symlink_metadata(&child).is_ok_and(|m| m.is_dir()) {
                self.log_tree(sources, &child, depth + 1, h);
            } else {
                self.log_file(sources, &child, h);
            }
        }
    }
    fn log_file(&mut self, sources: &mut Sources<'a>, path: &Path, h: &mut PrivateHistory) {
        match sources.file(path, "private_reflog") {
            Ok((_, Some(raw))) => {
                let (entries, gap) = partial_reflog(&raw, self.limits.source_entries);
                for (old, new) in entries {
                    for (oid, kind) in [(old, "reflog_old"), (new, "reflog_new")] {
                        if let Some(oid) = oid {
                            self.add_root(h, oid, path, kind);
                        }
                    }
                }
                if let Some(gap) = gap {
                    h.enumeration_complete = false;
                    h.gaps.push(gap);
                }
            }
            Ok((_, None)) => {}
            Err(e) => {
                h.enumeration_complete = false;
                h.gaps.push(e)
            }
        }
    }
    fn coverage(
        &mut self,
        f: &Family,
        histories: &mut [(BytePath, PrivateHistory)],
        anchors: &[RetainedAnchor],
    ) {
        let mut anchor_oids: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (index, anchor) in anchors.iter().enumerate() {
            anchor_oids
                .entry(anchor.oid.clone())
                .or_default()
                .push(index);
        }
        let mut verified_anchors = BTreeSet::new();
        let mut roots = BTreeSet::new();
        // Direct identities get first use of the object allowance across the family.
        // They need neither a complete anchor set nor a graph walk. Remaining roots
        // are verified before unrelated anchor objects are explored.
        for direct in [true, false] {
            for (_, h) in histories.iter_mut() {
                for root in &mut h.roots {
                    let indices = anchor_oids.get(&root.oid);
                    if indices.is_some() != direct {
                        continue;
                    }
                    if let Err(e) = self.commit(f, &root.oid) {
                        root.reason = Some(format!(
                            "private root is unavailable or not a verified commit: {e}"
                        ));
                        continue;
                    }
                    if let Some(indices) = indices {
                        verified_anchors.extend(indices.iter().copied());
                        root.result = "covered".into();
                        root.anchor = Some(anchors[indices[0]].clone());
                    } else {
                        roots.insert(root.oid.clone());
                    }
                }
            }
        }
        // Each verified vertex records a witness anchor. Parents are admitted only
        // from verified commits, so a partial union still supports positive paths.
        // Missing objects or a dropped frontier permanently forbid union negatives.
        // A small family-wide scheduling quantum favors paths from anchors that
        // already cover a private root. It spends the ordinary budgets and adds no
        // second graph/cache. Any unfinished priority frontier joins the full union.
        const WARM_STEPS: usize = 32;
        let mut warm = VecDeque::new();
        let mut warm_steps = 0;
        let mut queue = VecDeque::new();
        let mut scheduled = BTreeSet::new();
        let mut covered = BTreeMap::new();
        let mut incomplete = None;
        let mut invalid_anchor = false;
        for priority in [true, false] {
            for (oid, indices) in &anchor_oids {
                if verified_anchors.contains(&indices[0]) != priority {
                    continue;
                }
                if scheduled.len() >= self.limits.pending_oids {
                    incomplete = Some("private ancestry pending OID budget reached".to_string());
                    break;
                }
                scheduled.insert(oid.clone());
                if priority {
                    warm.push_back((oid.clone(), indices[0]));
                } else {
                    queue.push_back((oid.clone(), indices[0]));
                }
            }
        }
        for root in &roots {
            let mut spent = 0;
            while !covered.contains_key(root) && (!queue.is_empty() || !warm.is_empty()) {
                let mut added_covered = Vec::new();
                let mut added_anchors = Vec::new();
                let before_spent = spent;
                let mut staged = BTreeMap::new();
                let mut session = None;
                // Stage one bounded scheduler quantum; do not prefetch unrequested parents.
                for _ in 0..32 {
                    if covered.contains_key(root) || (queue.is_empty() && warm.is_empty()) {
                        break;
                    }
                    if spent >= self.limits.per_root_steps
                        || self.steps >= self.limits.traversal_steps
                        || self.started.elapsed() >= self.limits.timeout
                    {
                        break;
                    }
                    if warm_steps >= WARM_STEPS {
                        queue.append(&mut warm);
                    }
                    let priority = !warm.is_empty();
                    let next = if priority {
                        warm_steps += 1;
                        warm.pop_front()
                    } else {
                        queue.pop_front()
                    };
                    let Some((oid, witness)) = next else {
                        break;
                    };
                    spent += 1;
                    self.steps += 1;
                    match self.staged_commit(f, &oid, &mut staged, &mut session) {
                        Ok(commit) => {
                            if let Some(indices) = anchor_oids.get(&oid) {
                                for index in indices {
                                    if verified_anchors.insert(*index) {
                                        added_anchors.push(*index);
                                    }
                                }
                            }
                            added_covered.push(oid.clone());
                            covered.insert(oid, witness);
                            for parent in commit.parents {
                                if scheduled.contains(&parent) {
                                    // A parent may already be a cold anchor. Move its
                                    // existing slot, never duplicate or uncharge it.
                                    if priority
                                        && warm_steps < WARM_STEPS
                                        && let Some(index) =
                                            queue.iter().position(|(oid, _)| *oid == parent)
                                        && let Some(entry) = queue.remove(index)
                                    {
                                        warm.push_back(entry);
                                    }
                                    continue;
                                }
                                if scheduled.len() >= self.limits.pending_oids {
                                    incomplete.get_or_insert_with(|| {
                                        "private ancestry pending OID budget reached".into()
                                    });
                                    continue;
                                }
                                scheduled.insert(parent.clone());
                                if priority {
                                    warm.push_back((parent, witness));
                                } else {
                                    queue.push_back((parent, witness));
                                }
                            }
                        }
                        Err(e) => {
                            invalid_anchor |= anchor_oids.contains_key(&oid);
                            incomplete.get_or_insert_with(|| {
                                format!("retained anchor traversal incomplete or unsupported: {e}")
                            });
                        }
                    }
                }
                if let Some((session, child_timeout)) = session {
                    let remaining = self.limits.timeout.saturating_sub(self.started.elapsed());
                    let result = session.finish(remaining);
                    if let Err(e) = result {
                        let reason = if e.code == "git_timeout" {
                            if child_timeout.min(remaining)
                                < ControlledGit::default().timeout_limit()
                                && self.started.elapsed() >= self.limits.timeout
                            {
                                "private history time budget exhausted during Git read; dependent private reads refused".into()
                            } else {
                                let operation = GitRead::CommitBatch { oids: Vec::new() };
                                self.stopped_families
                                    .entry(Self::id(f))
                                    .or_insert(operation.clone());
                                family_timeout_reason(&operation)
                            }
                        } else {
                            e.to_string()
                        };
                        for oid in added_covered {
                            covered.remove(&oid);
                        }
                        for index in added_anchors {
                            verified_anchors.remove(&index);
                        }
                        staged.clear();
                        incomplete = Some(reason);
                        invalid_anchor = true;
                        queue.clear();
                        warm.clear();
                    }
                }
                self.objects.extend(staged);
                if spent == before_spent {
                    break;
                }
            }
        }
        self.anchors_verified += verified_anchors.len();
        for (_, h) in histories {
            h.anchors_complete &= !invalid_anchor;
            for root in &mut h.roots {
                if root.result == "covered" || !roots.contains(&root.oid) {
                    continue;
                }
                if let Some(witness) = covered.get(&root.oid) {
                    root.result = "covered".into();
                    root.anchor = Some(anchors[*witness].clone());
                    root.reason = None;
                } else if queue.is_empty()
                    && warm.is_empty()
                    && incomplete.is_none()
                    && h.anchors_complete
                {
                    root.result = "not_preserved_by_retained_refs".into();
                } else {
                    root.reason = Some(incomplete.clone().unwrap_or_else(|| {
                        if queue.is_empty() && warm.is_empty() {
                            "retained anchor enumeration incomplete".into()
                        } else {
                            "private ancestry traversal budget reached".into()
                        }
                    }));
                }
            }
            h.finished_unix_ms = now_ms();
        }
    }
    pub(super) fn collect(
        inventory: &Inventory,
        targets: &FamilyTargets,
        limits: &PrivateHistoryLimits,
        hooks: &'a dyn PrivateHistoryHooks,
        stopped: &BTreeMap<FamilyId, GitRead>,
    ) -> Result<Self, SweepError> {
        limits.validate()?;
        crate::residency::ensure_no_materialization()?;
        if inventory.schema != INVENTORY_SCHEMA {
            return Err(error("unsupported inventory schema"));
        }
        let mut run = Self {
            output: PrivateHistoryCollection {
                histories: BTreeMap::new(),
                shared_stashes: Vec::new(),
            },
            families: Vec::new(),
            limits: limits.clone(),
            hooks,
            started: Instant::now(),
            stopped_families: stopped.clone(),
            objects: BTreeMap::new(),
            reads: 0,
            cache_hits: 0,
            anchors_verified: 0,
            bytes: 0,
            steps: 0,
            processes: 0,
            roots: 0,
            anchors: 0,
            source_bytes: 0,
            source_output: 0,
            output_serializations: 0,
        };
        let mut identities = BTreeSet::new();
        for (index, family) in inventory.families.iter().enumerate() {
            if !identities.insert(Self::id(family)) {
                return Err(error("duplicate private history family"));
            }
            run.collect_family(index, family, targets.for_family(family))?;
        }
        run.bound_output()?;
        Ok(run)
    }
    fn collect_family(
        &mut self,
        index: usize,
        f: &Family,
        targets: &[BytePath],
    ) -> Result<(), SweepError> {
        let common = f.common_dir.to_path()?;
        let mut sources = Sources::new(
            self.hooks,
            self.limits.source_entries,
            self.limits.source_bytes.saturating_sub(self.source_bytes),
            self.limits.file_bytes,
            self.limits.output_bytes.saturating_sub(self.source_output),
            self.limits.timeout.saturating_sub(self.started.elapsed()),
        );
        let start = now_ms();
        let common_ok = fs::symlink_metadata(&common).is_ok_and(|m| {
            m.is_dir() && m.dev() == f.identity.device && m.ino() == f.identity.inode
        });
        let backend = self.read(f, &common, GitRead::RefFormat);
        let backend_ok = common_ok && backend.as_ref().is_ok_and(|b| b == b"files\n");
        let mut refs = BTreeMap::new();
        let mut loose_refs = BTreeSet::new();
        let mut anchor_gaps = Vec::new();
        let mut anchors = Vec::new();
        let mut private_packed = false;
        let mut packed_complete = false;
        if backend_ok {
            if self
                .admin(f, &common, b"refs", &common.join("refs"))
                .is_ok()
            {
                self.refs_tree(
                    &mut sources,
                    &common.join("refs"),
                    b"refs",
                    0,
                    &mut refs,
                    &mut anchor_gaps,
                );
            } else {
                anchor_gaps.push("shared refs path unverified".into());
            }
            loose_refs.extend(refs.keys().cloned());
            if self
                .admin(f, &common, b"packed-refs", &common.join("packed-refs"))
                .is_ok()
            {
                let previous_gaps = anchor_gaps.len();
                match sources.file(&common.join("packed-refs"), "packed_refs") {
                    Ok((_, Some(raw))) => {
                        if !raw.ends_with(b"\n") {
                            anchor_gaps.push("unterminated packed refs".into());
                        }
                        let mut packed = BTreeMap::new();
                        let mut seen = BTreeSet::new();
                        let mut duplicate = BTreeSet::new();
                        let mut previous = None;
                        let mut peeled = false;
                        for (count, line) in raw
                            .split(|b| *b == b'\n')
                            .filter(|l| !l.is_empty())
                            .enumerate()
                        {
                            if count >= self.limits.source_entries {
                                anchor_gaps.push("packed ref record budget reached".into());
                                break;
                            }
                            if line.starts_with(b"#") {
                                previous = None;
                                continue;
                            }
                            if let Some(value) = line.strip_prefix(b"^") {
                                let oid = oid_bytes(value).ok().flatten();
                                if peeled
                                    || previous.is_none()
                                    || oid.as_ref().is_none_or(|oid| Some(oid.len()) != previous)
                                {
                                    anchor_gaps.push(
                                        "malformed or orphan packed peeled object record".into(),
                                    );
                                }
                                peeled = true;
                                continue;
                            }
                            peeled = false;
                            previous = None;
                            let mut fields = line.splitn(2, |b| *b == b' ');
                            let oid = fields.next().and_then(|v| oid_bytes(v).ok().flatten());
                            let name = fields.next();
                            match (oid, name) {
                                (Some(oid), Some(name)) if history_ref(name).is_ok() => {
                                    previous = Some(oid.len());
                                    if private_name(name) {
                                        private_packed = true;
                                    }
                                    if !seen.insert(name.to_vec()) {
                                        packed.remove(name);
                                        duplicate.insert(name.to_vec());
                                        anchor_gaps.push("duplicate packed ref record".into());
                                    } else if !duplicate.contains(name) {
                                        packed.insert(name.to_vec(), RefValue::Oid(oid));
                                    }
                                }
                                _ => anchor_gaps.push("malformed packed ref source".into()),
                            }
                        }
                        for (name, value) in packed {
                            refs.entry(name).or_insert(value);
                        }
                    }
                    Ok((_, None)) => {}
                    Err(e) => anchor_gaps.push(e),
                }
                packed_complete = anchor_gaps.len() == previous_gaps;
            } else {
                anchor_gaps.push("packed refs path unverified".into());
            }
            for (name, value) in &refs {
                if private_name(name) {
                    continue;
                }
                match resolve_ref(value, &refs, self.limits.depth, true) {
                    Ok(oid) => {
                        if self.anchors >= self.limits.anchors {
                            anchor_gaps.push("retained anchor budget reached".into());
                            break;
                        }
                        self.anchors += 1;
                        anchors.push(RetainedAnchor {
                            reference: BytePath::from_bytes(name),
                            oid,
                        });
                    }
                    Err(e) => anchor_gaps.push(e),
                }
            }
            for target in targets {
                let name = target.bytes()?;
                if private_name(&name) || anchors.iter().any(|a| a.reference == *target) {
                    continue;
                }
                // Absence of a requested ref does not weaken enumeration of existing retained refs.
                if let Some(value) = refs.get(&name)
                    && let Ok(oid) = resolve_ref(value, &refs, self.limits.depth, true)
                    && self.anchors < self.limits.anchors
                {
                    anchors.push(RetainedAnchor {
                        reference: target.clone(),
                        oid,
                    });
                    self.anchors += 1;
                }
            }
        } else {
            anchor_gaps.push(format!(
                "files reference backend or family identity not verified: {:?}",
                backend.err()
            ));
        }
        anchors.sort_by(|a, b| a.reference.cmp(&b.reference));
        let mut anchors_complete = backend_ok && anchor_gaps.is_empty();
        let mut anchor_proofs = BTreeMap::new();
        for anchor in &anchors {
            let mut name = anchor.reference.bytes()?;
            let mut indices = Vec::new();
            let mut proof_available = true;
            for _ in 0..self.limits.depth {
                let path = common.join(std::ffi::OsStr::from_bytes(&name));
                let source = sources.records.iter().position(|s| {
                    loose_refs.contains(&name)
                        && s.path == BytePath::from_path(&path)
                        && s.exists == Some(true)
                        && s.complete
                });
                if let Some(i) = source {
                    indices.push(i);
                } else {
                    // A packed ref is current only while its loose override is provably absent.
                    // Retain that absence as its own dependency, not a broad refs manifest.
                    let first = sources.records.len();
                    match sources.file(&path, "anchor_loose_absence") {
                        Ok((i, None)) => indices.push(i),
                        Ok((i, Some(_))) => {
                            indices.push(i);
                            proof_available = false;
                            anchors_complete = false;
                            anchor_gaps.push("retained loose override was not resolved".into());
                        }
                        Err(e) => {
                            indices.extend(first..sources.records.len());
                            proof_available = false;
                            anchors_complete = false;
                            anchor_gaps.push(e);
                        }
                    }
                    if let Some(i) = sources.records.iter().position(|s| {
                        s.path == BytePath::from_path(&common.join("packed-refs"))
                            && s.exists == Some(true)
                    }) {
                        indices.push(i);
                    } else {
                        proof_available = false;
                        anchors_complete = false;
                    }
                }
                match refs.get(&name) {
                    Some(RefValue::Symbol(next)) => name = next.clone(),
                    _ => break,
                }
            }
            if !proof_available {
                indices.clear();
            }
            anchor_proofs.insert(anchor.reference.clone(), indices);
        }
        anchors.retain(|a| {
            anchor_proofs.get(&a.reference).is_some_and(|indices| {
                !indices.is_empty() && indices.iter().all(|i| sources.records[*i].complete)
            })
        });
        let anchor_sources = (0..sources.records.len()).collect::<Vec<_>>();
        let mut histories = Vec::new();
        for worktree in &f.worktrees {
            let mut h = PrivateHistory {
                git_dir: None,
                git_dir_identity: None,
                started_unix_ms: start,
                finished_unix_ms: now_ms(),
                enumeration_complete: backend_ok && packed_complete && !private_packed,
                anchors_complete,
                temporal_consistency: "not_guaranteed".into(),
                sources: Vec::new(),
                roots: Vec::new(),
                gaps: Vec::new(),
            };
            if private_packed {
                h.gaps.push(
                    "packed private refs are unsupported; private root enumeration incomplete"
                        .into(),
                );
            }
            let first = sources.records.len();
            let cwd = worktree.path.to_path()?;
            let dir = if backend_ok {
                self.read(f, &cwd, GitRead::GitDir)
                    .and_then(|raw| parse_path(&raw))
            } else {
                Err(error("private source backend not verified"))
            };
            match dir {
                Ok(dir) => {
                    h.git_dir = Some(BytePath::from_path(&dir));
                    let meta = fs::symlink_metadata(&dir);
                    let common_now = self
                        .read(f, &cwd, GitRead::CommonDir)
                        .and_then(|raw| parse_path(&raw));
                    if !meta.as_ref().is_ok_and(|m| m.is_dir())
                        || common_now.as_ref().is_err()
                        || common_now.as_ref().is_ok_and(|p| *p != common)
                    {
                        h.enumeration_complete = false;
                        h.gaps
                            .push("selected administration identity unverified".into());
                    } else {
                        if let Ok(meta) = meta {
                            h.git_dir_identity = Some(Identity {
                                device: meta.dev(),
                                inode: meta.ino(),
                            });
                        }
                        self.collect_worktree(f, &cwd, &dir, &mut sources, &refs, &mut h);
                    }
                    let indices = (first..sources.records.len()).collect::<Vec<_>>();
                    h.sources = anchor_sources
                        .iter()
                        .chain(&indices)
                        .map(|i| sources.records[*i].clone())
                        .collect();
                    h.gaps.extend(anchor_gaps.clone());
                    h.roots.sort_by(|a, b| a.oid.cmp(&b.oid));
                    h.finished_unix_ms = now_ms();
                    histories.push((worktree.path.clone(), dir, indices));
                }
                Err(e) => {
                    h.enumeration_complete = false;
                    h.gaps.push(e.to_string());
                }
            }
            self.output.histories.insert(worktree.path.clone(), h);
            self.bound_output()?;
        }
        let mut collected = f
            .worktrees
            .iter()
            .filter_map(|worktree| {
                self.output
                    .histories
                    .remove(&worktree.path)
                    .map(|h| (worktree.path.clone(), h))
            })
            .collect::<Vec<_>>();
        self.coverage(f, &mut collected, &anchors);
        for (path, history) in collected {
            self.output.histories.insert(path, history);
            self.bound_output()?;
        }
        let stash_first = sources.records.len();
        let mut stashes = self.stashes(f, &mut sources, &refs, backend_ok, start);
        if !packed_complete {
            stashes.enumeration_complete = false;
            stashes
                .gaps
                .push("shared stash packed ref enumeration incomplete".into());
        }
        let mut stash_sources = (stash_first..sources.records.len()).collect::<Vec<_>>();
        if let Some(i) = sources
            .records
            .iter()
            .position(|s| s.path == BytePath::from_path(&common.join("packed-refs")))
        {
            stash_sources.push(i);
            stashes.sources.push(sources.records[i].clone());
        }
        if let Some(dependencies) = anchor_proofs.get(&BytePath::from_bytes(b"refs/stash")) {
            for i in dependencies {
                if !stash_sources.contains(i) {
                    stash_sources.push(*i);
                    stashes.sources.push(sources.records[*i].clone());
                }
            }
        }
        self.source_bytes += sources.bytes;
        self.source_output += sources.output_charge();
        self.output.shared_stashes.push(stashes);
        self.families.push(FamilySources {
            family: index,
            sources,
            anchor_sources,
            anchor_proofs,
            histories,
            stash_sources,
        });
        Ok(())
    }
    fn collect_worktree(
        &mut self,
        f: &Family,
        cwd: &Path,
        dir: &Path,
        sources: &mut Sources<'a>,
        common_refs: &BTreeMap<Vec<u8>, RefValue>,
        h: &mut PrivateHistory,
    ) {
        let names = match sources.directory(dir, "private_administration_membership") {
            Ok((_, n)) => n,
            Err(e) => {
                h.enumeration_complete = false;
                h.gaps.push(e);
                return;
            }
        };
        let mut refs = common_refs.clone();
        let mut private_refs = BTreeMap::new();
        let mut gaps = Vec::new();
        for namespace in [
            b"refs/bisect".as_slice(),
            b"refs/worktree",
            b"refs/rewritten",
        ] {
            let relative = match std::str::from_utf8(namespace) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let path = dir.join(relative);
            if self.admin(f, cwd, namespace, &path).is_err() {
                h.enumeration_complete = false;
                h.gaps.push("private ref path unverified".into());
                continue;
            }
            self.refs_tree(sources, &path, namespace, 0, &mut private_refs, &mut gaps);
        }
        if dir == f.common_dir.to_path().unwrap_or_default() {
            for (name, value) in common_refs {
                if private_name(name) {
                    private_refs
                        .entry(name.clone())
                        .or_insert_with(|| value.clone());
                }
            }
        }
        refs.extend(private_refs.clone());
        for (name, value) in &private_refs {
            match resolve_ref(value, &refs, self.limits.depth, false) {
                Ok(oid) => {
                    let loose = dir.join(std::ffi::OsStr::from_bytes(name));
                    if sources
                        .records
                        .iter()
                        .any(|s| s.path == BytePath::from_path(&loose) && s.exists == Some(true))
                    {
                        self.add_root(h, oid, &loose, "private_ref");
                    } else {
                        self.add_root(h, oid, &dir.join("packed-refs"), "packed_private_ref");
                    }
                }
                Err(e) => gaps.push(e),
            }
        }
        for name in names {
            let b = name.as_bytes();
            if b == b"packed-refs" && dir != f.common_dir.to_path().unwrap_or_default() {
                let path = dir.join(&name);
                let _ = sources.file(&path, "unsupported_private_packed_refs");
                gaps.push("own packed private refs are unsupported".into());
                continue;
            }
            if !root_name(b) {
                continue;
            }
            let path = dir.join(&name);
            if b == b"HEAD" && self.admin(f, cwd, b"HEAD", &path).is_err() {
                gaps.push("HEAD path unverified".into());
                continue;
            }
            match sources.file(&path, "root_ref") {
                Ok((_, Some(raw))) => {
                    let result = if raw.starts_with(b"ref: ") {
                        ref_value(&raw)
                            .and_then(|v| resolve_ref(&v, &refs, self.limits.depth, false))
                            .map(|oid| vec![oid])
                    } else {
                        pseudo_roots(&raw, b)
                    };
                    match result {
                        Ok(roots) => {
                            for oid in roots {
                                if b == b"HEAD" && worktree_head_mismatch(h, &oid, f, cwd) {
                                    h.temporal_consistency = "change_observed".into();
                                    gaps.push("HEAD changed from discovery".into());
                                }
                                self.add_root(
                                    h,
                                    oid,
                                    &path,
                                    if b == b"HEAD" { "head" } else { "root_ref" },
                                );
                            }
                        }
                        Err(e) => gaps.push(e),
                    }
                }
                Ok((_, None)) => gaps.push("root ref disappeared after enumeration".into()),
                Err(e) => gaps.push(e),
            }
        }
        if !h
            .roots
            .iter()
            .any(|r| r.provenance.iter().any(|p| p.kind == "head"))
        {
            gaps.push("current HEAD not completely interpreted".into());
        }
        let own_logs = dir.join("logs");
        if self
            .admin(f, cwd, b"logs/HEAD", &own_logs.join("HEAD"))
            .is_ok()
        {
            match sources.directory(&own_logs, "own_root_reflog_membership") {
                Ok((_, names)) => {
                    for name in names {
                        if name.as_bytes() == b"refs" {
                            continue;
                        }
                        if !root_name(name.as_bytes()) {
                            gaps.push(format!(
                                "unsupported own root reflog entry: {}",
                                BytePath::from_bytes(name.as_bytes()).display()
                            ));
                        }
                        let child = own_logs.join(name);
                        if fs::symlink_metadata(&child).is_ok_and(|m| m.is_dir()) {
                            self.log_tree(sources, &child, 0, h);
                        } else {
                            self.log_file(sources, &child, h);
                        }
                    }
                }
                Err(e) => gaps.push(e),
            }
        } else {
            gaps.push("own root reflog path unverified".into());
        }
        for namespace in [
            b"logs/refs/bisect".as_slice(),
            b"logs/refs/worktree",
            b"logs/refs/rewritten",
        ] {
            if let Ok(relative) = std::str::from_utf8(namespace) {
                let path = dir.join(relative);
                if self.admin(f, cwd, namespace, &path).is_ok() {
                    self.log_tree(sources, &path, 0, h);
                } else {
                    gaps.push("private reflog path unverified".into());
                }
            }
        }
        if !gaps.is_empty() {
            h.enumeration_complete = false;
            h.gaps.extend(gaps);
        }
    }
    fn stashes(
        &mut self,
        f: &Family,
        sources: &mut Sources<'a>,
        refs: &BTreeMap<Vec<u8>, RefValue>,
        backend_ok: bool,
        start: u64,
    ) -> FamilyStashes {
        let first = sources.records.len();
        let mut result = FamilyStashes {
            common_dir: f.common_dir.clone(),
            identity: f.identity.clone(),
            started_unix_ms: start,
            finished_unix_ms: now_ms(),
            enumeration_complete: backend_ok,
            temporal_consistency: "not_guaranteed".into(),
            sources: Vec::new(),
            entries: Vec::new(),
            gaps: Vec::new(),
        };
        let common = match f.common_dir.to_path() {
            Ok(p) => p,
            Err(e) => {
                result.enumeration_complete = false;
                result.gaps.push(e.to_string());
                return result;
            }
        };
        let mut current = None;
        let mut members = Vec::new();
        if backend_ok {
            let reference = common.join("refs/stash");
            let log = common.join("logs/refs/stash");
            if self.admin(f, &common, b"refs/stash", &reference).is_ok() {
                match sources.file(&reference, "stash_ref") {
                    Ok((_, Some(raw))) => match ref_value(&raw)
                        .and_then(|v| resolve_ref(&v, refs, self.limits.depth, true))
                    {
                        Ok(oid) => current = Some(oid),
                        Err(e) => {
                            result.enumeration_complete = false;
                            result.gaps.push(e)
                        }
                    },
                    Ok((_, None)) => {
                        if let Some(value) = refs.get(b"refs/stash".as_slice()) {
                            current = resolve_ref(value, refs, self.limits.depth, true).ok();
                        }
                    }
                    Err(e) => {
                        result.enumeration_complete = false;
                        result.gaps.push(e)
                    }
                }
            } else {
                result.enumeration_complete = false;
                result.gaps.push("shared stash ref path unverified".into());
            }
            if self.admin(f, &common, b"logs/refs/stash", &log).is_ok() {
                match sources.file(&log, "stash_reflog") {
                    Ok((_, Some(raw))) => {
                        let (entries, gap) = partial_reflog(&raw, self.limits.source_entries);
                        if let Some(gap) = gap {
                            result.enumeration_complete = false;
                            result.gaps.push(gap);
                        }
                        let mut seen = BTreeSet::new();
                        for (_, new) in entries.into_iter().rev() {
                            if let Some(oid) = new
                                && seen.insert(oid.clone())
                            {
                                if members.len() >= self.limits.stash_entries {
                                    result.enumeration_complete = false;
                                    result.gaps.push("stash entry budget reached".into());
                                    break;
                                }
                                members.push(oid);
                            }
                        }
                    }
                    Ok((_, None)) => {}
                    Err(e) => {
                        result.enumeration_complete = false;
                        result.gaps.push(e)
                    }
                }
            } else {
                result.enumeration_complete = false;
                result
                    .gaps
                    .push("shared stash reflog path unverified".into());
            }
            if current.as_ref() != members.first() {
                result.enumeration_complete = false;
                result.gaps.push(
                    "shared stash ref and reflog membership disagree or reflog is absent".into(),
                );
                if let Some(oid) = current
                    && !members.contains(&oid)
                    && members.len() < self.limits.stash_entries
                {
                    members.insert(0, oid);
                }
            }
            let mut details_left = self.limits.output_bytes;
            for oid in members {
                let mut entry = self.stash_entry(f, &oid);
                let mut size = serde_json::to_vec(&entry)
                    .map(|v| v.len())
                    .unwrap_or(usize::MAX);
                if size > details_left {
                    entry.files.clear();
                    entry.files_complete = false;
                    entry
                        .gaps
                        .push("stash file detail omitted: output byte budget reached".into());
                    size = serde_json::to_vec(&entry)
                        .map(|v| v.len())
                        .unwrap_or(usize::MAX);
                }
                if size > details_left {
                    result.enumeration_complete = false;
                    result
                        .gaps
                        .push("stash entries omitted: output byte budget reached".into());
                    break;
                }
                details_left -= size;
                result.entries.push(entry);
            }
        } else {
            result.gaps.push("stash backend unverified".into());
        }
        result.sources = sources.records[first..].to_vec();
        result.finished_unix_ms = now_ms();
        result
    }
    fn stash_entry(&mut self, f: &Family, oid: &str) -> StashEntry {
        let mut entry = StashEntry {
            oid: oid.into(),
            base_oid: None,
            timestamp_unix: None,
            message: None,
            message_truncated: false,
            files: Vec::new(),
            metadata_complete: false,
            files_complete: false,
            gaps: Vec::new(),
        };
        let raw = match self.object(f, "commit", oid) {
            Ok(raw) => raw,
            Err(e) => {
                entry.gaps.push(format!("stash metadata unavailable: {e}"));
                return entry;
            }
        };
        let commit = match parse_commit(&raw, oid.len()) {
            Ok(c) => c,
            Err(e) => {
                entry.gaps.push(e.to_string());
                return entry;
            }
        };
        entry.base_oid = commit.parents.first().cloned();
        if let Ok((message, _)) = subject(&raw)
            && let Ok(bytes) = message.bytes()
        {
            entry.message = Some(BytePath::from_bytes(
                &bytes[..bytes.len().min(self.limits.message_bytes)],
            ));
        }
        let separator = raw.windows(2).position(|w| w == b"\n\n");
        if let Some(offset) = separator {
            let full = raw[offset + 2..]
                .split(|b| *b == b'\n')
                .next()
                .unwrap_or_default();
            entry.message_truncated = full.len() > self.limits.message_bytes;
        }
        for line in raw.split(|b| *b == b'\n').take_while(|l| !l.is_empty()) {
            if let Some(committer) = line.strip_prefix(b"committer ") {
                let parts = committer.rsplit(|b| *b == b' ').take(2).collect::<Vec<_>>();
                if parts.len() == 2
                    && parts[0].len() == 5
                    && matches!(parts[0][0], b'+' | b'-')
                    && parts[0][1..].iter().all(u8::is_ascii_digit)
                {
                    entry.timestamp_unix = std::str::from_utf8(parts[1])
                        .ok()
                        .and_then(|v| v.parse().ok());
                }
            }
        }
        entry.metadata_complete = entry.base_oid.is_some()
            && entry.timestamp_unix.is_some()
            && entry.message.is_some()
            && (2..=3).contains(&commit.parents.len());
        if !entry.metadata_complete {
            entry
                .gaps
                .push("stash commit metadata unsupported or incomplete".into());
            return entry;
        }
        let mut paths = BTreeSet::new();
        let files = (|| -> Result<(), SweepError> {
            let base = self.commit(f, &commit.parents[0])?;
            let index = self.commit(f, &commit.parents[1])?;
            let mut spent = 0;
            self.changed_paths(
                f,
                Some(&base.tree),
                Some(&commit.tree),
                &mut paths,
                &mut spent,
            )?;
            self.changed_paths(
                f,
                Some(&base.tree),
                Some(&index.tree),
                &mut paths,
                &mut spent,
            )?;
            if let Some(untracked) = commit.parents.get(2) {
                let untracked = self.commit(f, untracked)?;
                self.changed_paths(f, None, Some(&untracked.tree), &mut paths, &mut spent)?;
            }
            Ok(())
        })();
        entry.files = paths.into_iter().collect();
        match files {
            Ok(()) => entry.files_complete = true,
            Err(e) => entry.gaps.push(format!("stash files incomplete: {e}")),
        }
        entry
    }
    fn tree_entries(
        &mut self,
        f: &Family,
        oid: &str,
    ) -> Result<BTreeMap<Vec<u8>, (u32, String)>, SweepError> {
        let raw = self.object(f, "tree", oid)?;
        let width = oid.len() / 2;
        let mut rest = raw.as_slice();
        let mut entries = BTreeMap::new();
        while !rest.is_empty() {
            if entries.len() >= self.limits.source_entries {
                return Err(error("stash tree record budget reached"));
            }
            let end = rest
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| error("malformed stash tree"))?;
            let space = rest[..end]
                .iter()
                .position(|b| *b == b' ')
                .ok_or_else(|| error("malformed stash tree mode"))?;
            let mode = std::str::from_utf8(&rest[..space])
                .ok()
                .and_then(|v| u32::from_str_radix(v, 8).ok())
                .filter(|m| matches!(m, 0o40000 | 0o100644 | 0o100755 | 0o120000 | 0o160000))
                .ok_or_else(|| error("unsupported stash tree mode"))?;
            let name = &rest[space + 1..end];
            if name.is_empty()
                || name.contains(&b'/')
                || name == b"."
                || name == b".."
                || name == b".git"
            {
                return Err(error("unsafe stash tree path"));
            }
            let bytes = rest
                .get(end + 1..end + 1 + width)
                .ok_or_else(|| error("truncated stash tree object ID"))?;
            let child = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
            if !valid_oid(&child) || entries.insert(name.to_vec(), (mode, child)).is_some() {
                return Err(error("invalid or duplicate stash tree entry"));
            }
            rest = &rest[end + 1 + width..];
        }
        Ok(entries)
    }
    fn changed_paths(
        &mut self,
        f: &Family,
        old: Option<&str>,
        new: Option<&str>,
        paths: &mut BTreeSet<BytePath>,
        spent: &mut usize,
    ) -> Result<(), SweepError> {
        let mut queue = VecDeque::from([(
            old.map(str::to_owned),
            new.map(str::to_owned),
            Vec::new(),
            0usize,
        )]);
        while let Some((old, new, prefix, depth)) = queue.pop_front() {
            if *spent >= self.limits.per_root_steps
                || self.steps >= self.limits.traversal_steps
                || queue.len() >= self.limits.pending_oids
                || depth >= self.limits.depth
            {
                return Err(error("stash tree traversal budget reached"));
            }
            *spent += 1;
            self.steps += 1;
            let a = match old.as_deref() {
                Some(oid) => self.tree_entries(f, oid)?,
                None => BTreeMap::new(),
            };
            let b = match new.as_deref() {
                Some(oid) => self.tree_entries(f, oid)?,
                None => BTreeMap::new(),
            };
            for name in a.keys().chain(b.keys()).collect::<BTreeSet<_>>() {
                let before = a.get(name);
                let after = b.get(name);
                if before == after {
                    continue;
                }
                let mut path = prefix.clone();
                path.extend_from_slice(name);
                let old_dir = before
                    .filter(|(mode, _)| *mode == 0o40000)
                    .map(|(_, oid)| oid.clone());
                let new_dir = after
                    .filter(|(mode, _)| *mode == 0o40000)
                    .map(|(_, oid)| oid.clone());
                if before.is_some_and(|(mode, _)| *mode != 0o40000)
                    || after.is_some_and(|(mode, _)| *mode != 0o40000)
                {
                    if paths.len() >= self.limits.stash_paths
                        && !paths.contains(&BytePath::from_bytes(&path))
                    {
                        return Err(error("stash changed path budget reached"));
                    }
                    paths.insert(BytePath::from_bytes(&path));
                }
                if old_dir.is_some() || new_dir.is_some() {
                    path.push(b'/');
                    if queue.len() >= self.limits.pending_oids {
                        return Err(error("stash pending tree budget reached"));
                    }
                    queue.push_back((old_dir, new_dir, path, depth + 1));
                }
            }
        }
        Ok(())
    }
    fn bound_output(&mut self) -> Result<(), SweepError> {
        let mut left = self.limits.output_bytes.saturating_sub(16);
        let mut omitted = Vec::new();
        for (path, h) in &mut self.output.histories {
            self.output_serializations += 1;
            let mut size = serde_json::to_vec(h)
                .map_err(|e| error(e.to_string()))?
                .len();
            if size > left {
                h.enumeration_complete = false;
                h.anchors_complete = false;
                h.sources.clear();
                for root in &mut h.roots {
                    root.provenance.truncate(1);
                }
                h.gaps =
                    vec!["private history facts omitted: run output byte budget reached".into()];
                loop {
                    self.output_serializations += 1;
                    size = serde_json::to_vec(h)
                        .map_err(|e| error(e.to_string()))?
                        .len();
                    if size <= left || h.roots.is_empty() {
                        break;
                    }
                    let i = h
                        .roots
                        .iter()
                        .rposition(|r| r.result == "unknown")
                        .unwrap_or(h.roots.len() - 1);
                    h.roots.remove(i);
                }
            }
            if size > left {
                omitted.push(path.clone());
            } else {
                left -= size;
            }
        }
        for path in omitted {
            self.output.histories.remove(&path);
        }
        self.output.shared_stashes.retain_mut(|s| {
            self.output_serializations += 1;
            let mut size = serde_json::to_vec(s).map(|b| b.len()).unwrap_or(usize::MAX);
            if size > left {
                s.enumeration_complete = false;
                s.sources.clear();
                for entry in &mut s.entries {
                    entry.files.clear();
                    entry.files_complete = false;
                    entry
                        .gaps
                        .push("stash paths omitted: output byte budget reached".into());
                }
                s.gaps = vec!["stash facts omitted: run output byte budget reached".into()];
                loop {
                    self.output_serializations += 1;
                    size = serde_json::to_vec(s).map(|b| b.len()).unwrap_or(usize::MAX);
                    if size <= left || s.entries.is_empty() {
                        break;
                    }
                    s.entries.pop();
                }
            }
            if size > left {
                false
            } else {
                left -= size;
                true
            }
        });
        Ok(())
    }
    pub(super) fn finish(
        &mut self,
        inventory: &Inventory,
        stopped: &BTreeMap<FamilyId, GitRead>,
    ) -> Result<(), SweepError> {
        self.stopped_families.extend(stopped.clone());
        let hook = self.hooks.before_final_validation();
        let mut families = std::mem::take(&mut self.families);
        for state in &mut families {
            let family = &inventory.families[state.family];
            let initial_bytes = state.sources.bytes;
            state
                .sources
                .reserve_final_bytes(self.limits.source_bytes.saturating_sub(self.source_bytes));
            let failures = state.sources.validate();
            self.source_bytes += state.sources.bytes.saturating_sub(initial_bytes);
            let common = family.common_dir.to_path()?;
            let final_backend = self.read(family, &common, GitRead::RefFormat);
            let family_ok = final_backend.as_ref().is_ok_and(|raw| raw == b"files\n")
                && fs::symlink_metadata(&common).is_ok_and(|m| {
                    m.is_dir()
                        && m.dev() == family.identity.device
                        && m.ino() == family.identity.inode
                });
            let anchor_fail = selected_failure(&failures, &state.anchor_sources);
            let stash_fail = selected_failure(&failures, &state.stash_sources);
            if let Some(stash) = self
                .output
                .shared_stashes
                .iter_mut()
                .find(|s| s.identity == family.identity)
            {
                if let Some(fail) = stash_fail {
                    stash.enumeration_complete = false;
                    stash.gaps.push(fail.reason);
                    if fail.changed {
                        stash.temporal_consistency = "change_observed".into();
                    }
                }
                if !family_ok || hook.is_err() {
                    stash.enumeration_complete = false;
                    stash.gaps.push(
                        "shared stash final backend/family/source validation unavailable".into(),
                    );
                }
                stash.finished_unix_ms = now_ms();
            }
            for (path, dir, indices) in &state.histories {
                let source_fail = selected_failure(&failures, indices);
                let final_dir = if self.stopped_families.contains_key(&Self::id(family)) {
                    Err(error(
                        "family final administration check skipped after timeout",
                    ))
                } else {
                    self.read(family, &path.to_path()?, GitRead::GitDir)
                        .and_then(|raw| parse_path(&raw))
                };
                let dir_changed = final_dir.as_ref().is_ok_and(|p| p != dir)
                    || fs::symlink_metadata(dir).ok().is_some_and(|m| {
                        self.output
                            .histories
                            .get(path)
                            .and_then(|h| h.git_dir_identity.as_ref())
                            .is_some_and(|i| i.device != m.dev() || i.inode != m.ino())
                    });
                if let Some(h) = self.output.histories.get_mut(path) {
                    if let Some(fail) = &source_fail {
                        h.enumeration_complete = false;
                        h.gaps.push(fail.reason.clone());
                        if fail.changed {
                            h.temporal_consistency = "change_observed".into();
                        }
                    }
                    if let Some(fail) = &anchor_fail {
                        h.anchors_complete = false;
                        h.gaps.push(fail.reason.clone());
                        if fail.changed {
                            h.temporal_consistency = "change_observed".into();
                        }
                    }
                    if dir_changed {
                        h.enumeration_complete = false;
                        h.temporal_consistency = "change_observed".into();
                    }
                    if !family_ok {
                        h.anchors_complete = false;
                    }
                    let source_invalidating = indices.iter().any(|i| {
                        failures
                            .get(*i)
                            .and_then(Option::as_ref)
                            .is_some_and(|fail| fail.changed || state.sources.records[*i].complete)
                    });
                    let entire_invalid = source_invalidating
                        || dir_changed
                        || final_dir.is_err()
                        || hook.is_err()
                        || !family_ok;
                    for root in &mut h.roots {
                        let chosen_valid = root
                            .anchor
                            .as_ref()
                            .and_then(|a| state.anchor_proofs.get(&a.reference))
                            .is_some_and(|indices| {
                                !indices.is_empty()
                                    && selected_failure(&failures, indices).is_none()
                            });
                        if entire_invalid
                            || (anchor_fail.is_some()
                                && (root.result != "covered" || !chosen_valid))
                        {
                            invalidate_root(
                                root,
                                "private history final source/anchor/administration check incomplete or changed",
                            );
                        }
                    }
                    if entire_invalid {
                        h.enumeration_complete = false;
                        h.gaps.push("private history final source/anchor/administration check incomplete or changed".into());
                    }
                    h.finished_unix_ms = now_ms();
                }
            }
        }
        self.families = families;
        self.reconcile(inventory);
        self.bound_output()?;
        Ok(())
    }
    pub(super) fn reconcile(&mut self, inventory: &Inventory) {
        for family in &inventory.families {
            if let Some(op) = self.stopped_families.get(&Self::id(family)) {
                for worktree in &family.worktrees {
                    if let Some(h) = self.output.histories.get_mut(&worktree.path) {
                        invalidate_private(h, &family_timeout_reason(op));
                        h.finished_unix_ms = now_ms();
                    }
                }
            }
        }
    }
}
fn selected_failure(failures: &[Option<Validation>], indices: &[usize]) -> Option<Validation> {
    let mut selected = None;
    for i in indices {
        if let Some(fail) = failures.get(*i).cloned().flatten() {
            if fail.changed {
                return Some(fail);
            }
            selected.get_or_insert(fail);
        }
    }
    selected
}
fn worktree_head_mismatch(_h: &PrivateHistory, oid: &str, f: &Family, cwd: &Path) -> bool {
    f.worktrees
        .iter()
        .find(|w| w.path.to_path().is_ok_and(|p| p == cwd))
        .and_then(|w| w.head.as_deref())
        .is_some_and(|head| !oid_eq(head, oid))
}
fn invalidate_root(root: &mut PrivateRoot, reason: &str) {
    if root.result != "unknown" {
        root.observed_result = Some(root.result.clone());
        root.result = "unknown".into();
    }
    root.reason = Some(reason.into());
}
fn invalidate_private(h: &mut PrivateHistory, reason: &str) {
    for root in &mut h.roots {
        invalidate_root(root, reason);
    }
    if !h.gaps.iter().any(|g| g == reason) {
        h.gaps.push(reason.into());
    }
}

#[cfg(all(test, unix))]
#[path = "private_deadline_tests.rs"]
mod deadline_tests;

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod priority_tests {
    use super::*;
    use std::process::{Command, Stdio};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = PathBuf::from(format!(
                "/private/tmp/wt-sweep-private-priority-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            let fixture = Self(path);
            fixture.git(&["init", "--template=", "-b", "main"]);
            fs::write(fixture.0.join("file"), b"synthetic\n").unwrap();
            fixture.git(&["add", "file"]);
            fixture.git(&["commit", "-m", "root"]);
            fixture
        }
        fn git(&self, args: &[&str]) -> String {
            let mut child = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_ALLOW_PROTOCOL", "")
                .env("GIT_NO_LAZY_FETCH", "1")
                .env("GIT_AUTHOR_NAME", "Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .args(["-c", "core.hooksPath=/dev/null", "-c", "gc.auto=0", "-C"])
                .arg(&self.0)
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let started = Instant::now();
            while child.try_wait().unwrap().is_none() {
                if started.elapsed() >= Duration::from_secs(15) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("owned priority fixture Git exceeded deadline");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap().trim().into()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn private_commit_chain_preserves_roots_with_fewer_git_processes() {
        let fixture = Fixture::new();
        let root = fixture.git(&["rev-parse", "HEAD"]);
        let tree = fixture.git(&["rev-parse", "HEAD^{tree}"]);
        let mut head = root.clone();
        for index in 0..128 {
            head = fixture.git(&[
                "commit-tree",
                &tree,
                "-p",
                &head,
                "-m",
                &format!("chain {index}"),
            ]);
        }
        fixture.git(&["update-ref", "refs/heads/main", &head]);
        fs::write(fixture.0.join(".git/ORIG_HEAD"), format!("{root}\n")).unwrap();
        let inventory = crate::git::scan_family(&fixture.0).unwrap();
        let targets =
            FamilyTargets::bind_with(&inventory, TargetInput::Simple(&[]), &NativeHooks).unwrap();
        crate::git::take_child_read_journal();
        let mut run = PrivateHistoryRun::collect(
            &inventory,
            &targets,
            &PrivateHistoryLimits::default(),
            &NativeHooks,
            &BTreeMap::new(),
        )
        .unwrap();
        let children = crate::git::take_child_read_journal();
        assert_eq!(run.metrics().object_read_attempts, 129);
        assert_eq!(run.metrics().git_processes as usize, children.len());
        run.finish(&inventory, &BTreeMap::new()).unwrap();
        assert!(
            run.output
                .histories
                .values()
                .flat_map(|h| &h.roots)
                .any(|r| r.oid == root)
        );
        assert!(
            run.output
                .histories
                .values()
                .flat_map(|h| &h.roots)
                .all(|r| r.result == "covered")
        );
        assert!(
            children.len() * 3 < 129 * 2,
            "129 private commits used {} native processes",
            children.len()
        );
    }

    #[test]
    fn unchanged_bounded_private_facts_are_serialized_once_and_caps_still_refuse() {
        let fixture = Fixture::new();
        let inventory = crate::git::scan_family(&fixture.0).unwrap();
        let targets = [BytePath::from_bytes(b"refs/heads/main")];
        let bound =
            FamilyTargets::bind_with(&inventory, TargetInput::Simple(&targets), &NativeHooks)
                .unwrap();
        let mut run = PrivateHistoryRun::collect(
            &inventory,
            &bound,
            &Default::default(),
            &NativeHooks,
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(
            run.output
                .histories
                .values()
                .all(|history| history.enumeration_complete && !history.roots.is_empty()),
            "positive source evidence required"
        );
        let before = serde_json::to_value((
            run.output.histories.iter().collect::<Vec<_>>(),
            &run.output.shared_stashes,
        ))
        .unwrap();
        let expected = (run.output.histories.len() + run.output.shared_stashes.len()) as u64;
        let serializations = run.metrics().output_serializations;
        run.bound_output().unwrap();
        let after = serde_json::to_value((
            run.output.histories.iter().collect::<Vec<_>>(),
            &run.output.shared_stashes,
        ))
        .unwrap();
        assert_eq!(before, after, "budgeting changed complete under-cap facts");
        assert_eq!(
            run.metrics().output_serializations - serializations,
            expected,
            "each unchanged record needs one measured serialization per budgeting pass"
        );
        run.limits.output_bytes = 16;
        run.bound_output().unwrap();
        assert!(
            run.output.histories.is_empty() && run.output.shared_stashes.is_empty(),
            "finite output cap must still omit unavailable records"
        );
    }

    #[test]
    fn commit_session_can_cover_more_than_one_object_allowance() {
        let fixture = Fixture::new();
        let root = fixture.git(&["rev-parse", "HEAD"]);
        let tree = fixture.git(&["rev-parse", "HEAD^{tree}"]);
        let mut message = b"large synthetic commit\n\n".to_vec();
        message.resize(5 * 1024 * 1024, b'x');
        fs::write(fixture.0.join("message"), &message).unwrap();
        let middle = fixture.git(&["commit-tree", &tree, "-p", &root, "-F", "message"]);
        let next = fixture.git(&["commit-tree", &tree, "-p", &middle, "-F", "message"]);
        let head = fixture.git(&["commit-tree", &tree, "-p", &next, "-F", "message"]);
        fixture.git(&["update-ref", "refs/heads/main", &head]);
        fs::write(fixture.0.join(".git/ORIG_HEAD"), format!("{root}\n")).unwrap();
        let inventory = crate::git::scan_family(&fixture.0).unwrap();
        let targets =
            FamilyTargets::bind_with(&inventory, TargetInput::Simple(&[]), &NativeHooks).unwrap();
        let mut run = PrivateHistoryRun::collect(
            &inventory,
            &targets,
            &PrivateHistoryLimits::default(),
            &NativeHooks,
            &BTreeMap::new(),
        )
        .unwrap();
        assert!(run.bytes > 10 * 1024 * 1024);
        run.finish(&inventory, &BTreeMap::new()).unwrap();
        let history = run.output.histories.values().next().unwrap();
        assert!(
            history.roots.iter().all(|r| r.result == "covered"),
            "{:?}",
            history.roots
        );
    }
    #[test]
    fn useful_anchor_priority_counts_native_children() {
        let fixture = Fixture::new();
        let root = fixture.git(&["rev-parse", "main"]);
        let tree = fixture.git(&["rev-parse", "main^{tree}"]);
        let middle = fixture.git(&["commit-tree", &tree, "-p", &root, "-m", "middle"]);
        let head = fixture.git(&["commit-tree", &tree, "-p", &middle, "-m", "head"]);
        fixture.git(&["update-ref", "refs/heads/main", &head]);
        fs::write(fixture.0.join(".git/ORIG_HEAD"), format!("{root}\n")).unwrap();
        for n in 0..64 {
            let oid = fixture.git(&["commit-tree", &tree, "-m", &format!("independent {n}")]);
            fs::write(
                fixture.0.join(format!(".git/refs/heads/noise-{n}")),
                format!("{oid}\n"),
            )
            .unwrap();
        }
        let inventory = crate::git::scan_family(&fixture.0).unwrap();
        let targets =
            FamilyTargets::bind_with(&inventory, TargetInput::Simple(&[]), &NativeHooks).unwrap();
        crate::git::take_child_read_journal();
        let mut run = PrivateHistoryRun::collect(
            &inventory,
            &targets,
            &PrivateHistoryLimits::default(),
            &NativeHooks,
            &BTreeMap::new(),
        )
        .unwrap();
        let children = crate::git::take_child_read_journal();
        let commit_oids = children
            .iter()
            .flat_map(|(_, op)| match op {
                GitRead::Commit { oid } => vec![oid.clone()],
                GitRead::CommitBatch { oids } => oids.clone(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        // This journal is recorded at the actual child boundary, independently of
        // the private collector's counters and caches. No unrelated OID is read.
        assert_eq!(commit_oids.len(), 3, "{commit_oids:?}");
        assert_eq!(
            commit_oids.into_iter().collect::<BTreeSet<_>>(),
            BTreeSet::from([root, middle, head])
        );
        assert_eq!(run.metrics().object_read_attempts, 3);
        assert_eq!(run.metrics().traversal_steps, 3);
        assert_eq!(run.metrics().anchors_verified, 1);
        assert_eq!(run.metrics().git_processes, children.len() as u64);
        run.finish(&inventory, &BTreeMap::new()).unwrap();
        assert!(
            run.output
                .histories
                .values()
                .flat_map(|h| &h.roots)
                .all(|root| root.result == "covered")
        );
    }
}
