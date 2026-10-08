//! Digest-protected traversal state, separate from current Git observations.
use super::*;
use crate::model::{Identity, ObservationInterval};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CHECKPOINT_SCHEMA: &str = "wt-sweep.discovery-checkpoint/1";
pub const MAX_CHECKPOINT_BYTES: usize = 24 * 1024 * 1024;

fn invalid(message: impl Into<String>) -> SweepError {
    SweepError::new(
        "invalid_checkpoint",
        "discovery_resume",
        Path::new("."),
        message,
    )
}
fn absolute(path: &BytePath) -> Result<PathBuf, SweepError> {
    let decoded = path.to_path()?;
    if !decoded.is_absolute()
        || decoded.components().any(|c| {
            matches!(
                c,
                std::path::Component::ParentDir | std::path::Component::CurDir
            )
        })
    {
        return Err(invalid(
            "checkpoint paths must be absolute without dot components",
        ));
    }
    Ok(decoded)
}
fn scope_paths(scope: &ScanScope) -> Result<&[BytePath], SweepError> {
    match scope {
        ScanScope::Roots { paths } if !paths.is_empty() => {
            for path in paths {
                absolute(path)?;
            }
            Ok(paths)
        }
        _ => Err(invalid(
            "resumable discovery supports only nonempty explicit absolute root scopes",
        )),
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryCheckpoint {
    schema: String,
    sha256: String,
    state: WalkState,
}
impl DiscoveryCheckpoint {
    fn seal(state: WalkState) -> Result<Self, SweepError> {
        let sha256 = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&state).map_err(|e| invalid(e.to_string()))?)
        );
        let checkpoint = Self {
            schema: CHECKPOINT_SCHEMA.into(),
            sha256,
            state,
        };
        checkpoint.validate()?;
        Ok(checkpoint)
    }
    fn validate(&self) -> Result<(), SweepError> {
        if self.schema != CHECKPOINT_SCHEMA {
            return Err(invalid("unsupported discovery checkpoint version"));
        }
        let digest = format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&self.state).map_err(|e| invalid(e.to_string()))?)
        );
        if self.sha256 != digest {
            return Err(invalid("discovery checkpoint digest mismatch"));
        }
        self.state.validate()
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, SweepError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| invalid(e.to_string()))?;
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(invalid(
                "checkpoint byte limit exceeded; state was not published",
            ));
        }
        Ok(bytes)
    }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, SweepError> {
        if bytes.len() > MAX_CHECKPOINT_BYTES {
            return Err(invalid("checkpoint byte limit exceeded"));
        }
        let checkpoint: Self = serde_json::from_slice(bytes).map_err(|e| invalid(e.to_string()))?;
        checkpoint.validate()?;
        Ok(checkpoint)
    }
    pub fn scope(&self) -> &ScanScope {
        &self.state.scope
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SavedPath {
    pub path: BytePath,
    pub depth: usize,
    pub parent_device: Option<u64>,
}
impl SavedPath {
    pub fn from_pending((path, depth, parent_device): &PendingPath) -> Self {
        Self {
            path: BytePath::from_path(path),
            depth: *depth,
            parent_device: *parent_device,
        }
    }
    pub fn pending(&self) -> Result<PendingPath, SweepError> {
        Ok((absolute(&self.path)?, self.depth, self.parent_device))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DirectoryGuard {
    pub path: BytePath,
    pub identity: Identity,
}
impl DirectoryGuard {
    pub fn observed(path: &Path, metadata: &fs::Metadata) -> Self {
        Self {
            path: BytePath::from_path(path),
            identity: Identity {
                device: metadata.dev(),
                inode: metadata.ino(),
            },
        }
    }
    fn verify(&self, provider: &dyn DiscoveryProvider) -> Result<(), SweepError> {
        let path = absolute(&self.path)?;
        let metadata = provider.metadata(&path).map_err(|e| {
            invalid(format!(
                "saved directory cannot be revalidated at {}: {e}",
                self.path.display()
            ))
        })?;
        if metadata.file_type().is_symlink()
            || !metadata.is_dir()
            || provider.is_dataless(&metadata)
            || metadata.dev() != self.identity.device
            || metadata.ino() != self.identity.inode
        {
            return Err(invalid(format!(
                "saved directory identity/type changed at {}; start a fresh discovery",
                self.path.display()
            )));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RootGuard {
    requested: BytePath,
    directory: DirectoryGuard,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct AliasGuard {
    pub path: BytePath,
    pub canonical: BytePath,
    pub identity: Identity,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Candidate {
    pub directory: BytePath,
    pub marker: BytePath,
    pub kind: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WalkState {
    scope: ScanScope,
    roots: Vec<RootGuard>,
    pub pending: Vec<SavedPath>,
    pub current: Option<SavedPath>,
    pub current_guard: Option<DirectoryGuard>,
    pub completed: Vec<DirectoryGuard>,
    pub alias_guards: Vec<AliasGuard>,
    pub candidates: Vec<Candidate>,
    pub gaps: Vec<Gap>,
    pub aliases: Vec<PathAlias>,
    pub volumes: Vec<VolumeObservation>,
    pub intervals: Vec<ObservationInterval>,
}
impl WalkState {
    fn new(
        scope: &ScanScope,
        provider: &dyn DiscoveryProvider,
        options: &DiscoveryOptions,
        started: Instant,
    ) -> Result<Self, SweepError> {
        let paths = scope_paths(scope)?;
        if paths.len() > options.max_entries {
            return Err(invalid(
                "root identity initialization exceeds entry allowance; no checkpoint created",
            ));
        }
        let mut roots = Vec::new();
        let mut pending = Vec::new();
        for requested in paths {
            if let Some(reason) = requested_stop(provider, started, options, 0) {
                return Err(invalid(format!(
                    "root identity initialization stopped: {}; no valid checkpoint created",
                    reason.as_str()
                )));
            }
            let path = absolute(requested)?;
            let metadata = provider
                .metadata(&path)
                .map_err(|e| invalid(format!("root metadata failed: {e}")))?;
            if metadata.file_type().is_symlink()
                || !metadata.is_dir()
                || provider.is_dataless(&metadata)
            {
                return Err(invalid(
                    "resumable roots must be resident directories, not symlinks",
                ));
            }
            let canonical = provider
                .canonicalize(&path)
                .map_err(|e| invalid(format!("root canonicalization failed: {e}")))?;
            roots.push(RootGuard {
                requested: requested.clone(),
                directory: DirectoryGuard::observed(&canonical, &metadata),
            });
            pending.push(SavedPath {
                path: requested.clone(),
                depth: 0,
                parent_device: None,
            });
        }
        Ok(Self {
            scope: scope.clone(),
            roots,
            pending,
            current: None,
            current_guard: None,
            completed: Vec::new(),
            alias_guards: Vec::new(),
            candidates: Vec::new(),
            gaps: Vec::new(),
            aliases: Vec::new(),
            volumes: Vec::new(),
            intervals: Vec::new(),
        })
    }
    fn validate(&self) -> Result<(), SweepError> {
        let paths = scope_paths(&self.scope)?;
        if paths.len() != self.roots.len()
            || self
                .roots
                .iter()
                .zip(paths)
                .any(|(guard, path)| &guard.requested != path)
        {
            return Err(invalid("checkpoint roots disagree with requested scope"));
        }
        let within = |path: &BytePath| -> Result<(), SweepError> {
            let path = absolute(path)?;
            if !self.roots.iter().any(|root| {
                root.directory
                    .path
                    .to_path()
                    .is_ok_and(|p| path.starts_with(p))
                    || root.requested.to_path().is_ok_and(|p| path.starts_with(p))
            }) {
                return Err(invalid(
                    "checkpoint traversal path is outside its explicit roots",
                ));
            }
            Ok(())
        };
        for root in &self.roots {
            absolute(&root.directory.path)?;
        }
        for saved in self.pending.iter().chain(self.current.iter()) {
            within(&saved.path)?;
        }
        for guard in self.completed.iter().chain(self.current_guard.iter()) {
            within(&guard.path)?;
        }
        for alias in &self.alias_guards {
            within(&alias.path)?;
            absolute(&alias.canonical)?;
        }
        for candidate in &self.candidates {
            within(&candidate.directory)?;
            within(&candidate.marker)?;
            let directory = absolute(&candidate.directory)?;
            let marker = absolute(&candidate.marker)?;
            if !((candidate.kind == "git_marker" && marker == directory.join(".git"))
                || (candidate.kind == "bare_or_administration" && marker == directory))
            {
                return Err(invalid("invalid candidate marker kind or relationship"));
            }
        }
        for gap in &self.gaps {
            within(&gap.path)?;
        }
        for alias in &self.aliases {
            within(&alias.path)?;
            absolute(&alias.canonical)?;
        }
        for volume in &self.volumes {
            within(&volume.path)?;
        }
        for interval in &self.intervals {
            if interval.finished_unix_ms < interval.started_unix_ms {
                return Err(invalid("invalid observation interval"));
            }
        }
        Ok(())
    }
    pub fn seed(&self, inventory: &mut Inventory) {
        inventory.coverage.gaps = self.gaps.clone();
        inventory.coverage.aliases = self.aliases.clone();
        inventory.coverage.volumes = self.volumes.clone();
    }
    pub fn queue(&self) -> Result<VecDeque<PendingPath>, SweepError> {
        self.current
            .iter()
            .chain(self.pending.iter())
            .map(SavedPath::pending)
            .collect()
    }
    pub fn candidate(&mut self, directory: &Path, marker: &Path, kind: &str) {
        let candidate = Candidate {
            directory: BytePath::from_path(directory),
            marker: BytePath::from_path(marker),
            kind: kind.into(),
        };
        if !self
            .candidates
            .iter()
            .any(|c| c.marker == candidate.marker && c.kind == candidate.kind)
        {
            self.candidates.push(candidate);
        }
    }
    fn verify(
        &self,
        provider: &dyn DiscoveryProvider,
        options: &DiscoveryOptions,
        started: Instant,
    ) -> Result<Option<DiscoveryStop>, SweepError> {
        let mut checked = 0usize;
        for root in &self.roots {
            if let Some(reason) = requested_stop(provider, started, options, 0) {
                return Ok(Some(reason));
            }
            if checked >= options.max_entries {
                return Ok(Some(DiscoveryStop::EntryBudget));
            }
            let requested = absolute(&root.requested)?;
            let metadata = provider
                .metadata(&requested)
                .map_err(|e| invalid(format!("root revalidation failed: {e}")))?;
            if metadata.file_type().is_symlink() || provider.is_dataless(&metadata) {
                return Err(invalid("saved root became a symlink or dataless path"));
            }
            let canonical = provider
                .canonicalize(&requested)
                .map_err(|e| invalid(e.to_string()))?;
            if BytePath::from_path(&canonical) != root.directory.path {
                return Err(invalid("saved root canonical path changed"));
            }
            root.directory.verify(provider)?;
            checked += 1;
        }
        for directory in self.completed.iter().chain(self.current_guard.iter()) {
            if let Some(reason) = requested_stop(provider, started, options, 0) {
                return Ok(Some(reason));
            }
            if checked >= options.max_entries {
                return Ok(Some(DiscoveryStop::EntryBudget));
            }
            directory.verify(provider)?;
            checked += 1;
        }
        for alias in &self.alias_guards {
            if let Some(reason) = requested_stop(provider, started, options, 0) {
                return Ok(Some(reason));
            }
            if checked >= options.max_entries {
                return Ok(Some(DiscoveryStop::EntryBudget));
            }
            let path = absolute(&alias.path)?;
            let metadata = provider
                .metadata(&path)
                .map_err(|e| invalid(e.to_string()))?;
            if !metadata.file_type().is_symlink()
                || metadata.dev() != alias.identity.device
                || metadata.ino() != alias.identity.inode
                || provider.is_dataless(&metadata)
            {
                return Err(invalid("saved alias identity/type changed"));
            }
            if BytePath::from_path(
                &provider
                    .canonicalize(&path)
                    .map_err(|e| invalid(e.to_string()))?,
            ) != alias.canonical
            {
                return Err(invalid("saved symlink target changed"));
            }
            checked += 1;
        }
        Ok(None)
    }
}

pub struct DiscoveryBatch {
    pub inventory: Inventory,
    pub checkpoint: DiscoveryCheckpoint,
}
/// Revalidates saved traversal identities and collects Git families in the new interval.
pub fn scan_scope_resumable_with_progress(
    scope: &ScanScope,
    options: &DiscoveryOptions,
    provider: &dyn DiscoveryProvider,
    checkpoint: Option<&DiscoveryCheckpoint>,
    progress: &mut dyn FnMut(&DiscoveryProgress),
) -> Result<DiscoveryBatch, SweepError> {
    scope_paths(scope)?;
    if !(1..=8).contains(&options.workers)
        || options.max_entries == 0
        || options.max_directories == 0
        || options.max_observation_bytes == 0
    {
        return Err(invalid("invalid resumable discovery limits"));
    }
    let started = Instant::now();
    let interval_start = crate::model::now_ms();
    let mut state = if let Some(saved) = checkpoint {
        saved.validate()?;
        if serde_json::to_vec(saved.scope()).map_err(|e| invalid(e.to_string()))?
            != serde_json::to_vec(scope).map_err(|e| invalid(e.to_string()))?
        {
            return Err(invalid("resume scope differs from checkpoint scope"));
        }
        Some(saved.state.clone())
    } else {
        if scope_paths(scope)?.len() > options.max_entries {
            return Err(invalid(
                "root identity initialization exceeds entry allowance; no checkpoint created",
            ));
        }
        None
    };
    let queued_paths = state.as_ref().map_or(scope_paths(scope)?.len(), |s| {
        s.pending.len() + usize::from(s.current.is_some())
    });
    progress(&DiscoveryProgress {
        elapsed_ms: started.elapsed().as_millis() as u64,
        phase: DiscoveryPhase::Start,
        directories: 0,
        entries: 0,
        families: 0,
        queued_paths,
        metadata_ready_paths: 0,
        current: state
            .as_ref()
            .and_then(|s| s.current.as_ref().map(|path| path.path.clone())),
        stop: None,
    });
    crate::residency::ensure_no_materialization()?;
    let mut state = match state.take() {
        Some(state) => state,
        None => WalkState::new(scope, provider, options, started)?,
    };
    let mut inventory = if let Some(reason) = state.verify(provider, options, started)? {
        let mut inventory = Inventory::new(scope.clone());
        state.seed(&mut inventory);
        inventory.coverage.roots = scope_paths(scope)?.to_vec();
        let mut event = DiscoveryProgress {
            elapsed_ms: started.elapsed().as_millis() as u64,
            phase: DiscoveryPhase::Start,
            directories: 0,
            entries: 0,
            families: 0,
            queued_paths: state.pending.len() + usize::from(state.current.is_some()),
            metadata_ready_paths: 0,
            current: state.current.as_ref().map(|path| path.path.clone()),
            stop: None,
        };
        stop_discovery(&mut inventory, reason.clone(), event.queued_paths, None);
        inventory.finish();
        event.phase = DiscoveryPhase::Finished;
        event.stop = Some(reason);
        record_metrics(&mut inventory, &event, started, options.workers);
        progress(&event);
        inventory
    } else {
        scan_scope_with_progress_state(
            scope,
            options,
            provider,
            &mut |event| {
                if event.phase != DiscoveryPhase::Start {
                    progress(event);
                }
            },
            Some(&mut state),
            started,
        )?
    };
    state.intervals.push(ObservationInterval {
        started_unix_ms: interval_start,
        finished_unix_ms: crate::model::now_ms(),
    });
    inventory.coverage.discovery_intervals = state.intervals.clone();
    if state.intervals.len() > 1 {
        inventory.coverage.temporal_consistency =
            "multiple_observation_intervals_no_snapshot".into();
    }
    let checkpoint = DiscoveryCheckpoint::seal(state)?;
    checkpoint.to_bytes()?;
    Ok(DiscoveryBatch {
        inventory,
        checkpoint,
    })
}
