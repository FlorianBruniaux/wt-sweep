//! Serial bounded filesystem discovery, separate from deterministic judgement.
use crate::git::scan_family;
use crate::model::{
    BytePath, DiscoveryRemainder, DiscoveryStop, Gap, Inventory, Marker, NativeMountDetails,
    PathAlias, RemainingCover, ScanScope, SweepError, VolumeObservation,
};
use std::collections::{BTreeSet, VecDeque};
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

type PendingPath = (PathBuf, usize, Option<u64>);

fn metadata_batch_with<F>(
    paths: &[PathBuf],
    workers: usize,
    read: &F,
) -> Vec<io::Result<fs::Metadata>>
where
    F: Fn(&Path) -> io::Result<fs::Metadata> + Sync,
{
    if workers == 1 {
        return paths.iter().map(|path| read(path)).collect();
    }
    paths
        .chunks(workers.clamp(1, 8))
        .flat_map(|batch| {
            std::thread::scope(|scope| {
                let handles = batch
                    .iter()
                    .map(|path| scope.spawn(move || read(path)))
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .map(|handle| {
                        handle.join().unwrap_or_else(|_| {
                            Err(io::Error::other("metadata worker panicked; path unread"))
                        })
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct DiscoveryOptions {
    pub workers: usize,
    pub max_entries: usize,
    pub max_directories: usize,
    pub max_depth: usize,
    pub timeout: Duration,
    /// Soft serialized-record ceiling; one bounded operation and pending queues may exceed it.
    pub max_observation_bytes: usize,
}
impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            workers: 1,
            max_entries: 1_000_000,
            max_directories: 100_000,
            max_depth: 256,
            timeout: Duration::from_secs(60),
            max_observation_bytes: 24 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiscoveryPhase {
    Start,
    Progress,
    Finished,
}
impl DiscoveryPhase {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Progress => "progress",
            Self::Finished => "finished",
        }
    }
}
#[derive(Clone, Debug)]
pub struct DiscoveryProgress {
    pub elapsed_ms: u64,
    pub phase: DiscoveryPhase,
    pub directories: usize,
    pub entries: usize,
    pub families: usize,
    pub queued_paths: usize,
    pub metadata_ready_paths: usize,
    pub current: Option<BytePath>,
    pub stop: Option<DiscoveryStop>,
}

pub struct DirectoryListing {
    pub entries: Vec<PathBuf>,
    pub complete: bool,
    pub errors: Vec<String>,
}
#[derive(Clone, Debug)]
pub struct MountedRoot {
    pub path: PathBuf,
    pub available: bool,
    pub kind: String,
    pub native: Option<NativeMountDetails>,
}

/// Injection boundary: tests supply roots and errors, never scan the host's `/`.
pub trait DiscoveryProvider {
    /// Results must correspond one-for-one to the input paths, in that order.
    /// Injected providers stay serial unless they explicitly implement concurrency.
    fn metadata_batch(&self, paths: &[PathBuf], _workers: usize) -> Vec<io::Result<fs::Metadata>> {
        paths.iter().map(|path| self.metadata(path)).collect()
    }
    fn read_file(&self, path: &Path, limit: u64) -> io::Result<Vec<u8>> {
        let fd = rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::CLOEXEC
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK,
            rustix::fs::Mode::empty(),
        )?;
        let file = fs::File::from(fd);
        let metadata = file.metadata()?;
        if !metadata.is_file() || self.is_dataless(&metadata) {
            return Err(io::Error::other("not a resident regular file"));
        }
        let mut bytes = Vec::new();
        file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > limit {
            return Err(io::Error::other("file read byte limit exceeded"));
        }
        Ok(bytes)
    }
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        fs::symlink_metadata(path)
    }
    fn canonicalize(&self, path: &Path) -> io::Result<PathBuf> {
        fs::canonicalize(path)
    }
    fn read_directory(&self, path: &Path, limit: usize) -> io::Result<DirectoryListing> {
        let mut entries = Vec::new();
        let mut errors = Vec::new();
        let mut complete = true;
        for (count, entry) in fs::read_dir(path)?.enumerate() {
            if count >= limit {
                complete = false;
                break;
            }
            match entry {
                Ok(entry) => entries.push(entry.path()),
                Err(error) => {
                    errors.push(error.to_string());
                    complete = false;
                }
            }
        }
        entries.sort();
        Ok(DirectoryListing {
            entries,
            complete,
            errors,
        })
    }
    fn cancelled(&self) -> bool {
        false
    }
    fn is_dataless(&self, metadata: &fs::Metadata) -> bool {
        #[cfg(target_os = "macos")]
        {
            use std::os::macos::fs::MetadataExt as MacMetadataExt;
            // SF_DATALESS from the macOS SDK sys/stat.h. Inspecting metadata
            // precedes directory reads and Git reads that could hydrate data.
            metadata.st_flags() & 0x4000_0000 != 0
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = metadata;
            false
        }
    }
    fn mounted_roots(&self) -> io::Result<Vec<MountedRoot>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "discovery provider must explicitly supply mount roots",
        ))
    }
    fn mount_inventory_complete(&self) -> bool {
        false
    }
}
pub struct LocalFilesystem;
impl DiscoveryProvider for LocalFilesystem {
    fn cancelled(&self) -> bool {
        crate::cancel::requested()
    }
    fn mounted_roots(&self) -> io::Result<Vec<MountedRoot>> {
        crate::mounts::enumerate()?
            .records
            .into_iter()
            .map(|record| {
                Ok(MountedRoot {
                    path: record.path.to_path().map_err(io::Error::other)?,
                    available: true,
                    kind: "kernel_listed_mount".into(),
                    native: Some(NativeMountDetails {
                        source: record.source,
                        filesystem: record.filesystem,
                        flags: record.flags,
                        filesystem_type: record.filesystem_type,
                        filesystem_subtype: record.filesystem_subtype,
                    }),
                })
            })
            .collect()
    }
    fn mount_inventory_complete(&self) -> bool {
        cfg!(target_os = "macos")
    }
    fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
        // Thread policies can override the process policy. Each metadata worker
        // establishes its own refusal before resolving a path.
        crate::residency::ensure_no_materialization().map_err(io::Error::other)?;
        fs::symlink_metadata(path)
    }
    fn metadata_batch(&self, paths: &[PathBuf], workers: usize) -> Vec<io::Result<fs::Metadata>> {
        metadata_batch_with(paths, workers, &|path| self.metadata(path))
    }
}

fn gap(inventory: &mut Inventory, path: &Path, reason: impl Into<String>) {
    inventory.coverage.gaps.push(Gap {
        path: BytePath::from_path(path),
        reason: reason.into(),
    });
}

pub fn scan_scope(scope: &ScanScope) -> Result<Inventory, SweepError> {
    scan_scope_with(scope, &DiscoveryOptions::default(), &LocalFilesystem)
}

struct ScopeContext<'a> {
    roots: &'a [PathBuf],
    mounts: &'a [MountedRoot],
}

fn resident_metadata(
    provider: &dyn DiscoveryProvider,
    path: &Path,
    optional: bool,
) -> io::Result<Option<fs::Metadata>> {
    let metadata = match provider.metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if optional && error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if provider.is_dataless(&metadata) {
        return Err(io::Error::other(
            "dataless Git administration/control file: validation refused",
        ));
    }
    if metadata.file_type().is_symlink() {
        let canonical = provider.canonicalize(path)?;
        return resident_metadata(provider, &canonical, optional);
    }
    Ok(Some(metadata))
}

fn preflight_git(provider: &dyn DiscoveryProvider, marker: &Path) -> io::Result<()> {
    let metadata = resident_metadata(provider, marker, false)?
        .ok_or_else(|| io::Error::other("missing Git marker"))?;
    let marker = if provider.metadata(marker)?.file_type().is_symlink() {
        provider.canonicalize(marker)?
    } else {
        marker.to_path_buf()
    };
    let admin = if metadata.is_file() {
        let raw = provider.read_file(&marker, 256 * 1024)?;
        let pointer = raw
            .strip_prefix(b"gitdir: ")
            .ok_or_else(|| io::Error::other("malformed gitdir marker"))?;
        let pointer = pointer.strip_suffix(b"\n").unwrap_or(pointer);
        if pointer.is_empty() || pointer.contains(&0) {
            return Err(io::Error::other("empty or NUL gitdir pointer"));
        }
        let path = PathBuf::from(std::ffi::OsString::from_vec(pointer.to_vec()));
        if path.is_absolute() {
            path
        } else {
            marker
                .parent()
                .ok_or_else(|| io::Error::other("marker has no parent"))?
                .join(path)
        }
    } else if metadata.is_dir() {
        marker
    } else {
        return Err(io::Error::other("Git marker is not a file or directory"));
    };
    resident_metadata(provider, &admin, false)?;
    let mut administrations = vec![admin.clone()];
    let commondir = admin.join("commondir");
    if resident_metadata(provider, &commondir, true)?.is_some() {
        let bytes = provider.read_file(&commondir, 256 * 1024)?;
        let bytes = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
        if bytes.is_empty() || bytes.contains(&0) {
            return Err(io::Error::other("invalid common directory pointer"));
        }
        let common = PathBuf::from(std::ffi::OsString::from_vec(bytes.to_vec()));
        administrations.push(if common.is_absolute() {
            common
        } else {
            admin.join(common)
        });
    }
    let mut pending = VecDeque::new();
    for admin in administrations {
        resident_metadata(provider, &admin, false)?;
        for name in [
            "HEAD",
            "config",
            "packed-refs",
            "gitdir",
            "commondir",
            "objects",
        ] {
            resident_metadata(provider, &admin.join(name), true)?;
        }
        pending.push_back(admin.join("refs"));
        pending.push_back(admin.join("worktrees"));
    }
    let mut inspected = 0usize;
    let mut seen = BTreeSet::new();
    while let Some(path) = pending.pop_front() {
        inspected += 1;
        if inspected > 10_000 {
            return Err(io::Error::other(
                "Git residency preflight entry limit reached",
            ));
        }
        let Some(metadata) = resident_metadata(provider, &path, true)? else {
            continue;
        };
        if metadata.is_dir() && seen.insert((metadata.dev(), metadata.ino())) {
            let remaining = 10_000usize
                .saturating_sub(inspected)
                .saturating_sub(pending.len());
            let listing = provider.read_directory(&path, remaining)?;
            if !listing.complete || !listing.errors.is_empty() {
                return Err(io::Error::other(
                    "Git residency preflight directory read incomplete",
                ));
            }
            pending.extend(listing.entries);
        }
    }
    Ok(())
}

fn validate_candidate(
    inventory: &mut Inventory,
    provider: &dyn DiscoveryProvider,
    directory: &Path,
    marker: &Path,
    kind: &str,
    families: &mut BTreeSet<(u64, u64)>,
    context: &ScopeContext<'_>,
) {
    let mut observation = Marker {
        path: BytePath::from_path(marker),
        kind: kind.into(),
        common_dir: None,
        origin: "filesystem".into(),
    };
    match preflight_git(provider, marker) {
        Err(error) => gap(
            inventory,
            marker,
            format!("marker metadata read failed: {error}"),
        ),
        Ok(()) => match scan_family(directory) {
            Ok(mut candidate) => {
                inventory.coverage.gaps.append(&mut candidate.coverage.gaps);
                for mut family in candidate.families.drain(..) {
                    observation.common_dir = Some(family.common_dir.clone());
                    if kind == "bare_or_administration" {
                        observation.kind = if family.worktrees.iter().any(|tree| tree.bare) {
                            "bare".into()
                        } else {
                            "git_administration".into()
                        };
                    }
                    for tree in &mut family.worktrees {
                        let tree_path = match tree.path.to_path() {
                            Ok(path) => path,
                            Err(error) => {
                                gap(inventory, marker, error.to_string());
                                continue;
                            }
                        };
                        if context
                            .mounts
                            .iter()
                            .any(|mount| !mount.available && tree_path.starts_with(&mount.path))
                        {
                            tree.path_state = crate::model::DirState::VolumeUnavailable {
                                reason: "registered path lies on a supplied unavailable volume"
                                    .into(),
                            };
                            tree.identity = None;
                            gap(
                                inventory,
                                &tree_path,
                                "volume_unavailable: registered worktree cannot be observed",
                            );
                        }
                    }
                    if families.insert((family.identity.device, family.identity.inode)) {
                        for tree in &family.worktrees {
                            let outside = tree.path.to_path().is_ok_and(|path| {
                                !context.roots.iter().any(|root| path.starts_with(root))
                            });
                            inventory.markers.push(Marker {
                                path: tree.path.clone(),
                                kind: "registered_worktree".into(),
                                common_dir: Some(family.common_dir.clone()),
                                origin: if outside {
                                    "git_registry_outside_roots".into()
                                } else {
                                    "git_registry".into()
                                },
                            });
                        }
                        inventory.families.push(family);
                    } else if let Some(earlier) = inventory
                        .families
                        .iter()
                        .find(|earlier| earlier.identity == family.identity)
                    {
                        match (
                            serde_json::to_string(earlier),
                            serde_json::to_string(&family),
                        ) {
                            (Ok(first), Ok(later)) if first != later => gap(
                                inventory,
                                marker,
                                format!(
                                    "contradictory family observations during traversal; earlier={first}; later={later}"
                                ),
                            ),
                            (Err(error), _) | (_, Err(error)) => gap(
                                inventory,
                                marker,
                                format!("family observations could not be compared: {error}"),
                            ),
                            _ => {}
                        }
                    }
                }
            }
            Err(error) => gap(inventory, marker, format!("Git validation failed: {error}")),
        },
    }
    inventory.markers.push(observation);
}

pub fn scan_scope_with(
    scope: &ScanScope,
    options: &DiscoveryOptions,
    provider: &dyn DiscoveryProvider,
) -> Result<Inventory, SweepError> {
    scan_scope_with_progress(scope, options, provider, &mut |_| {})
}

#[derive(Default)]
struct ObservationBudget {
    bytes: usize,
    charged: [usize; 5],
}
impl ObservationBudget {
    fn charge(&mut self, inventory: &Inventory) -> Result<(), SweepError> {
        macro_rules! charge {
            ($index:expr, $records:expr) => {{
                let records = $records;
                for record in &records[self.charged[$index]..] {
                    self.bytes = self
                        .bytes
                        .saturating_add(crate::output::observation_bytes(record)?);
                }
                self.charged[$index] = records.len();
            }};
        }
        charge!(0, &inventory.families);
        charge!(1, &inventory.markers);
        charge!(2, &inventory.coverage.gaps);
        charge!(3, &inventory.coverage.aliases);
        charge!(4, &inventory.coverage.volumes);
        Ok(())
    }
}

fn requested_stop(
    provider: &dyn DiscoveryProvider,
    started: Instant,
    options: &DiscoveryOptions,
    bytes: usize,
) -> Option<DiscoveryStop> {
    if provider.cancelled() {
        Some(DiscoveryStop::Interrupted)
    } else if started.elapsed() >= options.timeout {
        Some(DiscoveryStop::Deadline)
    } else if bytes > options.max_observation_bytes {
        Some(DiscoveryStop::ObservationBudget)
    } else {
        None
    }
}
fn stop_discovery(
    inventory: &mut Inventory,
    reason: DiscoveryStop,
    pending: usize,
    current: Option<&Path>,
) {
    if inventory.coverage.remaining.is_some() {
        return;
    }
    let path = current
        .map(Path::to_path_buf)
        .or_else(|| {
            inventory
                .coverage
                .roots
                .first()
                .and_then(|p| p.to_path().ok())
        })
        .unwrap_or_else(|| PathBuf::from("/"));
    gap(
        inventory,
        &path,
        format!(
            "discovery stopped: {}; requested scope remains conservatively covered",
            reason.as_str()
        ),
    );
    inventory.coverage.remaining = Some(DiscoveryRemainder {
        reason,
        known_pending_paths: pending,
        cover: RemainingCover::RequestedScope,
    });
    inventory.coverage.traversal_complete = false;
}
struct ProgressReporter<'a> {
    started: Instant,
    callback: &'a mut dyn FnMut(&DiscoveryProgress),
    last_emit: Instant,
    last_work: usize,
}
impl ProgressReporter<'_> {
    fn emit(&mut self, event: &DiscoveryProgress, force: bool) {
        let work = event.entries.saturating_add(event.directories);
        if force
            || work.saturating_sub(self.last_work) >= 128
            || self.last_emit.elapsed() >= Duration::from_millis(250)
        {
            let mut timed = event.clone();
            timed.elapsed_ms = self.started.elapsed().as_millis() as u64;
            (self.callback)(&timed);
            self.last_emit = Instant::now();
            self.last_work = work;
        }
    }
}

fn record_metrics(
    inventory: &mut Inventory,
    event: &DiscoveryProgress,
    started: Instant,
    workers: usize,
) {
    inventory.coverage.discovery_metrics = Some(crate::model::DiscoveryMetrics {
        elapsed_ms: started.elapsed().as_millis() as u64,
        directory_read_attempts: event.directories,
        entries_listed: event.entries,
        workers,
    });
}

pub fn scan_scope_with_progress(
    scope: &ScanScope,
    options: &DiscoveryOptions,
    provider: &dyn DiscoveryProvider,
    progress: &mut dyn FnMut(&DiscoveryProgress),
) -> Result<Inventory, SweepError> {
    let started = Instant::now();
    let mut event = DiscoveryProgress {
        elapsed_ms: 0,
        phase: DiscoveryPhase::Start,
        directories: 0,
        entries: 0,
        families: 0,
        queued_paths: match scope {
            ScanScope::Repo { .. } => 1,
            ScanScope::Roots { paths } => paths.len(),
            ScanScope::AllMounted => 0,
        },
        metadata_ready_paths: 0,
        current: None,
        stop: None,
    };
    let mut reporter = ProgressReporter {
        started,
        callback: progress,
        last_emit: started,
        last_work: 0,
    };
    reporter.emit(&event, true);
    crate::residency::ensure_no_materialization()?;
    if !(1..=8).contains(&options.workers)
        || options.max_entries == 0
        || options.max_directories == 0
        || options.max_observation_bytes == 0
    {
        return Err(SweepError::new(
            "invalid_limits",
            "discover",
            Path::new("."),
            "workers must be 1..=8 and entry, directory and observation limits must be nonzero",
        ));
    }
    let mut budget = ObservationBudget::default();
    let mut initial = Inventory::new(scope.clone());
    initial.coverage.roots = match scope {
        ScanScope::Repo { path } => vec![path.clone()],
        ScanScope::Roots { paths } => paths.clone(),
        ScanScope::AllMounted => Vec::new(),
    };
    if let Some(reason) = requested_stop(provider, started, options, 0) {
        stop_discovery(&mut initial, reason.clone(), event.queued_paths, None);
        initial.finish();
        event.phase = DiscoveryPhase::Finished;
        event.stop = Some(reason);
        record_metrics(&mut initial, &event, started, options.workers);
        reporter.emit(&event, true);
        return Ok(initial);
    }
    if let ScanScope::Repo { path } = scope {
        event.phase = DiscoveryPhase::Progress;
        event.current = Some(path.clone());
        reporter.emit(&event, true);
        if let Some(reason) = requested_stop(provider, started, options, 0) {
            stop_discovery(&mut initial, reason, 1, Some(&path.to_path()?));
        } else {
            match scan_family(&path.to_path()?) {
                Ok(observed) => initial = observed,
                Err(error) if provider.cancelled() => gap(
                    &mut initial,
                    &path.to_path()?,
                    format!("interrupted Git validation: {error}"),
                ),
                Err(error) => return Err(error),
            }
            budget.charge(&initial)?;
            if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
                stop_discovery(&mut initial, reason, 1, Some(&path.to_path()?));
            }
        }
        initial.coverage.roots = vec![path.clone()];
        initial.finish();
        event.phase = DiscoveryPhase::Finished;
        event.families = initial.families.len();
        event.queued_paths = usize::from(initial.coverage.remaining.is_some());
        event.stop = initial
            .coverage
            .remaining
            .as_ref()
            .map(|r| r.reason.clone());
        record_metrics(&mut initial, &event, started, options.workers);
        reporter.emit(&event, true);
        return Ok(initial);
    }
    let mut inventory = Inventory::new(scope.clone());
    let mounted_before = if matches!(scope, ScanScope::AllMounted) {
        if !provider.mount_inventory_complete() {
            gap(
                &mut inventory,
                Path::new("/"),
                "mount enumeration unavailable; unmounted and unexposed volumes remain unknown",
            );
        }
        match provider.mounted_roots() {
            Ok(roots) => roots,
            Err(error) => {
                gap(
                    &mut inventory,
                    Path::new("/"),
                    format!("mount observations failed: {error}"),
                );
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };
    let roots = match scope {
        ScanScope::Roots { paths } => {
            if paths.is_empty() {
                return Err(SweepError::new(
                    "invalid_scope",
                    "discover",
                    Path::new("."),
                    "root scope requires at least one root",
                ));
            }
            paths
                .iter()
                .map(BytePath::to_path)
                .collect::<Result<Vec<_>, _>>()?
        }
        ScanScope::AllMounted => mounted_before
            .iter()
            .map(|volume| volume.path.clone())
            .collect(),
        ScanScope::Repo { .. } => unreachable!(),
    };
    if options.max_entries == 0 || options.max_directories == 0 {
        return Err(SweepError::new(
            "invalid_limits",
            "discover",
            Path::new("."),
            "entry and directory limits must be nonzero",
        ));
    }
    inventory.coverage.roots = roots.iter().map(|root| BytePath::from_path(root)).collect();
    for root in &mounted_before {
        inventory.coverage.volumes.push(VolumeObservation {
            path: BytePath::from_path(&root.path),
            available: root.available,
            kind: root.kind.clone(),
            native: root.native.clone(),
        });
        if !root.available {
            gap(
                &mut inventory,
                &root.path,
                "volume_unavailable: supplied mount is not accessible",
            );
        }
    }
    let canonical_roots: Vec<_> = roots
        .iter()
        .map(|root| provider.canonicalize(root).unwrap_or_else(|_| root.clone()))
        .collect();
    let context = ScopeContext {
        roots: &canonical_roots,
        mounts: &mounted_before,
    };
    let mut queue: VecDeque<_> = roots
        .into_iter()
        .filter(|root| {
            !mounted_before
                .iter()
                .any(|volume| &volume.path == root && !volume.available)
        })
        .map(|path| (path, 0usize, None))
        .collect();
    let mut seen_directories = BTreeSet::new();
    let mut families = BTreeSet::new();
    let mut entry_count = 0usize;
    let mut directory_count = 0usize;
    let mut metadata_ready: VecDeque<(PendingPath, io::Result<fs::Metadata>)> = VecDeque::new();
    let mut current: Option<PathBuf> = None;
    while !queue.is_empty() || !metadata_ready.is_empty() {
        budget.charge(&inventory)?;
        event.phase = DiscoveryPhase::Progress;
        event.directories = directory_count;
        event.entries = entry_count;
        event.families = inventory.families.len();
        event.queued_paths = queue.len();
        event.metadata_ready_paths = metadata_ready.len();
        event.current = current.as_deref().map(BytePath::from_path);
        reporter.emit(&event, false);
        if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
            stop_discovery(
                &mut inventory,
                reason,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(usize::from(current.is_some())),
                current.as_deref(),
            );
            break;
        }
        if metadata_ready.is_empty() {
            let count = queue.len().min(options.workers);
            let batch = queue.drain(..count).collect::<Vec<_>>();
            let paths = batch
                .iter()
                .map(|entry| entry.0.clone())
                .collect::<Vec<_>>();
            let results = provider.metadata_batch(&paths, options.workers);
            if results.len() != batch.len() {
                return Err(SweepError::new(
                    "discovery_provider",
                    "metadata_batch",
                    Path::new("."),
                    "metadata provider omitted or added path results",
                ));
            }
            metadata_ready.extend(batch.into_iter().zip(results));
            if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
                stop_discovery(
                    &mut inventory,
                    reason,
                    queue.len().saturating_add(metadata_ready.len()),
                    current.as_deref(),
                );
                break;
            }
        }
        let Some(((path, depth, parent_device), metadata)) = metadata_ready.pop_front() else {
            break;
        };
        current = Some(path.clone());
        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(error) => {
                gap(
                    &mut inventory,
                    &path,
                    format!("path metadata read failed, absence not assumed: {error}"),
                );
                continue;
            }
        };
        if provider.is_dataless(&metadata) {
            gap(
                &mut inventory,
                &path,
                "dataless path: traversal refused to avoid hydration",
            );
            continue;
        }
        let (path, metadata) = if metadata.file_type().is_symlink() {
            match provider.canonicalize(&path).and_then(|canonical| {
                provider
                    .metadata(&canonical)
                    .map(|metadata| (canonical, metadata))
            }) {
                Ok((canonical, target)) => {
                    inventory.coverage.aliases.push(PathAlias {
                        path: BytePath::from_path(&path),
                        canonical: BytePath::from_path(&canonical),
                    });
                    if !canonical_roots
                        .iter()
                        .any(|root| canonical.starts_with(root))
                    {
                        gap(
                            &mut inventory,
                            &path,
                            "symlink target outside explicit roots; traversal boundary retained without expansion",
                        );
                        continue;
                    }
                    if target.is_dir() && seen_directories.contains(&(target.dev(), target.ino())) {
                        gap(
                            &mut inventory,
                            &path,
                            "symlink reaches an already visited directory; cycle or repeated alias retained",
                        );
                    }
                    if provider.is_dataless(&target) {
                        gap(
                            &mut inventory,
                            &path,
                            "dataless symlink target: traversal refused",
                        );
                        continue;
                    }
                    (canonical, target)
                }
                Err(error) => {
                    gap(
                        &mut inventory,
                        &path,
                        format!("broken or cyclic symlink: {error}"),
                    );
                    continue;
                }
            }
        } else {
            (path, metadata)
        };
        if !metadata.is_dir() {
            continue;
        }
        let directory_id = (metadata.dev(), metadata.ino());
        if !seen_directories.insert(directory_id) {
            continue;
        }
        if parent_device.is_some_and(|device| device != metadata.dev()) {
            inventory.coverage.volumes.push(VolumeObservation {
                path: BytePath::from_path(&path),
                available: true,
                kind: "observed_device_boundary".into(),
                native: None,
            });
        }
        if depth > options.max_depth {
            gap(
                &mut inventory,
                &path,
                "depth limit reached; subtree not visited",
            );
            continue;
        }
        if directory_count >= options.max_directories {
            stop_discovery(
                &mut inventory,
                DiscoveryStop::DirectoryBudget,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        let remaining = options.max_entries.saturating_sub(entry_count);
        if remaining == 0 {
            stop_discovery(
                &mut inventory,
                DiscoveryStop::EntryBudget,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        budget.charge(&inventory)?;
        if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
            stop_discovery(
                &mut inventory,
                reason,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        directory_count += 1;
        let mut listing = match provider.read_directory(&path, remaining) {
            Ok(listing) => listing,
            Err(error) => {
                gap(
                    &mut inventory,
                    &path,
                    format!("directory read failed, absence not assumed: {error}"),
                );
                continue;
            }
        };
        if listing.entries.len() > remaining {
            listing.entries.truncate(remaining);
            listing.complete = false;
        }
        listing.entries.sort();
        entry_count += listing.entries.len();
        if !listing.complete {
            gap(
                &mut inventory,
                &path,
                "directory listing incomplete or entry limit reached",
            );
        }
        for error in listing.errors {
            gap(
                &mut inventory,
                &path,
                format!("directory entry read failed: {error}"),
            );
        }
        for entry in &listing.entries {
            queue.push_back((entry.clone(), depth + 1, Some(metadata.dev())));
        }
        budget.charge(&inventory)?;
        if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
            stop_discovery(
                &mut inventory,
                reason,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        let has = |name: &str| {
            listing
                .entries
                .iter()
                .any(|entry| entry.file_name() == Some(std::ffi::OsStr::new(name)))
        };
        if has(".git") {
            event.phase = DiscoveryPhase::Progress;
            event.directories = directory_count;
            event.entries = entry_count;
            event.families = inventory.families.len();
            event.queued_paths = queue.len();
            event.metadata_ready_paths = metadata_ready.len();
            event.current = Some(BytePath::from_path(&path));
            reporter.emit(&event, true);
            if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
                stop_discovery(
                    &mut inventory,
                    reason,
                    queue
                        .len()
                        .saturating_add(metadata_ready.len())
                        .saturating_add(1),
                    Some(&path),
                );
                break;
            }
            validate_candidate(
                &mut inventory,
                provider,
                &path,
                &path.join(".git"),
                "git_marker",
                &mut families,
                &context,
            );
        }
        budget.charge(&inventory)?;
        if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
            stop_discovery(
                &mut inventory,
                reason,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        if has("HEAD") && has("objects") && has("refs") {
            event.phase = DiscoveryPhase::Progress;
            event.directories = directory_count;
            event.entries = entry_count;
            event.families = inventory.families.len();
            event.queued_paths = queue.len();
            event.metadata_ready_paths = metadata_ready.len();
            event.current = Some(BytePath::from_path(&path));
            reporter.emit(&event, true);
            if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
                stop_discovery(
                    &mut inventory,
                    reason,
                    queue
                        .len()
                        .saturating_add(metadata_ready.len())
                        .saturating_add(1),
                    Some(&path),
                );
                break;
            }
            validate_candidate(
                &mut inventory,
                provider,
                &path,
                &path,
                "bare_or_administration",
                &mut families,
                &context,
            );
        }
        budget.charge(&inventory)?;
        if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
            stop_discovery(
                &mut inventory,
                reason,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
        if !listing.complete && entry_count >= options.max_entries {
            stop_discovery(
                &mut inventory,
                DiscoveryStop::EntryBudget,
                queue
                    .len()
                    .saturating_add(metadata_ready.len())
                    .saturating_add(1),
                Some(&path),
            );
            break;
        }
    }
    if matches!(scope, ScanScope::AllMounted) {
        match provider.mounted_roots() {
            Ok(after) => {
                let summarize = |roots: &[MountedRoot]| {
                    roots
                        .iter()
                        .map(|root| {
                            (
                                BytePath::from_path(&root.path),
                                root.available,
                                root.kind.clone(),
                                root.native.clone(),
                            )
                        })
                        .collect::<BTreeSet<_>>()
                };
                if summarize(&after) != summarize(&mounted_before) {
                    gap(
                        &mut inventory,
                        Path::new("/"),
                        "volume observations changed during discovery; temporal consistency unknown",
                    );
                }
                for before in &mounted_before {
                    let unchanged = after.iter().any(|current| {
                        current.path == before.path
                            && current.available
                            && current.kind == before.kind
                            && current.native == before.native
                    });
                    if !unchanged {
                        for volume in &mut inventory.coverage.volumes {
                            if volume.path == BytePath::from_path(&before.path) {
                                volume.available = false;
                            }
                        }
                    }
                }
                for tree in inventory.families.iter_mut().flat_map(|f| &mut f.worktrees) {
                    let path = tree.path.to_path()?;
                    // The longest matching mount wins: a nested mounted volume
                    // may remain available even if its containing mount changes.
                    let containing = mounted_before
                        .iter()
                        .filter(|mount| path.starts_with(&mount.path))
                        .max_by_key(|mount| mount.path.components().count());
                    if let Some(before) = containing {
                        let unchanged = after.iter().any(|current| {
                            current.path == before.path
                                && current.available
                                && current.kind == before.kind
                                && current.native == before.native
                        });
                        if !unchanged {
                            tree.path_state = crate::model::DirState::VolumeUnavailable {
                                reason: "containing mount disappeared or changed during discovery"
                                    .into(),
                            };
                        }
                    }
                }
            }
            Err(error) => {
                gap(
                    &mut inventory,
                    Path::new("/"),
                    format!("final volume observation failed: {error}"),
                );
                for tree in inventory.families.iter_mut().flat_map(|f| &mut f.worktrees) {
                    if matches!(tree.path_state, crate::model::DirState::Absent) {
                        tree.path_state = crate::model::DirState::Unknown {
                            reason:
                                "directory absence cannot be separated from an unavailable mount"
                                    .into(),
                        };
                    }
                }
            }
        }
    }
    budget.charge(&inventory)?;
    if let Some(reason) = requested_stop(provider, started, options, budget.bytes) {
        stop_discovery(
            &mut inventory,
            reason,
            queue
                .len()
                .saturating_add(metadata_ready.len())
                .saturating_add(usize::from(current.is_some())),
            current.as_deref(),
        );
    }
    inventory
        .families
        .sort_by(|a, b| a.common_dir.cmp(&b.common_dir));
    inventory
        .markers
        .sort_by(|a, b| (&a.path, &a.origin, &a.kind).cmp(&(&b.path, &b.origin, &b.kind)));
    inventory
        .coverage
        .aliases
        .sort_by(|a, b| a.path.cmp(&b.path));
    inventory
        .coverage
        .gaps
        .sort_by(|a, b| (&a.path, &a.reason).cmp(&(&b.path, &b.reason)));
    inventory.coverage.traversal_complete =
        inventory.coverage.gaps.is_empty() && inventory.coverage.remaining.is_none();
    inventory.finish();
    event.phase = DiscoveryPhase::Finished;
    event.directories = directory_count;
    event.entries = entry_count;
    event.families = inventory.families.len();
    event.queued_paths = queue.len();
    event.metadata_ready_paths = metadata_ready.len();
    event.current = current.as_deref().map(BytePath::from_path);
    event.stop = inventory
        .coverage
        .remaining
        .as_ref()
        .map(|r| r.reason.clone());
    record_metrics(&mut inventory, &event, started, options.workers);
    reporter.emit(&event, true);
    Ok(inventory)
}

#[cfg(test)]
mod concurrency_tests {
    use super::*;
    use std::sync::{Condvar, Mutex};

    #[test]
    fn metadata_reads_overlap_without_exceeding_the_worker_limit() {
        let state = Mutex::new((0usize, 0usize, 0usize));
        let ready = Condvar::new();
        let read = |_: &Path| {
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
            state.0 += 1;
            state.1 = state.1.max(state.0);
            state.2 += 1;
            ready.notify_all();
            let (mut state, _) = ready
                .wait_timeout_while(state, Duration::from_secs(1), |state| state.2 < 4)
                .unwrap_or_else(|e| e.into_inner());
            state.0 -= 1;
            Err(io::Error::other("injected metadata error"))
        };
        let paths = (0..4)
            .map(|n| PathBuf::from(n.to_string()))
            .collect::<Vec<_>>();
        let results = metadata_batch_with(&paths, 4, &read);
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(Result::is_err));
        let state = state.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(
            state.1, 4,
            "worker setting must allow actual overlapping reads"
        );
    }
}
