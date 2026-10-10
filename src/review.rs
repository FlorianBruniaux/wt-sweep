//! Local Git observations and a pure review policy. Never authorizes an action.
use crate::git::{ControlledGit, GitRead, scan_family_with_git};
use crate::model::*;
use crate::output::canonical_digest;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub mod context;
pub mod control;
#[cfg(all(test, unix))]
mod deep_batch_tests;
mod history;
pub mod private;
#[cfg(all(test, unix))]
mod summary_deadline_tests;
pub mod targets;
use targets::{FamilyTargets, TargetInput};

#[derive(Clone, Debug)]
pub struct ReviewOptions {
    /// Collect expensive evidence only for this exact recorded worktree.
    pub worktree: Option<BytePath>,
    pub evidence: ReviewLimits,
    pub budget_profile: &'static str,
    pub local: crate::local::SummaryLimits,
    pub local_timeout: Duration,
    pub private: private::PrivateHistoryLimits,
    pub controls: control::ControlLimits,
    pub context: ContextOptions,
}
impl Default for ReviewOptions {
    fn default() -> Self {
        Self {
            worktree: None,
            evidence: ReviewLimits::default(),
            budget_profile: "standard",
            local: crate::local::SummaryLimits::default(),
            local_timeout: Duration::from_secs(120),
            private: private::PrivateHistoryLimits::default(),
            controls: control::ControlLimits::default(),
            context: ContextOptions::default(),
        }
    }
}
impl ReviewOptions {
    pub fn with_budget(profile: &str) -> Result<Self, SweepError> {
        match profile {
            "standard" => Ok(Self::default()),
            "thorough" => Ok(Self {
                budget_profile: "thorough",
                evidence: ReviewLimits::thorough(),
                local: crate::local::SummaryLimits::thorough(),
                local_timeout: Duration::from_secs(900),
                private: private::PrivateHistoryLimits::thorough(),
                ..Self::default()
            }),
            _ => Err(error("budget must be standard or thorough")),
        }
    }
    fn metrics(&self) -> CollectionMetrics {
        let r = &self.evidence;
        let l = &self.local;
        let p = &self.private;
        CollectionMetrics {
            budget_profile: self.budget_profile.into(),
            phases: Vec::new(),
            applied_limits: [
                ("history_object_reads", r.object_read_attempts as u64),
                ("history_object_bytes", r.object_bytes as u64),
                ("history_traversal_steps", r.traversal_steps as u64),
                ("history_per_target_steps", r.per_target_steps as u64),
                ("history_timeout_ms", r.timeout.as_millis() as u64),
                ("deep_object_reads", r.deep_object_read_attempts as u64),
                ("deep_object_bytes", r.deep_object_bytes as u64),
                ("deep_traversal_steps", r.deep_traversal_steps as u64),
                ("deep_per_target_steps", r.deep_per_target_steps as u64),
                ("local_max_objects", l.max_objects as u64),
                ("local_max_total_bytes", l.max_total_bytes),
                ("local_worktree_timeout_ms", l.timeout.as_millis() as u64),
                (
                    "local_phase_timeout_ms",
                    self.local_timeout.as_millis() as u64,
                ),
                ("private_object_reads", p.object_reads as u64),
                ("private_object_bytes", p.object_bytes as u64),
                ("private_traversal_steps", p.traversal_steps as u64),
                ("private_per_root_steps", p.per_root_steps as u64),
                ("private_pending_oids", p.pending_oids as u64),
                ("private_timeout_ms", p.timeout.as_millis() as u64),
                ("private_git_processes", p.git_processes as u64),
            ]
            .into_iter()
            .map(|(k, v)| (k.into(), v))
            .collect(),
        }
    }
}
fn record_phase(
    metrics: &mut CollectionMetrics,
    name: &str,
    started: Instant,
    counters: impl IntoIterator<Item = (&'static str, u64)>,
) {
    metrics.phases.push(CollectionPhase {
        name: name.into(),
        elapsed_ms: started.elapsed().as_millis() as u64,
        counters: counters.into_iter().map(|(k, v)| (k.into(), v)).collect(),
    });
}
#[derive(Clone, Debug)]
pub struct ContextOptions {
    pub enabled: bool,
    pub changelog_paths: Vec<BytePath>,
    pub limits: ContextLimits,
}
impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            changelog_paths: [
                "CHANGELOG.md",
                "CHANGELOG",
                "CHANGES.md",
                "CHANGES",
                "HISTORY.md",
                "HISTORY",
            ]
            .iter()
            .map(|p| BytePath::from_bytes(p.as_bytes()))
            .collect(),
            limits: ContextLimits::default(),
        }
    }
}
#[derive(Clone, Debug)]
pub struct ContextLimits {
    pub object_read_attempts: usize,
    pub object_bytes: usize,
    pub traversal_steps: usize,
    pub per_worktree_steps: usize,
    pub object_output_bytes: usize,
    pub commit_samples: usize,
    pub changelog_paths: usize,
    pub path_bytes: usize,
    pub path_components: usize,
    pub excerpt_bytes: usize,
    pub output_bytes: usize,
    pub rows: usize,
    pub timeout: Duration,
}
impl Default for ContextLimits {
    fn default() -> Self {
        Self {
            object_read_attempts: 512,
            object_bytes: 4 * 1024 * 1024,
            traversal_steps: 16384,
            per_worktree_steps: 1024,
            object_output_bytes: 256 * 1024,
            commit_samples: 10,
            changelog_paths: 8,
            path_bytes: 4096,
            path_components: 32,
            excerpt_bytes: 4096,
            output_bytes: 1024 * 1024,
            rows: 4096,
            timeout: Duration::from_secs(10),
        }
    }
}
/// Collect decision evidence, then optional context from its recorded object IDs.
pub fn enrich_with_options(
    inventory: &Inventory,
    input: TargetInput<'_>,
    forge: Option<&ForgeExport>,
    options: &ReviewOptions,
    hooks: &dyn control::ControlHooks,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<Observations, SweepError> {
    options.context.validate()?;
    if options.local_timeout.is_zero() || options.local_timeout > Duration::from_secs(900) {
        return Err(error("invalid local phase timeout"));
    }
    if !["standard", "thorough"].contains(&options.budget_profile) {
        return Err(error("budget must be standard or thorough"));
    }
    let limits = &options.evidence;
    let control_limits = &options.controls;
    collect_with_options(
        inventory,
        input,
        forge,
        limits,
        control_limits,
        &options.context,
        options.worktree.as_ref(),
        options,
        hooks,
        callback,
    )
}

/// Runtime work-unit limits. CLI defaults are unchanged; fixture callers may inject smaller caps.
#[derive(Clone, Debug)]
pub struct ReviewLimits {
    pub object_read_attempts: usize,
    pub object_bytes: usize,
    pub traversal_steps: usize,
    pub per_target_steps: usize,
    pub slice_object_read_attempts: usize,
    pub slice_object_bytes: usize,
    pub slice_traversal_steps: usize,
    pub target_jobs: usize,
    pub timeout: Duration,
    pub deep_object_read_attempts: usize,
    pub deep_object_bytes: usize,
    pub deep_traversal_steps: usize,
    pub deep_per_target_steps: usize,
}
impl Default for ReviewLimits {
    fn default() -> Self {
        Self {
            object_read_attempts: 2048,
            object_bytes: 32 * 1024 * 1024,
            traversal_steps: 65536,
            per_target_steps: 512,
            slice_object_read_attempts: 4,
            slice_object_bytes: 64 * 1024,
            slice_traversal_steps: 16,
            target_jobs: 4096,
            timeout: Duration::from_secs(60),
            deep_object_read_attempts: 2048,
            deep_object_bytes: 32 * 1024 * 1024,
            deep_traversal_steps: 65536,
            deep_per_target_steps: 512,
        }
    }
}
impl ReviewLimits {
    pub fn thorough() -> Self {
        Self {
            object_read_attempts: 65536,
            object_bytes: 128 * 1024 * 1024,
            traversal_steps: 262144,
            per_target_steps: 65536,
            timeout: Duration::from_secs(300),
            deep_object_read_attempts: 65536,
            deep_object_bytes: 256 * 1024 * 1024,
            deep_traversal_steps: 262144,
            deep_per_target_steps: 65536,
            ..Self::default()
        }
    }
    fn validate(&self) -> Result<(), SweepError> {
        for (value, cap) in [
            (self.object_read_attempts, 65536),
            (self.object_bytes, 128 * 1024 * 1024),
            (self.traversal_steps, 262144),
            (self.per_target_steps, 65536),
            (self.slice_object_read_attempts, 2048),
            (self.slice_object_bytes, 32 * 1024 * 1024),
            (self.slice_traversal_steps, 4096),
            (self.target_jobs, 4096),
            (self.deep_object_read_attempts, 65536),
            (self.deep_object_bytes, 256 * 1024 * 1024),
            (self.deep_traversal_steps, 262144),
            (self.deep_per_target_steps, 65536),
        ] {
            if value == 0 || value > cap {
                return Err(error("invalid review work-unit limit"));
            }
        }
        if self.timeout.is_zero() || self.timeout > Duration::from_secs(300) {
            return Err(error("invalid review timeout limit"));
        }
        Ok(())
    }
}
/// Provisional collected facts for progress display; never a cleanup verdict.
#[derive(Clone, Debug)]
pub enum ProgressObservation {
    History {
        path: BytePath,
        head: Option<String>,
        target_ref: BytePath,
        target_oid: Option<String>,
        result: String,
        complete: bool,
    },
    Local {
        path: BytePath,
        summary: Box<LocalSummary>,
    },
}

#[derive(Clone, Debug)]
pub struct ProgressEvent {
    pub phase: &'static str,
    pub processed: u64,
    pub stop_reason: Option<String>,
    pub observation: Option<ProgressObservation>,
}
fn progress(
    callback: &mut dyn FnMut(ProgressEvent),
    phase: &'static str,
    processed: u64,
    stop_reason: Option<String>,
) {
    callback(ProgressEvent {
        phase,
        processed,
        stop_reason,
        observation: None,
    });
}

fn error(message: impl Into<String>) -> SweepError {
    SweepError::new("invalid_inventory", "review", Path::new(""), message)
}

fn valid_oid(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value.bytes().all(|b| b.is_ascii_hexdigit())
        && value.bytes().any(|b| b != b'0')
}

fn subject(bytes: &[u8]) -> Result<(BytePath, bool), SweepError> {
    let start = bytes
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .ok_or_else(|| error("commit message separator absent"))?
        + 2;
    let first = bytes[start..]
        .split(|byte| *byte == b'\n')
        .next()
        .unwrap_or_default();
    if first.contains(&0) {
        return Err(error(
            "commit subject contains NUL; description unavailable",
        ));
    }
    Ok((
        BytePath::from_bytes(&first[..first.len().min(240)]),
        first.len() > 240,
    ))
}

pub(crate) fn full_ref(reference: &BytePath) -> Result<(), SweepError> {
    let bytes = reference.bytes()?;
    if !(bytes.starts_with(b"refs/heads/") || bytes.starts_with(b"refs/remotes/"))
        || bytes
            .iter()
            .any(|b| *b <= b' ' || *b == 127 || b"~^:?*[\\".contains(b))
        || bytes
            .windows(2)
            .any(|p| p == b".." || p == b"@{" || p == b"//")
        || bytes.ends_with(b"/")
        || bytes.ends_with(b".")
        || bytes.ends_with(b".lock")
    {
        return Err(error(
            "integration targets must be full branch refs, without revision expressions",
        ));
    }
    Ok(())
}

fn remote_identity(url: &str) -> Option<(String, String)> {
    if url.bytes().any(|b| b <= b' ' || b == 127) {
        return None;
    }
    let (authority, path) = if let Some((scheme, rest)) = url.split_once("://") {
        if !["https", "http", "ssh", "git"].contains(&scheme) {
            return None;
        }
        rest.split_once('/')?
    } else {
        if !url.contains('@') {
            return None;
        }
        url.split_once(':')?
    };
    let host = authority.rsplit('@').next()?;
    if host.is_empty() || host.contains('[') || host.contains(']') {
        return None;
    }
    let path = path
        .trim_end_matches('/')
        .strip_suffix(".git")
        .unwrap_or(path.trim_end_matches('/'));
    if path.is_empty() {
        return None;
    }
    Some((host.to_ascii_lowercase(), path.into()))
}

fn remotes(git: &ControlledGit, common: &Path) -> Result<BTreeSet<(String, String)>, SweepError> {
    let raw = git.read(common, GitRead::RemoteUrls)?;
    let mut identities = BTreeSet::new();
    for field in raw.split(|b| *b == 0).filter(|b| !b.is_empty()) {
        let Some(split) = field.iter().position(|b| *b == b'\n') else {
            return Err(error("malformed remote identity output"));
        };
        if let Ok(url) = std::str::from_utf8(&field[split + 1..])
            && let Some(identity) = remote_identity(url)
        {
            identities.insert(identity);
        }
    }
    Ok(identities)
}

fn matching_requests(
    export: &ForgeExport,
    remotes: &BTreeSet<(String, String)>,
    worktree: &Worktree,
) -> Vec<ForgeRequest> {
    export
        .requests
        .iter()
        .filter(|request| {
            let Some(host) = request.host.as_deref() else {
                return false;
            };
            let Some(source) = request.head_oid.as_deref() else {
                return false;
            };
            if !optional_oid_eq(worktree.head.as_deref(), Some(source)) {
                return false;
            }
            if let Some(branch) = &worktree.branch {
                let Some(source_branch) = &request.source_branch else {
                    return false;
                };
                if branch.bytes().ok().as_deref()
                    != Some(format!("refs/heads/{source_branch}").as_bytes())
                {
                    return false;
                }
            }
            let mut project = request.source_project.clone();
            if request.provider == "gitlab" {
                // A GitLab numeric ID cannot identify a fork's path. Only equal known
                // source/target IDs let the target URL establish the source namespace.
                if request.source_project.is_none()
                    || request.source_project != request.target_project
                {
                    return false;
                }
                project = request
                    .url
                    .as_deref()
                    .and_then(|u| u.split_once("/-/merge_requests/"))
                    .and_then(|(base, _)| remote_identity(base))
                    .map(|(_, project)| project);
            }
            project.is_some_and(|p| remotes.contains(&(host.to_ascii_lowercase(), p)))
        })
        .cloned()
        .collect()
}

fn verify_forge(export: &ForgeExport) -> Result<(), SweepError> {
    if export.schema != FORGE_SCHEMA || export.sources.len() != 1 {
        return Err(error("unsupported forge schema or source count"));
    }
    let source = &export.sources[0];
    let raw = source.raw_path.to_path()?;
    let receipt = source
        .receipt_path
        .as_ref()
        .map(BytePath::to_path)
        .transpose()?;
    let replay = crate::forge::ingest(&raw, receipt.as_deref())?;
    if canonical_digest(&replay)? != canonical_digest(export)? {
        return Err(error(
            "forge artifact no longer matches its persisted raw evidence",
        ));
    }
    Ok(())
}

type DeepObjectKey = (u64, u64, BytePath, String, String);
struct ObjectReader {
    git: ControlledGit,
    cache: BTreeMap<DeepObjectKey, Rc<Vec<u8>>>,
    verified_trees: BTreeSet<DeepObjectKey>,
    cache_hits: u64,
    reads_left: usize,
    bytes_left: usize,
    started: Instant,
    timeout: Duration,
    steps_left: usize,
    per_target_steps: usize,
}
impl ObjectReader {
    #[cfg(test)]
    fn new() -> Self {
        Self::with_limits(&ReviewLimits::default())
    }
    fn with_limits(limits: &ReviewLimits) -> Self {
        Self {
            git: ControlledGit::default(),
            cache: BTreeMap::new(),
            verified_trees: BTreeSet::new(),
            cache_hits: 0,
            reads_left: limits.deep_object_read_attempts,
            bytes_left: limits.deep_object_bytes,
            steps_left: limits.deep_traversal_steps,
            per_target_steps: limits.deep_per_target_steps,
            timeout: limits.timeout,
            started: Instant::now(),
        }
    }
    fn key(common: &Path, kind: &str, oid: &str) -> Result<DeepObjectKey, SweepError> {
        use std::os::unix::fs::MetadataExt;
        let meta = std::fs::symlink_metadata(common).map_err(|e| error(e.to_string()))?;
        if !meta.is_dir() {
            return Err(error(
                "deep object administration is not a physical directory",
            ));
        }
        Ok((
            meta.dev(),
            meta.ino(),
            BytePath::from_path(common),
            kind.into(),
            oid.to_ascii_lowercase(),
        ))
    }
    fn blobs(&mut self, common: &Path, oids: &[String]) -> Result<(), SweepError> {
        const OUTPUT: usize = 8 * 1024 * 1024;
        for chunk in oids.chunks(128) {
            if self.expired() {
                return Err(error("whole-review time budget reached"));
            }
            let mut needed = Vec::new();
            for oid in chunk {
                if self.cache.contains_key(&Self::key(common, "blob", oid)?) {
                    self.cache_hits += 1;
                } else {
                    needed.push(oid.clone());
                }
            }
            if needed.is_empty() {
                continue;
            }
            if needed.len() > self.reads_left {
                return Err(error("whole-review object budget reached"));
            }
            let info = self
                .git
                .read(
                    common,
                    GitRead::ObjectInfo {
                        oids: needed.clone(),
                    },
                )
                .map_err(|mut e| {
                    e.message = format!(
                        "{}; requested blob batch [{}]",
                        e.message,
                        needed.join(", ")
                    );
                    e
                })?;
            let info = crate::git::parse_object_info(&info, &needed)?;
            let mut payload = Vec::new();
            let mut framed_bytes = 0usize;
            let mut raw_bytes = 0usize;
            for (oid, info) in needed.into_iter().zip(info) {
                let info = info.map_err(error)?;
                if info.kind != "blob" {
                    return Err(error("expected blob in deep verification"));
                }
                let framed = info
                    .size
                    .saturating_add(oid.len() + 6 + info.size.to_string().len() + 2);
                if framed > OUTPUT {
                    self.blob_batch(common, &payload)?;
                    payload.clear();
                    framed_bytes = 0;
                    raw_bytes = 0;
                    self.read(common, "blob", &oid)?;
                    continue;
                }
                if framed_bytes.saturating_add(framed) > OUTPUT {
                    self.blob_batch(common, &payload)?;
                    payload.clear();
                    framed_bytes = 0;
                    raw_bytes = 0;
                }
                raw_bytes = raw_bytes.saturating_add(info.size);
                if raw_bytes > self.bytes_left {
                    return Err(error("whole-review byte budget reached"));
                }
                payload.push(oid);
                framed_bytes += framed;
            }
            self.blob_batch(common, &payload)?;
        }
        Ok(())
    }
    fn blob_batch(&mut self, common: &Path, oids: &[String]) -> Result<(), SweepError> {
        if oids.is_empty() {
            return Ok(());
        }
        if self.expired() {
            return Err(error("whole-review time budget reached"));
        }
        if oids.len() > self.reads_left {
            return Err(error("whole-review object budget reached"));
        }
        self.reads_left -= oids.len();
        let raw = self
            .git
            .read(
                common,
                GitRead::Objects {
                    oids: oids.to_vec(),
                },
            )
            .map_err(|mut e| {
                e.message = format!("{}; requested blob batch [{}]", e.message, oids.join(", "));
                e
            })?;
        let objects = crate::git::parse_objects(&raw, oids)?;
        for (oid, object) in oids.iter().zip(objects) {
            let object = object.map_err(error)?;
            if object.kind != "blob" {
                return Err(error("expected blob in deep verification"));
            }
            let Some(left) = self.bytes_left.checked_sub(object.bytes.len()) else {
                self.bytes_left = 0;
                return Err(error("whole-review byte budget reached"));
            };
            self.bytes_left = left;
            verify_object("blob", oid, &object.bytes)?;
            if self.expired() {
                return Err(error("whole-review time budget reached"));
            }
            self.cache
                .insert(Self::key(common, "blob", oid)?, Rc::new(object.bytes));
        }
        Ok(())
    }
    fn expired(&self) -> bool {
        self.started.elapsed() >= self.timeout
    }
    fn exhausted(&self) -> bool {
        self.expired() || self.reads_left == 0 || self.bytes_left == 0 || self.steps_left == 0
    }
    fn step(&mut self) -> Result<(), SweepError> {
        if self.steps_left == 0 {
            return Err(error("deep traversal step budget reached"));
        }
        self.steps_left -= 1;
        Ok(())
    }
    fn read(&mut self, common: &Path, kind: &str, oid: &str) -> Result<Rc<Vec<u8>>, SweepError> {
        if self.expired() {
            return Err(error("whole-review time budget reached"));
        }
        if self.reads_left == 0 {
            return Err(error("whole-review object budget reached"));
        }
        if self.bytes_left == 0 {
            return Err(error("whole-review byte budget reached"));
        }
        if !valid_oid(oid) {
            return Err(error("invalid object OID"));
        }
        let canonical = oid.to_ascii_lowercase();
        let oid = canonical.as_str();
        let key = Self::key(common, kind, oid)?;
        if let Some(bytes) = self.cache.get(&key) {
            self.cache_hits += 1;
            return Ok(bytes.clone());
        }
        self.reads_left -= 1;
        let operation = match kind {
            "commit" => GitRead::Commit { oid: oid.into() },
            "tree" => GitRead::Tree { oid: oid.into() },
            "blob" => GitRead::Blob { oid: oid.into() },
            _ => return Err(error("unsupported object kind")),
        };
        let bytes = self.git.read(common, operation)?;
        let Some(remaining) = self.bytes_left.checked_sub(bytes.len()) else {
            self.bytes_left = 0;
            return Err(error("whole-review byte budget reached"));
        };
        self.bytes_left = remaining;
        if self.expired() {
            return Err(error("whole-review time budget reached"));
        }
        verify_object(kind, oid, &bytes)?;
        let bytes = Rc::new(bytes);
        self.cache.insert(key, bytes.clone());
        Ok(bytes)
    }
}

#[derive(Clone)]
struct Commit {
    tree: String,
    parents: Vec<String>,
}

fn oid_eq(a: &str, b: &str) -> bool {
    valid_oid(a) && valid_oid(b) && a.eq_ignore_ascii_case(b)
}
fn optional_oid_eq(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => oid_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

fn commit(reader: &mut ObjectReader, common: &Path, oid: &str) -> Result<Commit, SweepError> {
    if !valid_oid(oid) {
        return Err(error("invalid or absent commit OID"));
    }
    let bytes = reader.read(common, "commit", oid)?;
    parse_commit(&bytes, oid.len())
}
fn parse_commit(bytes: &[u8], width: usize) -> Result<Commit, SweepError> {
    parse_commit_bounded(bytes, width, usize::MAX)
}
// Structural framing only. Author/date semantics and signature validity remain
// outside graph judgement; full object identity is verified before parsing.
fn parse_commit_bounded(
    raw: &[u8],
    width: usize,
    parent_limit: usize,
) -> Result<Commit, SweepError> {
    let separator = raw
        .windows(2)
        .position(|pair| pair == b"\n\n")
        .ok_or_else(|| error("commit header separator absent"))?;
    let header = &raw[..separator];
    if header.contains(&0) {
        return Err(error("commit header contains NUL"));
    }
    let mut lines = header.split(|byte| *byte == b'\n').peekable();
    let object_id = |line: &[u8], prefix: &[u8]| -> Result<String, SweepError> {
        let value = line
            .strip_prefix(prefix)
            .ok_or_else(|| error("commit structural header syntax or order invalid"))?;
        let value = std::str::from_utf8(value).map_err(|_| error("commit header OID invalid"))?;
        if value.len() != width || !valid_oid(value) {
            return Err(error("commit object ID width or syntax invalid"));
        }
        Ok(value.to_ascii_lowercase())
    };
    let tree = object_id(lines.next().unwrap_or_default(), b"tree ")?;
    let mut parents = Vec::new();
    while lines
        .peek()
        .is_some_and(|line| line.starts_with(b"parent "))
    {
        if parents.len() >= parent_limit {
            return Err(error("commit parent fanout budget reached"));
        }
        parents.push(object_id(lines.next().unwrap_or_default(), b"parent ")?);
    }
    for prefix in [b"author ".as_slice(), b"committer "] {
        let line = lines.next().unwrap_or_default();
        if line
            .strip_prefix(prefix)
            .is_none_or(|value| value.is_empty())
        {
            return Err(error("commit structural header syntax or order invalid"));
        }
    }
    let mut extension = false;
    for line in lines {
        if line.starts_with(b" ") {
            if !extension {
                return Err(error("commit orphan header continuation"));
            }
            continue;
        }
        let split = line
            .iter()
            .position(|byte| byte.is_ascii_whitespace())
            .ok_or_else(|| error("commit extension header framing invalid"))?;
        let key = &line[..split];
        if key.is_empty()
            || line[split] != b' '
            || [b"tree".as_slice(), b"parent", b"author", b"committer"].contains(&key)
        {
            return Err(error("commit structural header syntax or order invalid"));
        }
        extension = true;
    }
    Ok(Commit { tree, parents })
}

fn resolve(git: &ControlledGit, common: &Path, reference: &BytePath) -> Result<String, SweepError> {
    let raw = git.read(
        common,
        GitRead::ResolveCommit {
            reference: reference.clone(),
        },
    )?;
    let oid = raw
        .strip_suffix(b"\n")
        .and_then(|v| std::str::from_utf8(v).ok())
        .filter(|s| valid_oid(s))
        .ok_or_else(|| error("invalid resolved target OID"))?;
    Ok(oid.to_ascii_lowercase())
}

fn local_refs(git: &ControlledGit, common: &Path) -> Result<Vec<BytePath>, SweepError> {
    let bytes = git.read(common, GitRead::LocalRefs)?;
    let mut refs = Vec::new();
    for record in bytes.split(|b| *b == b'\n').filter(|b| !b.is_empty()) {
        let fields: Vec<_> = record.split(|b| *b == 0).collect();
        if fields.len() != 3 || !fields[2].is_empty() {
            return Err(error("malformed local ref output"));
        }
        refs.push(BytePath::from_bytes(fields[0]));
    }
    Ok(refs)
}

fn relation(
    reader: &mut ObjectReader,
    common: &Path,
    source: &str,
    target: &str,
) -> Result<Option<&'static str>, SweepError> {
    let source = source.to_ascii_lowercase();
    let target = target.to_ascii_lowercase();
    let source_commit = commit(reader, common, &source)?;
    let target_commit = commit(reader, common, &target)?;
    inspect_tree(reader, common, &source_commit.tree)?;
    inspect_tree(reader, common, &target_commit.tree)?;
    if source == target {
        return Ok(Some("identical_commit"));
    }
    let mut visited = BTreeSet::new();
    let mut queue = VecDeque::from([target.to_string()]);
    while let Some(oid) = queue.pop_front() {
        if !visited.insert(oid.clone()) {
            continue;
        }
        reader.step()?;
        if visited.len() > reader.per_target_steps {
            return Err(error("commit traversal budget reached; ancestry unknown"));
        }
        if oid == source {
            return Ok(Some("ancestor"));
        }
        for parent in commit(reader, common, &oid)?.parents {
            queue.push_back(parent);
        }
    }
    if source_commit.tree == target_commit.tree {
        return Ok(Some("identical_tree"));
    }
    Ok(None)
}

fn inspect_tree(reader: &mut ObjectReader, common: &Path, oid: &str) -> Result<(), SweepError> {
    if reader.expired() {
        return Err(error("whole-review time budget reached"));
    }
    let root_key = ObjectReader::key(common, "tree", oid)?;
    if reader.verified_trees.contains(&root_key) {
        reader.cache_hits += 1;
        return Ok(());
    }
    let width = oid.len() / 2;
    let mut queue = VecDeque::from([oid.to_ascii_lowercase()]);
    let mut seen = BTreeSet::new();
    while let Some(oid) = queue.pop_front() {
        if !seen.insert(oid.clone()) {
            continue;
        }
        reader.step()?;
        if seen.len() > reader.per_target_steps {
            return Err(error("tree traversal budget reached"));
        }
        let raw = reader.read(common, "tree", &oid)?;
        let mut rest = raw.as_slice();
        let mut blobs = BTreeSet::new();
        while !rest.is_empty() {
            let end = rest
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| error("malformed tree entry"))?;
            let header = &rest[..end];
            let mode_end = header
                .iter()
                .position(|b| *b == b' ')
                .ok_or_else(|| error("malformed tree mode"))?;
            let mode = &header[..mode_end];
            if ![
                b"40000".as_slice(),
                b"100644",
                b"100755",
                b"120000",
                b"160000",
            ]
            .contains(&mode)
            {
                return Err(error("unsupported tree mode"));
            }
            if header.len() <= mode_end + 1 || rest.len() < end + 1 + width {
                return Err(error("truncated tree entry"));
            }
            let child = &rest[end + 1..end + 1 + width];
            // Tree OID identity includes each gitlink's exact mode and object ID.
            // No diff or submodule-ignore setting can hide it here.
            if mode == b"40000" {
                queue.push_back(BytePath::from_bytes(child).hex);
            } else if mode != b"160000" {
                blobs.insert(BytePath::from_bytes(child).hex);
            }
            rest = &rest[end + 1 + width..];
        }
        reader.blobs(common, &blobs.into_iter().collect::<Vec<_>>())?;
    }
    reader.verified_trees.insert(root_key);
    Ok(())
}

fn unavailable_local_summary(
    worktree: &Worktree,
    reason: String,
) -> Result<LocalSummary, SweepError> {
    let observed = now_ms();
    let mut summary = LocalSummary {
        started_unix_ms: observed,
        finished_unix_ms: observed,
        temporal_consistency: "not_guaranteed".into(),
        gaps: vec![Gap {
            path: worktree.path.clone(),
            reason,
        }],
        ..LocalSummary::default()
    };
    summary.digest_sha256 = canonical_digest(&summary)?;
    Ok(summary)
}

#[derive(Default)]
struct LocalCollection {
    metrics: BTreeMap<&'static str, u64>,
    summaries: BTreeMap<BytePath, Option<LocalSummary>>,
    stopped_families: BTreeMap<(u64, u64), GitRead>,
}

fn stops_family_history(operation: &GitRead) -> bool {
    !matches!(operation, GitRead::IgnorePaths { .. })
}

fn family_timeout_reason(operation: &GitRead) -> String {
    let operation = match operation {
        GitRead::Objects { .. } => "Objects".into(),
        GitRead::CommitBatch { .. } => "CommitBatch".into(),
        GitRead::ObjectInfo { .. } => "ObjectInfo".into(),
        GitRead::Commit { oid } => format!("Commit {oid}"),
        GitRead::Tree { oid } => format!("Tree {oid}"),
        GitRead::Blob { oid } => format!("Blob {oid}"),
        GitRead::CommonDir => "CommonDir".into(),
        GitRead::GitDir => "GitDir".into(),
        GitRead::TopLevel => "TopLevel".into(),
        GitRead::ObjectFormat => "ObjectFormat".into(),
        GitRead::RefFormat => "RefFormat".into(),
        GitRead::AdminPath { name } => format!("AdminPath {}", name.display()),
        GitRead::Worktrees => "Worktrees".into(),
        GitRead::ResolveCommit { .. } => "ResolveCommit".into(),
        GitRead::LocalRefs => "LocalRefs".into(),
        GitRead::RemoteUrls => "RemoteUrls".into(),
        GitRead::TrackingRefs => "TrackingRefs".into(),
        GitRead::IgnorePaths { .. } => "IgnorePaths".into(),
    };
    format!(
        "family collection stopped after Git timeout in {operation}; remaining dependent Git reads not performed"
    )
}

#[cfg(test)]
fn local_summaries_with_byte_budget(
    inventory: &Inventory,
    budget: usize,
) -> Result<LocalCollection, SweepError> {
    local_summaries_with_stopped_families(inventory, budget, &BTreeMap::new())
}
#[cfg(test)]
fn local_summaries_with_stopped_families(
    inventory: &Inventory,
    budget: usize,
    stopped_families: &BTreeMap<(u64, u64), GitRead>,
) -> Result<LocalCollection, SweepError> {
    local_summaries_collect(
        inventory,
        budget,
        stopped_families,
        crate::local::collect_summary_for_review,
    )
}

#[cfg(test)]
fn local_summaries_collect(
    inventory: &Inventory,
    budget: usize,
    stopped_families: &BTreeMap<(u64, u64), GitRead>,
    collect: impl FnMut(
        &Worktree,
        &crate::local::SummaryLimits,
    ) -> Result<crate::local::SummaryOutcome, SweepError>,
) -> Result<LocalCollection, SweepError> {
    local_summaries_collect_with_limits(
        inventory,
        budget,
        stopped_families,
        &crate::local::SummaryLimits::default(),
        Duration::from_secs(120),
        collect,
    )
}
#[cfg(test)]
fn local_summaries_collect_with_limits(
    inventory: &Inventory,
    budget: usize,
    stopped_families: &BTreeMap<(u64, u64), GitRead>,
    limits: &crate::local::SummaryLimits,
    phase_timeout: Duration,
    collect: impl FnMut(
        &Worktree,
        &crate::local::SummaryLimits,
    ) -> Result<crate::local::SummaryOutcome, SweepError>,
) -> Result<LocalCollection, SweepError> {
    local_summaries_collect_with_limits_and_progress(
        inventory,
        budget,
        stopped_families,
        limits,
        phase_timeout,
        collect,
        &mut |_| {},
    )
}
fn local_summaries_collect_with_limits_and_progress(
    inventory: &Inventory,
    budget: usize,
    stopped_families: &BTreeMap<(u64, u64), GitRead>,
    limits: &crate::local::SummaryLimits,
    phase_timeout: Duration,
    mut collect: impl FnMut(
        &Worktree,
        &crate::local::SummaryLimits,
    ) -> Result<crate::local::SummaryOutcome, SweepError>,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<LocalCollection, SweepError> {
    let started = Instant::now();
    let mut output = LocalCollection {
        stopped_families: stopped_families.clone(),
        ..LocalCollection::default()
    };
    let mut families = BTreeSet::new();
    let mut summary_bytes = 0usize;
    let mut pending = inventory
        .families
        .iter()
        .flat_map(|f| &f.worktrees)
        .filter(|w| !w.bare && matches!(w.path_state, DirState::Present))
        .count();
    for family in &inventory.families {
        let family_id = (family.identity.device, family.identity.inode);
        if !family.common_dir.to_path()?.is_absolute() || !families.insert(family_id) {
            return Err(error("invalid or duplicate inventory family"));
        }
        for worktree in &family.worktrees {
            let remaining_worktrees = pending.max(1);
            if !worktree.bare && matches!(worktree.path_state, DirState::Present) {
                pending = pending.saturating_sub(1);
            }
            if output.summaries.contains_key(&worktree.path) {
                return Err(error("duplicate worktree path in inventory"));
            }
            if summary_bytes >= budget {
                output.summaries.insert(worktree.path.clone(), None);
                continue;
            }
            let remaining = phase_timeout.saturating_sub(started.elapsed());
            let unavailable = if remaining.is_zero() {
                Some("whole-review local collection time budget reached")
            } else if worktree.bare || !matches!(worktree.path_state, DirState::Present) {
                Some("local summary unavailable for bare or non-present worktree")
            } else {
                None
            };
            let summary = if let Some(operation) = output.stopped_families.get(&family_id) {
                unavailable_local_summary(worktree, family_timeout_reason(operation))?
            } else if let Some(reason) = unavailable {
                unavailable_local_summary(worktree, reason.into())?
            } else {
                let mut limits = limits.clone();
                // Reserve a share for every remaining present worktree. Unused
                // time stays available to later rows; a focused retry gets its full cap.
                let shares = u32::try_from(remaining_worktrees).unwrap_or(u32::MAX);
                limits.timeout = limits.timeout.min(remaining / shares);
                match collect(worktree, &limits) {
                    Ok(outcome) => {
                        for (name, value) in [
                            ("object_reads", outcome.metrics.object_reads),
                            ("object_bytes", outcome.metrics.object_bytes),
                            ("git_processes", outcome.metrics.git_processes),
                            ("cache_hits", outcome.metrics.cache_hits),
                            ("paths_checked", outcome.metrics.paths_checked),
                        ] {
                            *output.metrics.entry(name).or_default() += value;
                        }
                        if let Some(operation) = outcome.first_git_timeout
                            && stops_family_history(&operation)
                        {
                            output.stopped_families.insert(family_id, operation);
                        }
                        outcome.summary
                    }
                    Err(error) => unavailable_local_summary(worktree, error.to_string())?,
                }
            };
            let bytes = serde_json::to_vec(&summary)
                .map_err(|e| error(e.to_string()))?
                .len();
            if bytes > budget.saturating_sub(summary_bytes) {
                output.summaries.insert(worktree.path.clone(), None);
                summary_bytes = budget;
            } else {
                summary_bytes += bytes;
                callback(ProgressEvent {
                    phase: "local_observation",
                    processed: output.summaries.len() as u64 + 1,
                    stop_reason: summary.gaps.first().map(|gap| gap.reason.clone()),
                    observation: Some(ProgressObservation::Local {
                        path: worktree.path.clone(),
                        summary: Box::new(summary.clone()),
                    }),
                });
                output
                    .summaries
                    .insert(worktree.path.clone(), Some(summary));
            }
        }
    }
    Ok(output)
}

fn local_summary_incomplete(summary: &LocalSummary) -> bool {
    !summary.head_complete
        || !summary.index_complete
        || !summary.disk_complete
        || !summary.gaps.is_empty()
        || [
            &summary.staged,
            &summary.unstaged,
            &summary.conflicts,
            &summary.untracked,
            &summary.ignored,
        ]
        .iter()
        .any(|count| !count.complete)
}

fn family_change_observed(before: &Family, after: &Family) -> bool {
    if before.identity != after.identity || before.common_dir != after.common_dir {
        return true;
    }
    let observed: BTreeMap<_, _> = after
        .worktrees
        .iter()
        .map(|tree| (&tree.path, tree))
        .collect();
    before.worktrees.iter().any(|previous| {
        let Some(current) = observed.get(&previous.path) else {
            return false;
        };
        let identities_differ = previous
            .identity
            .as_ref()
            .zip(current.identity.as_ref())
            .is_some_and(|(a, b)| a != b);
        let heads_differ = previous
            .head
            .as_ref()
            .zip(current.head.as_ref())
            .is_some_and(|(a, b)| !oid_eq(a, b));
        let branches_differ = previous
            .branch
            .as_ref()
            .zip(current.branch.as_ref())
            .is_some_and(|(a, b)| a != b);
        let presence_changed = matches!(
            (&previous.path_state, &current.path_state),
            (DirState::Present, DirState::Absent) | (DirState::Absent, DirState::Present)
        );
        identities_differ
            || heads_differ
            || branches_differ
            || presence_changed
            || previous.bare != current.bare
            || previous.detached != current.detached
    })
}

fn invalidate_local_summary(
    summary: &mut LocalSummary,
    change_observed: bool,
) -> Result<(), SweepError> {
    summary.head_complete = false;
    summary.index_complete = false;
    summary.disk_complete = false;
    if change_observed {
        summary.temporal_consistency = "change_observed".into();
    }
    for count in [
        &mut summary.staged,
        &mut summary.unstaged,
        &mut summary.conflicts,
        &mut summary.untracked,
        &mut summary.ignored,
    ] {
        count.complete = false;
    }
    summary.digest_sha256 = canonical_digest(summary)?;
    Ok(())
}

fn record_deep_timeout(
    stopped: &mut BTreeMap<(u64, u64), GitRead>,
    family: &Family,
    git: &ControlledGit,
    reader: &ObjectReader,
) -> bool {
    if let Some(operation) = git
        .first_timeout()
        .or_else(|| reader.git.first_timeout())
        .filter(stops_family_history)
    {
        stopped
            .entry((family.identity.device, family.identity.inode))
            .or_insert(operation);
        true
    } else {
        false
    }
}

pub fn enrich(
    inventory: &Inventory,
    targets: &[BytePath],
    forge: Option<&ForgeExport>,
) -> Result<Observations, SweepError> {
    enrich_with_limits(inventory, targets, forge, &ReviewLimits::default())
}
pub fn enrich_with_limits(
    inventory: &Inventory,
    targets: &[BytePath],
    forge: Option<&ForgeExport>,
    limits: &ReviewLimits,
) -> Result<Observations, SweepError> {
    enrich_with_limits_and_progress(inventory, targets, forge, limits, &mut |_| {})
}
pub fn enrich_with_limits_and_progress(
    inventory: &Inventory,
    targets: &[BytePath],
    forge: Option<&ForgeExport>,
    limits: &ReviewLimits,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<Observations, SweepError> {
    enrich_with_controls(
        inventory,
        targets,
        forge,
        limits,
        &Default::default(),
        &crate::local::NativeHooks,
        callback,
    )
}
pub fn enrich_with_controls(
    inventory: &Inventory,
    targets: &[BytePath],
    forge: Option<&ForgeExport>,
    limits: &ReviewLimits,
    control_limits: &control::ControlLimits,
    hooks: &dyn control::ControlHooks,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<Observations, SweepError> {
    enrich_with_target_input_and_controls(
        inventory,
        TargetInput::Simple(targets),
        forge,
        limits,
        control_limits,
        hooks,
        callback,
    )
}
pub fn enrich_with_target_input_and_controls(
    inventory: &Inventory,
    input: TargetInput<'_>,
    forge: Option<&ForgeExport>,
    limits: &ReviewLimits,
    control_limits: &control::ControlLimits,
    hooks: &dyn control::ControlHooks,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<Observations, SweepError> {
    enrich_with_options(
        inventory,
        input,
        forge,
        &ReviewOptions {
            worktree: None,
            evidence: limits.clone(),
            controls: control_limits.clone(),
            context: ContextOptions::default(),
            ..ReviewOptions::default()
        },
        hooks,
        callback,
    )
}
#[allow(clippy::too_many_arguments)]
fn collect_with_options(
    inventory: &Inventory,
    input: TargetInput<'_>,
    forge: Option<&ForgeExport>,
    limits: &ReviewLimits,
    control_limits: &control::ControlLimits,
    context_options: &ContextOptions,
    selected_worktree: Option<&BytePath>,
    options: &ReviewOptions,
    hooks: &dyn control::ControlHooks,
    callback: &mut dyn FnMut(ProgressEvent),
) -> Result<Observations, SweepError> {
    limits.validate()?;
    let family_targets = FamilyTargets::bind_with(inventory, input, hooks)?;
    let mut evidence_inventory = inventory.clone();
    if let Some(selected) = selected_worktree {
        if !selected.to_path()?.is_absolute()
            || inventory
                .families
                .iter()
                .flat_map(|f| &f.worktrees)
                .filter(|w| &w.path == selected)
                .count()
                != 1
        {
            return Err(error(
                "worktree must match exactly one recorded path (absolute, byte-exact)",
            ));
        }
        for family in &mut evidence_inventory.families {
            family.worktrees.retain(|w| &w.path == selected);
        }
        evidence_inventory
            .families
            .retain(|f| !f.worktrees.is_empty());
    }
    if let Some(export) = forge {
        verify_forge(export)?;
    }
    let started_unix_ms = now_ms();
    let mut collection_metrics = options.metrics();
    let phase_started = Instant::now();
    progress(callback, "controls", 0, None);
    let mut controls = control::ControlRun::collect(inventory, control_limits, hooks)?;
    record_phase(&mut collection_metrics, "controls", phase_started, []);
    let phase_started = Instant::now();
    let mut history = history::HistoryRun::collect(
        &evidence_inventory,
        &family_targets,
        limits,
        &controls.stopped_families,
        callback,
    )?;
    record_phase(
        &mut collection_metrics,
        "history",
        phase_started,
        [
            ("object_reads", history.metrics.object_read_attempts),
            ("object_bytes", history.metrics.object_bytes),
            ("traversal_steps", history.metrics.traversal_steps),
            ("cache_hits", history.metrics.cache_hits),
            ("git_processes", history.metrics.spawned_processes),
        ],
    );
    let phase_started = Instant::now();
    progress(callback, "local", 0, None);
    let LocalCollection {
        summaries: mut local_summaries,
        stopped_families,
        metrics: local_metrics,
    } = local_summaries_collect_with_limits_and_progress(
        &evidence_inventory,
        24 * 1024 * 1024,
        &history.stopped_families,
        &options.local,
        options.local_timeout,
        crate::local::collect_summary_for_review,
        callback,
    )?;
    record_phase(
        &mut collection_metrics,
        "local",
        phase_started,
        local_metrics,
    );
    history.stopped_families.extend(stopped_families.clone());
    let phase_started = Instant::now();
    progress(callback, "deep", 0, None);
    let mut reader = ObjectReader::with_limits(limits);
    let mut deep_auxiliary_processes = 0;
    let mut deep_object_processes = 0;
    let mut items = Vec::new();
    let mut identities = BTreeSet::new();
    for family in &inventory.families {
        let targets = family_targets.for_family(family);
        let common = family.common_dir.to_path()?;
        if !common.is_absolute() {
            return Err(error("common directory must be absolute"));
        }
        if !identities.insert((family.identity.device, family.identity.inode)) {
            return Err(error("duplicate inventory family"));
        }
        // Timeout telemetry is scoped to this physical family; object budgets/cache remain global.
        let git = ControlledGit::default();
        deep_object_processes += reader.git.spawned_processes();
        reader.git = ControlledGit::default();
        let family_id = (family.identity.device, family.identity.inode);
        let family_timeout = history.stopped_families.get(&family_id);
        if family_timeout.is_some() || reader.expired() {
            let reason = family_timeout
                .map(family_timeout_reason)
                .unwrap_or_else(|| {
                    "whole-review time budget reached; family not revalidated".into()
                });
            for worktree in &family.worktrees {
                items.push(ReviewItem {
                    worktree_state: controls.states.get(&worktree.path).cloned(),
                    decision: None,
                    private_history: None,
                    path: worktree.path.clone(),
                    local_summary: local_summaries.remove(&worktree.path).flatten(),
                    branch: worktree.branch.clone(),
                    head: worktree.head.clone(),
                    subject: None,
                    subject_truncated: false,
                    classification: "to_triage".into(),
                    observations: Vec::new(),
                    history: Vec::new(),
                    history_omitted_targets: 0,
                    requests: Vec::new(),
                    risks: Vec::new(),
                    unknowns: vec![reason.clone()],
                });
            }
            continue;
        }
        let current = controls
            .initial
            .get(&family_id)
            .cloned()
            .unwrap_or_else(|| Err(error("initial family control unavailable")));
        let before = current.as_ref().ok();
        let initial_change_observed =
            before.is_some_and(|observed| family_change_observed(family, observed));
        let refs = if reader.exhausted() || history.stopped_families.contains_key(&family_id) {
            Err(error("deep work budget reached; refs not collected"))
        } else {
            local_refs(&git, &common)
        };
        record_deep_timeout(&mut history.stopped_families, family, &git, &reader);
        let remote_identities = forge
            .filter(|_| !history.stopped_families.contains_key(&family_id))
            .map(|_| remotes(&git, &common));
        record_deep_timeout(&mut history.stopped_families, family, &git, &reader);
        let mut candidate_refs: BTreeSet<BytePath> = targets.iter().cloned().collect();
        if let Ok(refs) = &refs {
            candidate_refs.extend(refs.iter().cloned());
        }
        // Explicit integration targets get the budget first. Keep each group
        // ordered and all refs deduplicated by the set above.
        let (mut candidate_refs, intermediate_refs): (Vec<_>, Vec<_>) = candidate_refs
            .into_iter()
            .partition(|reference| targets.contains(reference));
        candidate_refs.extend(intermediate_refs);
        let mut paths = BTreeSet::new();
        for worktree in &family.worktrees {
            if !paths.insert(worktree.path.clone()) {
                return Err(error("duplicate worktree in inventory"));
            }
            let mut item = ReviewItem {
                worktree_state: controls.states.get(&worktree.path).cloned(),
                decision: None,
                private_history: None,
                path: worktree.path.clone(),
                local_summary: local_summaries.remove(&worktree.path).flatten(),
                branch: worktree.branch.clone(),
                head: worktree.head.clone(),
                subject: None,
                subject_truncated: false,
                classification: "to_triage".into(),
                observations: Vec::new(),
                history: Vec::new(),
                history_omitted_targets: 0,
                requests: Vec::new(),
                risks: vec![
                    "integration observations do not establish preservation of local files".into(),
                ],
                unknowns: Vec::new(),
            };
            if let Some(operation) = history.stopped_families.get(&family_id) {
                item.unknowns.push(family_timeout_reason(operation));
                items.push(item);
                continue;
            }
            let same = before
                .filter(|f| f.identity == family.identity && f.common_dir == family.common_dir)
                .and_then(|f| f.worktrees.iter().find(|w| w.path == worktree.path))
                .is_some_and(|w| {
                    w.identity == worktree.identity
                        && optional_oid_eq(w.head.as_deref(), worktree.head.as_deref())
                        && w.branch == worktree.branch
                        && w.path_state == worktree.path_state
                });
            if !same {
                if let Some(summary) = &mut item.local_summary {
                    invalidate_local_summary(summary, initial_change_observed)?;
                }
                item.unknowns.push(
                    "inventory identity or HEAD changed, or current family cannot be read".into(),
                );
                if let Err(e) = &current {
                    item.unknowns.push(e.to_string());
                }
                items.push(item);
                continue;
            }
            if selected_worktree.is_some_and(|selected| selected != &worktree.path) {
                item.unknowns
                    .push("Not selected for evidence collection (--worktree).".into());
                items.push(item);
                continue;
            }
            history.attach(&mut item);
            if let Some(export) = forge
                && let Some(Ok(remotes)) = &remote_identities
            {
                item.requests = matching_requests(export, remotes, worktree);
            }
            if inventory
                .local_inventories
                .iter()
                .any(|local| local.worktree == worktree.path)
            {
                item.unknowns.push("saved local manifest describes the scan interval; current local summary is a separate observation".into());
            }
            if !matches!(worktree.path_state, DirState::Present | DirState::Absent) {
                item.unknowns.push("worktree path unavailable".into());
                items.push(item);
                continue;
            }
            if matches!(worktree.path_state, DirState::Absent) {
                item.risks.push(
                    "registered worktree directory absent; administrative state remains".into(),
                );
            }
            let Some(source) = worktree.head.as_deref() else {
                item.unknowns.push("source HEAD unknown".into());
                items.push(item);
                continue;
            };
            let source_bytes = reader.read(&common, "commit", source);
            record_deep_timeout(&mut history.stopped_families, family, &git, &reader);
            let source_available = source_bytes.is_ok();
            match source_bytes.and_then(|raw| subject(&raw)) {
                Ok((subject, truncated)) => {
                    item.subject = Some(subject);
                    item.subject_truncated = truncated;
                }
                Err(e) => item.unknowns.push(format!("description: {e}")),
            }
            // Unknown C4 is still possible here. It remains mandatory at final judgement.
            if !c1(&item, family)
                || !c3(&item)
                || !keep_reasons(&item, family, started_unix_ms, now_ms()).is_empty()
                || targets.is_empty()
            {
                item.unknowns.push(
                    "deep verification not required for an already retained or incomplete worktree"
                        .into(),
                );
                if forge.is_none() {
                    item.unknowns.push("forge state unknown (no export)".into());
                }
                items.push(item);
                continue;
            }
            if targets.is_empty() {
                item.unknowns
                    .push("no explicitly designated integration target".into());
            }
            if let Err(e) = &refs {
                item.unknowns.push(e.to_string());
            }
            for (index, reference) in candidate_refs.iter().enumerate() {
                if history.stopped_families.contains_key(&family_id) {
                    break;
                }
                // Every relation requires the source commit. A failed source
                // read already makes it unknown; repeated target reads cannot
                // repair that observation and can multiply a blocked-read delay.
                // A malformed subject alone does not prevent Git judgement.
                if !source_available {
                    item.unknowns
                        .push("integration unknown: source commit unavailable".into());
                    break;
                }
                if index >= 128 || reader.exhausted() {
                    item.unknowns.push("target count or whole-review time budget reached; remaining targets unknown".into());
                    break;
                }
                if worktree.branch.as_ref() == Some(reference) && !targets.contains(reference) {
                    continue;
                }
                let observed_start = now_ms();
                let result = (|| {
                    let target = resolve(&git, &common, reference)?;
                    let relation = relation(&mut reader, &common, source, &target)?;
                    let again = resolve(&git, &common, reference)?;
                    if target != again {
                        return Err(error("target changed during observation"));
                    }
                    Ok((target, relation))
                })();
                record_deep_timeout(&mut history.stopped_families, family, &git, &reader);
                match result {
                    Ok((target, Some(relation))) => {
                        item.observations.push(IntegrationObservation {
                            source_oid: source.into(),
                            target_ref: reference.clone(),
                            target_oid: target,
                            relation: relation.into(),
                            target_role: if targets.contains(reference) {
                                "integration"
                            } else {
                                "intermediate"
                            }
                            .into(),
                            started_unix_ms: observed_start,
                            finished_unix_ms: now_ms(),
                        })
                    }
                    Ok((_, None)) => {}
                    Err(e) => item.unknowns.push(format!("{}: {e}", reference.display())),
                }
            }
            if let Some(export) = forge {
                if let Some(Ok(remotes)) = &remote_identities {
                    item.requests = matching_requests(export, remotes, worktree);
                }
                if item.requests.is_empty() {
                    item.unknowns.push(
                        "no scoped forge association established; request absence unknown".into(),
                    );
                }
                item.unknowns.push("export observations do not establish current server state or absence of requests".into());
            } else {
                item.unknowns.push("forge state unknown (no export)".into());
            }
            items.push(item);
        }
        deep_auxiliary_processes += git.spawned_processes();
    }
    for item in &mut items {
        if selected_worktree.is_some_and(|selected| selected != &item.path) {
            let omitted = "Not selected for evidence collection (--worktree).";
            if !item.unknowns.iter().any(|reason| reason == omitted) {
                item.unknowns.push(omitted.into());
            }
            continue;
        }
        if item.local_summary.is_none() {
            item.unknowns.push(
                "current local summary omitted: whole-review local summary byte budget reached"
                    .into(),
            );
        }
        if item
            .local_summary
            .as_ref()
            .is_none_or(local_summary_incomplete)
        {
            item.unknowns
                .push("current local summary incomplete; inspect local counts and gaps".into());
        }
    }
    record_phase(
        &mut collection_metrics,
        "deep",
        phase_started,
        [
            (
                "object_reads",
                (limits.deep_object_read_attempts - reader.reads_left) as u64,
            ),
            (
                "object_bytes",
                (limits.deep_object_bytes - reader.bytes_left) as u64,
            ),
            (
                "traversal_steps",
                (limits.deep_traversal_steps - reader.steps_left) as u64,
            ),
            (
                "git_processes",
                reader.git.spawned_processes() + deep_object_processes + deep_auxiliary_processes,
            ),
            ("cache_hits", reader.cache_hits),
        ],
    );
    let phase_started = Instant::now();
    progress(callback, "private_history", 0, None);
    let mut private = private::PrivateHistoryRun::collect(
        &evidence_inventory,
        &family_targets,
        &options.private,
        &crate::local::NativeHooks,
        &history.stopped_families,
    )?;
    let private_metrics = private.metrics();
    record_phase(
        &mut collection_metrics,
        "private_history",
        phase_started,
        [
            ("object_reads", private_metrics.object_read_attempts),
            ("object_bytes", private_metrics.object_bytes),
            ("git_processes", private_metrics.git_processes),
            ("cache_hits", private_metrics.object_cache_hits),
            ("traversal_steps", private_metrics.traversal_steps),
            ("anchors_verified", private_metrics.anchors_verified),
            ("roots_enumerated", private_metrics.roots_enumerated),
            (
                "output_serializations",
                private_metrics.output_serializations,
            ),
        ],
    );
    let phase_started = Instant::now();
    let history_processes_before_final = history.metrics.spawned_processes;
    history
        .stopped_families
        .extend(private.stopped_families.clone());
    progress(
        callback,
        "history_revalidate",
        history.metrics.first_pass_items,
        history.metrics.stop_reason.clone(),
    );
    controls.finish(inventory, &history.stopped_families)?;
    history
        .stopped_families
        .extend(controls.stopped_families.clone());
    for item in &mut items {
        item.worktree_state = controls.states.get(&item.path).cloned();
        if item
            .worktree_state
            .as_ref()
            .is_some_and(|state| state.checks == "change_observed")
            && let Some(summary) = &mut item.local_summary
        {
            invalidate_local_summary(summary, true)?;
        }
    }
    history.finish(&evidence_inventory, &mut items, &controls.final_families)?;
    progress(callback, "private_revalidate", items.len() as u64, None);
    private.finish(&evidence_inventory, &history.stopped_families)?;
    history
        .stopped_families
        .extend(private.stopped_families.clone());
    history.reconcile_stopped(&evidence_inventory, &mut items);
    for item in &mut items {
        item.private_history = private.output.histories.remove(&item.path);
        if item.private_history.is_none()
            && selected_worktree.is_none_or(|selected| selected == &item.path)
        {
            item.unknowns
                .push("private history not collected: output byte budget reached".into());
        }
    }
    for item in &mut items {
        if let Some(family) = inventory
            .families
            .iter()
            .find(|f| f.worktrees.iter().any(|w| w.path == item.path))
            && history
                .stopped_families
                .contains_key(&(family.identity.device, family.identity.inode))
            && let Some(state) = &mut item.worktree_state
        {
            if state.checks != "change_observed" {
                state.checks = "incomplete".into();
            }
            state
                .gaps
                .push("required final validation quarantined after family Git timeout".into());
        }
    }
    control::bound_item_states(&mut items, control_limits.output_bytes)?;
    let private_after = private.metrics();
    record_phase(
        &mut collection_metrics,
        "revalidation",
        phase_started,
        [
            (
                "private_output_serializations",
                private_after
                    .output_serializations
                    .saturating_sub(private_metrics.output_serializations),
            ),
            (
                "history_git_processes",
                history
                    .metrics
                    .spawned_processes
                    .saturating_sub(history_processes_before_final),
            ),
            (
                "private_git_processes",
                private_after
                    .git_processes
                    .saturating_sub(private_metrics.git_processes),
            ),
        ],
    );
    let evidence_finished_unix_ms = now_ms();
    let phase_started = Instant::now();
    let context = if context_options.enabled {
        progress(callback, "context", items.len() as u64, None);
        let context_items: Vec<_> = items
            .iter()
            .filter(|item| selected_worktree.is_none_or(|selected| selected == &item.path))
            .cloned()
            .collect();
        Some(context::collect(
            &evidence_inventory,
            &context_items,
            &family_targets,
            &history,
            &reader,
            context_options,
            hooks,
        )?)
    } else {
        None
    };
    if context_options.enabled {
        record_phase(&mut collection_metrics, "context", phase_started, []);
    }
    let shared_stashes = private.output.shared_stashes;

    let history_metrics = history.metrics;
    progress(
        callback,
        "complete",
        items.len() as u64,
        history_metrics.stop_reason.clone(),
    );
    items.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(Observations {
        context,
        shared_stashes,
        inventory: inventory.clone(),
        history_metrics,
        collection_metrics: Some(collection_metrics),
        items,
        forge: forge.cloned(),
        started_unix_ms,
        finished_unix_ms: evidence_finished_unix_ms,
    })
}

fn worktree_matches(a: &Worktree, b: &Worktree) -> bool {
    a.path == b.path
        && a.identity == b.identity
        && optional_oid_eq(a.head.as_deref(), b.head.as_deref())
        && a.branch == b.branch
        && a.detached == b.detached
        && a.bare == b.bare
        && a.locked == b.locked
        && a.prunable == b.prunable
        && a.path_state == b.path_state
}
fn state_bound(item: &ReviewItem, family: &Family) -> bool {
    item.worktree_state.as_ref().is_some_and(|s| {
        s.common_dir == family.common_dir
            && s.family_identity.as_ref() == Some(&family.identity)
            && s.observed.as_ref().is_some_and(|w| {
                w.path == item.path
                    && optional_oid_eq(w.head.as_deref(), item.head.as_deref())
                    && family.worktrees.iter().any(|old| {
                        w.path == old.path
                            && w.identity == old.identity
                            && optional_oid_eq(w.head.as_deref(), old.head.as_deref())
                            && w.branch == old.branch
                            && w.path_state == old.path_state
                    })
            })
    })
}
fn c1(item: &ReviewItem, family: &Family) -> bool {
    state_bound(item, family)
        && item.worktree_state.as_ref().is_some_and(|s| {
            s.role == "linked"
                && s.checks == "complete"
                && s.protection_complete
                && s.operations_complete
                && s.protections.is_empty()
                && s.operations.is_empty()
                && s.git_dir.is_some()
                && s.git_dir_identity.is_some()
                && s.observed.as_ref().is_some_and(|w| {
                    matches!(w.path_state, DirState::Present)
                        && w.identity.is_some()
                        && w.head.as_deref().is_some_and(valid_oid)
                        && w.locked.is_none()
                })
        })
}
fn deep_witness(item: &ReviewItem, o: &IntegrationObservation) -> bool {
    o.target_role == "integration"
        && matches!(o.relation.as_str(), "identical_commit" | "ancestor")
        && optional_oid_eq(Some(&o.source_oid), item.head.as_deref())
        && valid_oid(&o.source_oid)
        && valid_oid(&o.target_oid)
        && (o.relation != "identical_commit" || oid_eq(&o.source_oid, &o.target_oid))
        && full_ref(&o.target_ref).is_ok()
}
fn c2(item: &ReviewItem) -> bool {
    item.observations.iter().any(|o| deep_witness(item, o))
}
fn local_bound(item: &ReviewItem) -> bool {
    item.worktree_state
        .as_ref()
        .zip(item.local_summary.as_ref())
        .is_some_and(|(s, l)| {
            s.observed.as_ref().is_some_and(|w| {
                l.worktree_identity.is_some()
                    && l.worktree_identity == w.identity
                    && optional_oid_eq(l.head.as_deref(), w.head.as_deref())
                    && optional_oid_eq(l.head.as_deref(), item.head.as_deref())
            }) && l.git_dir.is_some()
                && l.git_dir == s.git_dir
        })
}
fn c3(item: &ReviewItem) -> bool {
    local_bound(item)
        && item.local_summary.as_ref().is_some_and(|s| {
            !local_summary_incomplete(s)
                && s.temporal_consistency != "change_observed"
                && [
                    &s.staged,
                    &s.unstaged,
                    &s.conflicts,
                    &s.untracked,
                    &s.ignored,
                ]
                .iter()
                .all(|n| n.observed == 0)
        })
}
fn private_bound(item: &ReviewItem) -> bool {
    item.worktree_state
        .as_ref()
        .zip(item.private_history.as_ref())
        .is_some_and(|(s, h)| {
            h.git_dir.is_some()
                && h.git_dir == s.git_dir
                && h.git_dir_identity.is_some()
                && h.git_dir_identity == s.git_dir_identity
        })
}
fn c4(item: &ReviewItem) -> bool {
    private_bound(item)
        && item.private_history.as_ref().is_some_and(|h| {
            h.enumeration_complete
                && h.temporal_consistency != "change_observed"
                && !h.roots.is_empty()
                && h.roots
                    .iter()
                    .any(|r| optional_oid_eq(Some(&r.oid), item.head.as_deref()))
                && h.roots.iter().all(|r| {
                    r.result == "covered"
                        && r.anchor.as_ref().is_some_and(|a| {
                            valid_oid(&a.oid)
                                && a.reference
                                    .bytes()
                                    .is_ok_and(|name| private::retained_anchor_name(&name))
                        })
                })
        })
}
fn interval(start: u64, end: u64, facts: &Observations) -> bool {
    facts.started_unix_ms <= start && start <= end && end <= facts.finished_unix_ms
}
fn c5(item: &ReviewItem, facts: &Observations) -> bool {
    let Some(state) = &item.worktree_state else {
        return false;
    };
    let family = facts
        .inventory
        .families
        .iter()
        .find(|f| f.worktrees.iter().any(|w| w.path == item.path));
    state.checks == "complete"
        && family.is_some_and(|f| state_bound(item, f))
        && interval(state.started_unix_ms, state.finished_unix_ms, facts)
        && local_bound(item)
        && item.local_summary.as_ref().is_some_and(|l| {
            interval(l.started_unix_ms, l.finished_unix_ms, facts)
                && l.temporal_consistency != "change_observed"
        })
        && private_bound(item)
        && item.private_history.as_ref().is_some_and(|h| {
            interval(h.started_unix_ms, h.finished_unix_ms, facts)
                && h.enumeration_complete
                && h.temporal_consistency != "change_observed"
        })
        && item.observations.iter().any(|o| {
            deep_witness(item, o) && interval(o.started_unix_ms, o.finished_unix_ms, facts)
        })
}
fn local_keep_reasons(item: &ReviewItem) -> Vec<String> {
    let mut reasons = Vec::new();
    if let Some(s) = &item.local_summary {
        for (code, count) in [
            ("local_staged", &s.staged),
            ("local_unstaged", &s.unstaged),
            ("local_conflicts", &s.conflicts),
            ("local_untracked", &s.untracked),
            ("local_ignored", &s.ignored),
        ] {
            if count.observed > 0 {
                reasons.push(code.into());
            }
        }
    }
    reasons
}
fn keep_reasons(item: &ReviewItem, family: &Family, started: u64, finished: u64) -> Vec<String> {
    let mut reasons = Vec::new();
    if let Some(s) = &item.worktree_state {
        match s.role.as_str() {
            "main" => reasons.push("main_checkout".into()),
            "bare" => reasons.push("bare_repository".into()),
            _ => {}
        }
        if s.observed.as_ref().is_some_and(|w| w.locked.is_some()) {
            reasons.push("git_locked".into());
        }
        if !s.protections.is_empty() {
            reasons.push("administrative_lock".into());
        }
        if !s.operations.is_empty() {
            reasons.push("git_operation".into());
        }
    }
    reasons.extend(local_keep_reasons(item));
    if state_bound(item, family)
        && item.requests.iter().any(|r| {
            matches!(r.state.as_str(), "open" | "opened")
                && optional_oid_eq(r.head_oid.as_deref(), item.head.as_deref())
        })
    {
        reasons.push("open_request".into());
    }
    if state_bound(item, family)
        && item.history_omitted_targets == 0
        && !item.history.is_empty()
        && item.history.iter().all(|h| {
            started <= h.started_unix_ms
                && h.started_unix_ms <= h.finished_unix_ms
                && h.finished_unix_ms <= finished
                && h.result == "not_ancestor"
                && optional_oid_eq(h.source_oid.as_deref(), item.head.as_deref())
                && h.target_oid.as_deref().is_some_and(valid_oid)
                && h.stop_reason.is_none()
        })
    {
        reasons.push("head_not_integrated".into());
    }
    if private_bound(item)
        && item.private_history.as_ref().is_some_and(|h| {
            h.roots
                .iter()
                .any(|r| r.result == "not_preserved_by_retained_refs")
        })
    {
        reasons.push("private_history_not_preserved".into());
    }
    reasons
}

pub fn judge(facts: &Observations) -> Result<Review, SweepError> {
    let mut items = facts.items.clone();
    for item in &mut items {
        let family = facts
            .inventory
            .families
            .iter()
            .find(|f| f.worktrees.iter().any(|w| w.path == item.path));
        let reasons = family
            .map(|f| keep_reasons(item, f, facts.started_unix_ms, facts.finished_unix_ms))
            .unwrap_or_else(|| local_keep_reasons(item));
        let conditions = [
            family.is_some_and(|f| c1(item, f)),
            c2(item),
            c3(item),
            c4(item),
            c5(item, facts),
        ];
        let missing_conditions = conditions
            .iter()
            .enumerate()
            .filter(|(_, valid)| !**valid)
            .map(|(index, _)| format!("C{}", index + 1))
            .collect::<Vec<_>>();
        item.classification = if !reasons.is_empty() {
            "keep"
        } else if missing_conditions.is_empty() {
            "cleanup_candidate"
        } else {
            "verify"
        }
        .into();
        item.decision = Some(TriageDecision {
            keep_reasons: reasons,
            missing_conditions,
        });
        if !item
            .unknowns
            .iter()
            .any(|u| u == "usage UNKNOWN; confirm usage before any removal")
        {
            item.unknowns
                .push("usage UNKNOWN; confirm usage before any removal".into());
        }
        item.risks.push("no removal authorized; ACLs, xattrs and unreachable objects outside enumerated roots are outside preservation scope".into());
    }
    let mut review = Review {
        disk_usage: facts.inventory.disk_usage.clone(),
        context: facts.context.clone(),
        shared_stashes: facts.shared_stashes.clone(),
        history_metrics: facts.history_metrics.clone(),
        collection_metrics: facts.collection_metrics.clone(),
        schema: REVIEW_SCHEMA.into(),
        policy_version: "triage/1".into(),
        inventory_sha256: canonical_digest(&facts.inventory)?,
        digest_sha256: String::new(),
        started_unix_ms: facts.started_unix_ms,
        finished_unix_ms: facts.finished_unix_ms,
        coverage: facts.inventory.coverage.clone(),
        items,
        forge: facts.forge.clone(),
        not_collected: facts.inventory.not_collected.clone(),
        local_inventory_refs: facts
            .inventory
            .local_inventories
            .iter()
            .map(|local| LocalInventoryRef {
                worktree: local.worktree.clone(),
                started_unix_ms: local.started_unix_ms,
                finished_unix_ms: local.finished_unix_ms,
                content_digest_sha256: local.content_digest_sha256.clone(),
                traversal_complete: local.traversal_complete,
                temporal_consistency: local.temporal_consistency.clone(),
                entry_count: local.entries.len(),
                index_entry_count: local.index_entries.len(),
                administration_entry_count: local.administration_entries.len(),
                gap_count: local.gaps.len(),
                not_collected_count: local.not_collected.len(),
            })
            .collect(),
    };
    review.digest_sha256 = canonical_digest(&review)?;
    Ok(review)
}

pub(crate) fn verify_object(kind: &str, oid: &str, bytes: &[u8]) -> Result<(), SweepError> {
    use sha2::Digest;
    let header = format!("{kind} {}\0", bytes.len());
    let computed = match oid.len() {
        40 => {
            let mut hash = sha1_checked::Sha1::new();
            hash.update(header.as_bytes());
            hash.update(bytes);
            let result = hash.try_finalize();
            if result.has_collision() {
                return Err(error("SHA-1 collision detected in Git object"));
            }
            format!("{:x}", result.hash())
        }
        64 => {
            let mut hash = sha2::Sha256::new();
            hash.update(header.as_bytes());
            hash.update(bytes);
            format!("{:x}", hash.finalize())
        }
        _ => return Err(error("unsupported object ID length")),
    };
    if !computed.eq_ignore_ascii_case(oid) {
        return Err(error("Git object bytes do not match their OID"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_git_object_bytes_must_match_their_full_oid() {
        let empty_blob = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
        assert!(verify_object("blob", empty_blob, b"").is_ok());
        assert!(verify_object("blob", empty_blob, b"forged bytes").is_err());
    }

    #[test]
    fn exhausted_review_budgets_refuse_before_trying_a_repository_read() {
        let path = Path::new("/fixture-does-not-exist");
        let oid = "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391";
        for (resource, message) in [
            ("objects", "whole-review object budget reached"),
            ("bytes", "whole-review byte budget reached"),
            ("time", "whole-review time budget reached"),
        ] {
            let mut reader = ObjectReader::new();
            match resource {
                "objects" => reader.reads_left = 0,
                "bytes" => reader.bytes_left = 0,
                _ => reader.started = Instant::now() - Duration::from_secs(61),
            }
            let result = reader.read(path, "blob", oid);
            assert!(
                matches!(result, Err(e) if e.message == message),
                "{resource}"
            );
        }
    }
    #[test]
    fn only_history_dependent_timeouts_stop_family_reads() {
        assert!(!stops_family_history(&GitRead::IgnorePaths {
            paths: vec![BytePath::from_bytes(b"a\nb")],
        }));
        let oid = "a".repeat(40);
        let operation = GitRead::Blob { oid: oid.clone() };
        assert!(stops_family_history(&operation));
        assert!(stops_family_history(&GitRead::GitDir));
        let reason = family_timeout_reason(&operation);
        assert!(reason.contains(&oid));
        assert!(reason.contains("remaining dependent Git reads not performed"));
    }

    #[test]
    fn family_metadata_failure_is_not_a_positive_change_observation() -> Result<(), SweepError> {
        let before = Family {
            common_dir: BytePath::from_bytes(b"/fixture/.git"),
            identity: Identity {
                device: 1,
                inode: 10,
            },
            worktrees: vec![Worktree {
                path: BytePath::from_bytes(b"/fixture/linked"),
                identity: Some(Identity {
                    device: 1,
                    inode: 20,
                }),
                head: Some("a".repeat(40)),
                branch: Some(BytePath::from_bytes(b"refs/heads/topic")),
                detached: false,
                bare: false,
                locked: None,
                prunable: None,
                path_state: DirState::Present,
            }],
        };
        let mut after = before.clone();
        for state in [
            DirState::ReadError {
                reason: "EIO".into(),
            },
            DirState::Inaccessible {
                reason: "EACCES".into(),
            },
            DirState::VolumeUnavailable {
                reason: "unmounted".into(),
            },
            DirState::Unknown {
                reason: "not examined".into(),
            },
        ] {
            after.worktrees[0].identity = None;
            after.worktrees[0].path_state = state;
            let mut summary = LocalSummary {
                temporal_consistency: "not_guaranteed".into(),
                ..Default::default()
            };
            invalidate_local_summary(&mut summary, family_change_observed(&before, &after))?;
            assert_eq!(summary.temporal_consistency, "not_guaranteed");
        }
        after = before.clone();
        after.worktrees[0].identity = Some(Identity {
            device: 1,
            inode: 30,
        });
        assert!(family_change_observed(&before, &after));
        after = before.clone();
        after.worktrees[0].head = Some("b".repeat(40));
        assert!(family_change_observed(&before, &after));
        after = before.clone();
        after.worktrees[0].path_state = DirState::Absent;
        after.worktrees[0].identity = None;
        assert!(family_change_observed(&before, &after));
        assert!(!family_change_observed(&before, &before));
        Ok(())
    }

    #[test]
    fn local_revalidation_failure_is_unknown_not_observed_drift() -> Result<(), SweepError> {
        let mut summary = LocalSummary {
            temporal_consistency: "not_guaranteed".into(),
            staged: LocalCount {
                observed: 2,
                complete: true,
                ..Default::default()
            },
            ..Default::default()
        };
        invalidate_local_summary(&mut summary, false)?;
        assert_eq!(summary.temporal_consistency, "not_guaranteed");
        assert_eq!(summary.staged.observed, 2);
        assert!(!summary.staged.complete);
        invalidate_local_summary(&mut summary, true)?;
        assert_eq!(summary.temporal_consistency, "change_observed");
        invalidate_local_summary(&mut summary, false)?;
        assert_eq!(summary.temporal_consistency, "change_observed");
        Ok(())
    }

    #[test]
    fn aggregate_local_summary_budget_counts_unavailable_placeholders()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut inventory = Inventory::new(ScanScope::Roots { paths: Vec::new() });
        inventory.families.push(Family {
            common_dir: BytePath::from_bytes(b"/synthetic/bare"),
            identity: Identity {
                device: 1,
                inode: 2,
            },
            worktrees: (0..5)
                .map(|n| Worktree {
                    path: BytePath::from_bytes(format!("/synthetic/bare/{n}").as_bytes()),
                    identity: None,
                    head: None,
                    branch: None,
                    detached: false,
                    bare: true,
                    locked: None,
                    prunable: None,
                    path_state: DirState::Present,
                })
                .collect(),
        });
        let summaries = local_summaries_with_byte_budget(&inventory, 32)?.summaries;
        let bytes: usize = summaries
            .values()
            .filter_map(Option::as_ref)
            .map(|s| serde_json::to_vec(s).map(|bytes| bytes.len()))
            .sum::<Result<usize, _>>()?;
        assert!(bytes <= 32, "fallback summaries exceeded budget: {bytes}");
        assert_eq!(summaries.len(), 5, "omitted rows must remain represented");
        assert!(summaries.values().all(Option::is_none));
        let sufficient = local_summaries_with_byte_budget(&inventory, 8192)?.summaries;
        assert!(
            sufficient.values().all(Option::is_some),
            "budget logic always omitted"
        );
        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod deep_timeout_regression {
    use super::*;
    use crate::git::scan_family;
    use std::{
        fs,
        path::{Path, PathBuf},
        process::{Command, Stdio},
        sync::atomic::{AtomicU64, Ordering},
        time::{Duration, Instant},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = PathBuf::from(format!(
                "/private/tmp/wt-sweep-private-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            let f = Self(p);
            f.git(&["init", "--template=", "-b", "main"]);
            fs::write(f.0.join("file"), "fixture\n").unwrap();
            f.git(&["add", "file"]);
            f.git(&["commit", "-m", "fixture"]);
            f
        }
        fn at(path: &Path, args: &[&str]) -> String {
            let mut child = Command::new("/usr/bin/git")
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("TMPDIR", "/private/tmp")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_ALLOW_PROTOCOL", "")
                .env("GIT_NO_LAZY_FETCH", "1")
                .env("GIT_AUTHOR_NAME", "Fixture")
                .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
                .env("GIT_COMMITTER_NAME", "Fixture")
                .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
                .args([
                    "-c",
                    "core.hooksPath=/dev/null",
                    "-c",
                    "core.fsmonitor=false",
                    "-c",
                    "gc.auto=0",
                    "-c",
                    "maintenance.auto=false",
                    "-C",
                ])
                .arg(path)
                .args(args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap();
            let start = Instant::now();
            while child.try_wait().unwrap().is_none() {
                if start.elapsed() > Duration::from_secs(15) {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("owned fixture Git exceeded deadline");
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let out = child.wait_with_output().unwrap();
            assert!(
                out.status.success(),
                "{args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8(out.stdout)
                .unwrap()
                .trim_end_matches('\n')
                .into()
        }
        fn git(&self, args: &[&str]) -> String {
            Self::at(&self.0, args)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn make_fifo(path: &Path) {
        let mut child = Command::new("/usr/bin/mkfifo")
            .env_clear()
            .arg(path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            if start.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned FIFO fixture child exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[test]
    fn deep_timeout_has_no_later_blocked_family_children() {
        let blocked = Fixture::new();
        let linked = blocked.0.join("linked");
        blocked.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
        let healthy = Fixture::new();
        let base = blocked.git(&["rev-parse", "HEAD"]);
        let tree = blocked.git(&["rev-parse", "HEAD^{tree}"]);
        let exclusive = blocked.git(&[
            "commit-tree",
            &tree,
            "-p",
            &base,
            "-m",
            "blocked intermediate",
        ]);
        blocked.git(&["update-ref", "refs/heads/zz_blocked", &exclusive]);
        let mut inventory = scan_family(&blocked.0).unwrap();
        inventory
            .families
            .extend(scan_family(&healthy.0).unwrap().families);
        let path = blocked
            .0
            .join(".git/objects")
            .join(&exclusive[..2])
            .join(&exclusive[2..]);
        let raw = fs::read(&path).unwrap();
        fs::remove_file(&path).unwrap();
        make_fifo(&path);
        crate::git::take_child_read_journal();
        let mut after_deep = Vec::new();
        let mut restored = false;
        let target_map = TargetMap {
            schema: TARGET_MAP_SCHEMA.into(),
            families: inventory
                .families
                .iter()
                .map(|family| TargetMapFamily {
                    common_dir: family.common_dir.clone(),
                    identity: family.identity.clone(),
                    targets: vec![BytePath::from_bytes(b"refs/heads/main")],
                })
                .collect(),
        };
        let facts = enrich_with_target_input_and_controls(
            &inventory,
            targets::TargetInput::Map(&target_map),
            None,
            &Default::default(),
            &Default::default(),
            &crate::local::NativeHooks,
            &mut |event| {
                if event.phase == "private_history" {
                    after_deep = crate::git::take_child_read_journal();
                    fs::remove_file(&path).unwrap();
                    fs::write(&path, &raw).unwrap();
                    restored = true;
                }
            },
        )
        .unwrap();
        assert!(restored);
        let later = crate::git::take_child_read_journal();
        let blocked_reads = after_deep
            .iter()
            .filter(|(path, _)| path.to_path().unwrap().starts_with(&blocked.0))
            .collect::<Vec<_>>();
        let (last_path, last_operation) = blocked_reads.last().unwrap();
        assert_eq!(*last_path, BytePath::from_path(&blocked.0.join(".git")));
        assert!(
            matches!(last_operation, GitRead::ResolveCommit { reference } if *reference == BytePath::from_bytes(b"refs/heads/zz_blocked")),
            "{last_operation:?}"
        );
        assert_eq!(blocked_reads.iter().filter(|(_, op)| matches!(op, GitRead::ResolveCommit { reference } if *reference == BytePath::from_bytes(b"refs/heads/zz_blocked"))).count(), 1);
        assert!(
            later
                .iter()
                .all(|(path, _)| !path.to_path().unwrap().starts_with(&blocked.0)),
            "dependent child retried after deep timeout: {later:?}"
        );
        assert!(
            later
                .iter()
                .any(|(path, _)| path.to_path().unwrap().starts_with(&healthy.0))
        );
        let item = facts
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&blocked.0))
            .unwrap();
        assert!(
            !item.private_history.as_ref().unwrap().enumeration_complete,
            "deep timeout was retried as a fresh private history"
        );
        assert!(item.observations.is_empty());
        assert!(item.local_summary.as_ref().unwrap().index_complete);
        let good = facts
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&healthy.0))
            .unwrap();
        assert!(good.private_history.as_ref().unwrap().enumeration_complete);
        assert_eq!(good.history[0].result, "identical_commit");
    }
}
