#![cfg(unix)]
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use wt_sweep::discover::resume::{DiscoveryCheckpoint, scan_scope_resumable_with_progress};
use wt_sweep::discover::{DirectoryListing, DiscoveryOptions, DiscoveryProvider, LocalFilesystem};
use wt_sweep::model::{BytePath, ScanScope};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-resume-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        Self(path)
    }
    fn scope(&self) -> ScanScope {
        ScanScope::Roots {
            paths: vec![BytePath::from_path(&self.0)],
        }
    }
    fn git(&self, repo: &Path, args: &[&str]) {
        let mut child = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(repo)
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| panic!("git: {e}"));
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait().unwrap_or_else(|e| panic!("wait: {e}")) {
                assert!(status.success(), "fixture Git failed: {args:?}");
                return;
            }
            if started.elapsed() > Duration::from_secs(10) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("fixture Git timed out: {args:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn repo(&self, relative: &Path) -> PathBuf {
        let path = self.0.join(relative);
        fs::create_dir_all(&path).unwrap_or_else(|e| panic!("mkdir: {e}"));
        self.git(&path, &["init", "-b", "main"]);
        self.git(&path, &["commit", "--allow-empty", "-m", "initial"]);
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn options(directories: usize) -> DiscoveryOptions {
    DiscoveryOptions {
        max_directories: directories,
        timeout: Duration::from_secs(20),
        ..Default::default()
    }
}
fn family_paths(inventory: &wt_sweep::model::Inventory) -> BTreeSet<PathBuf> {
    inventory
        .families
        .iter()
        .map(|f| {
            f.common_dir
                .to_path()
                .unwrap_or_else(|e| panic!("path: {e}"))
        })
        .collect()
}

// Metadata batches identify the walker path; Git residency preflight uses
// individual metadata reads. Keeping both counters avoids hiding the required
// repeated Git administration reads behind enumeration-only metrics.
#[derive(Default)]
struct CountingDiscovery {
    walker_path: RefCell<Option<PathBuf>>,
    walker_reads: RefCell<BTreeMap<PathBuf, usize>>,
    walker_entries: Cell<usize>,
    preflight_reads: Cell<usize>,
}
impl CountingDiscovery {
    fn begin_pass(&self) {
        // Resume refreshes known Git candidates before taking a new walker batch.
        // A previous pass's stopped path must not label that fresh preflight I/O.
        *self.walker_path.borrow_mut() = None;
    }
}
impl DiscoveryProvider for CountingDiscovery {
    fn metadata_batch(&self, paths: &[PathBuf], workers: usize) -> Vec<io::Result<fs::Metadata>> {
        assert_eq!(workers, 1, "counting fixture expects a serial walker");
        assert_eq!(paths.len(), 1, "counting fixture expects one walker path");
        *self.walker_path.borrow_mut() = paths.first().cloned();
        LocalFilesystem.metadata_batch(paths, workers)
    }
    fn read_directory(&self, path: &Path, limit: usize) -> io::Result<DirectoryListing> {
        let is_walk = self.walker_path.borrow().as_deref() == Some(path);
        if is_walk {
            *self
                .walker_reads
                .borrow_mut()
                .entry(path.to_path_buf())
                .or_default() += 1;
        } else {
            self.preflight_reads.set(self.preflight_reads.get() + 1);
        }
        let listing = LocalFilesystem.read_directory(path, limit)?;
        if is_walk {
            assert!(
                listing.complete && listing.errors.is_empty(),
                "fixture directory must fit a full listing"
            );
            self.walker_entries
                .set(self.walker_entries.get() + listing.entries.len());
        }
        Ok(listing)
    }
}

#[test]
fn repeated_bounded_passes_find_hidden_nested_repositories_without_duplicates() {
    let f = Fixture::new();
    let first = f.repo(Path::new("main"));
    let hidden = f.repo(Path::new(".hidden/ignored/nested"));
    let nested = f.repo(Path::new("main/nested"));
    let expected = [first.join(".git"), hidden.join(".git"), nested.join(".git")]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let bounded = DiscoveryOptions {
        workers: 1,
        ..options(2)
    };
    let complete = DiscoveryOptions {
        workers: 1,
        ..options(1000)
    };
    let resumed_provider = CountingDiscovery::default();
    resumed_provider.begin_pass();
    // Fixture creation is excluded. Continuation durations include the
    // in-memory checkpoint encode/decode, but no CLI startup or checkpoint disk I/O.
    let first_started = Instant::now();
    let mut batch = scan_scope_resumable_with_progress(
        &f.scope(),
        &bounded,
        &resumed_provider,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    let mut total_pass_time = first_started.elapsed();
    assert!(!batch.inventory.coverage.traversal_complete);
    let first_metrics = batch
        .inventory
        .coverage
        .discovery_metrics
        .as_ref()
        .unwrap_or_else(|| panic!("first pass metrics missing"));
    let mut passes = 1;
    let mut attempts = first_metrics.directory_read_attempts;
    let mut entries = first_metrics.entries_listed;
    for _ in 0..150 {
        if batch.inventory.coverage.traversal_complete {
            break;
        }
        let pass_started = Instant::now();
        let saved = batch
            .checkpoint
            .to_bytes()
            .unwrap_or_else(|e| panic!("save: {e}"));
        let checkpoint =
            DiscoveryCheckpoint::from_bytes(&saved).unwrap_or_else(|e| panic!("load: {e}"));
        resumed_provider.begin_pass();
        batch = scan_scope_resumable_with_progress(
            &f.scope(),
            &bounded,
            &resumed_provider,
            Some(&checkpoint),
            &mut |_| {},
        )
        .unwrap_or_else(|e| panic!("resume: {e}"));
        total_pass_time += pass_started.elapsed();
        let metrics = batch
            .inventory
            .coverage
            .discovery_metrics
            .as_ref()
            .unwrap_or_else(|| panic!("resumed pass metrics missing"));
        passes += 1;
        attempts += metrics.directory_read_attempts;
        entries += metrics.entries_listed;
    }
    assert!(
        batch.inventory.coverage.traversal_complete,
        "resume never completed: {:?}",
        batch.inventory.coverage.gaps
    );
    assert_eq!(family_paths(&batch.inventory), expected);
    assert_eq!(batch.inventory.families.len(), 3);
    assert_eq!(batch.inventory.coverage.discovery_intervals.len(), passes);
    assert!(passes > 1);
    assert_eq!(
        batch.inventory.coverage.temporal_consistency,
        "multiple_observation_intervals_no_snapshot"
    );

    let fresh_provider = CountingDiscovery::default();
    fresh_provider.begin_pass();
    let fresh_started = Instant::now();
    let fresh = wt_sweep::discover::scan_scope_with(&f.scope(), &complete, &fresh_provider)
        .unwrap_or_else(|e| panic!("fresh scan: {e}"));
    let fresh_time = fresh_started.elapsed();
    assert!(fresh.coverage.traversal_complete);
    assert_eq!(family_paths(&fresh), expected);
    let fresh_metrics = fresh
        .coverage
        .discovery_metrics
        .as_ref()
        .unwrap_or_else(|| panic!("fresh metrics missing"));
    let resumed_reads = resumed_provider.walker_reads.borrow();
    let fresh_reads = fresh_provider.walker_reads.borrow();
    // Every fixture listing completed: a second walker read of any path is a
    // repeated completed directory, even if aggregate telemetry looks correct.
    for (path, count) in resumed_reads.iter() {
        assert_eq!(*count, 1, "completed directory relisted: {path:?}");
    }
    assert_eq!(
        *resumed_reads, *fresh_reads,
        "continuation must enumerate exactly the fresh scan's directories once"
    );
    assert_eq!(attempts, resumed_reads.values().sum::<usize>());
    assert_eq!(entries, resumed_provider.walker_entries.get());
    assert_eq!(
        fresh_metrics.directory_read_attempts,
        fresh_reads.values().sum::<usize>()
    );
    assert_eq!(
        fresh_metrics.entries_listed,
        fresh_provider.walker_entries.get()
    );
    assert_eq!(
        attempts, fresh_metrics.directory_read_attempts,
        "bounded passes must not repeat completed directory listings"
    );
    assert_eq!(
        entries, fresh_metrics.entries_listed,
        "bounded passes must not relist previously completed entries"
    );
    println!(
        "resumable_discovery_work: passes={passes} total_elapsed_ms={} directory_attempts={attempts} entries_listed={entries} families={} git_preflight_directory_attempts={}; fresh_scan: passes=1 total_elapsed_ms={} directory_attempts={} entries_listed={} families={} git_preflight_directory_attempts={}; measurement=single_sample os_cache=not_cleared order=resume_then_fresh fixture_creation=excluded checkpoint_roundtrip=in_memory_included cli_startup_and_checkpoint_disk_io=excluded speed_assertion=none",
        total_pass_time.as_millis(),
        batch.inventory.families.len(),
        resumed_provider.preflight_reads.get(),
        fresh_time.as_millis(),
        fresh_metrics.directory_read_attempts,
        fresh_metrics.entries_listed,
        fresh.families.len(),
        fresh_provider.preflight_reads.get()
    );
}

struct StopAfterListing {
    target: PathBuf,
    stop: Cell<bool>,
}
impl DiscoveryProvider for StopAfterListing {
    fn read_directory(&self, path: &Path, limit: usize) -> io::Result<DirectoryListing> {
        let listing = LocalFilesystem.read_directory(path, limit)?;
        if path == self.target {
            self.stop.set(true);
        }
        Ok(listing)
    }
    fn cancelled(&self) -> bool {
        self.stop.get()
    }
}
#[test]
fn interruption_after_listing_replays_parent_marker_before_committing_children() {
    let f = Fixture::new();
    let repo = f.repo(Path::new("repo"));
    let provider = StopAfterListing {
        target: repo.clone(),
        stop: Cell::new(false),
    };
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &provider,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    assert!(!first.inventory.coverage.traversal_complete);
    let next = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(next.inventory.coverage.traversal_complete);
    assert_eq!(
        family_paths(&next.inventory),
        [repo.join(".git")].into_iter().collect()
    );
    assert_eq!(next.inventory.families.len(), 1);
}
#[test]
fn a_wide_incomplete_listing_requires_replay_with_sufficient_allowance() {
    let f = Fixture::new();
    for index in 0..6 {
        fs::create_dir(f.0.join(format!("dir-{index}"))).unwrap_or_else(|e| panic!("mkdir: {e}"));
    }
    f.repo(Path::new("dir-5/hidden"));
    let small = DiscoveryOptions {
        max_entries: 2,
        ..options(1000)
    };
    let first =
        scan_scope_resumable_with_progress(&f.scope(), &small, &LocalFilesystem, None, &mut |_| {})
            .unwrap_or_else(|e| panic!("first: {e}"));
    let stalled = scan_scope_resumable_with_progress(
        &f.scope(),
        &small,
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("small resume: {e}"));
    assert!(!stalled.inventory.coverage.traversal_complete);
    assert!(stalled.inventory.coverage.remaining.is_some());
    let complete = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&stalled.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("larger resume: {e}"));
    assert!(complete.inventory.coverage.traversal_complete);
    assert_eq!(complete.inventory.families.len(), 1);
}

#[test]
fn checkpoint_rejects_tampering_versions_and_scope_changes() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("pending")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    let bytes = first
        .checkpoint
        .to_bytes()
        .unwrap_or_else(|e| panic!("save: {e}"));
    for field in ["schema", "sha256"] {
        let mut value: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("JSON: {e}"));
        value[field] = "bad".into();
        assert!(
            DiscoveryCheckpoint::from_bytes(&serde_json::to_vec(&value).unwrap_or_default())
                .is_err()
        );
    }
    assert!(DiscoveryCheckpoint::from_bytes(b"not json").is_err());
    let other = Fixture::new();
    assert!(
        scan_scope_resumable_with_progress(
            &other.scope(),
            &options(1),
            &LocalFilesystem,
            Some(&first.checkpoint),
            &mut |_| {}
        )
        .is_err()
    );
    assert!(
        scan_scope_resumable_with_progress(
            &ScanScope::AllMounted,
            &options(1),
            &LocalFilesystem,
            None,
            &mut |_| {}
        )
        .is_err()
    );
    assert!(
        scan_scope_resumable_with_progress(
            &ScanScope::Roots {
                paths: vec![BytePath::from_path(Path::new("relative"))]
            },
            &options(1),
            &LocalFilesystem,
            None,
            &mut |_| {}
        )
        .is_err()
    );
}
#[test]
fn replacing_completed_directory_refuses_saved_identity() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("a")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fs::create_dir(f.0.join("z")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(2),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    fs::rename(f.0.join("a"), f.0.join("moved-a")).unwrap_or_else(|e| panic!("rename owned: {e}"));
    fs::create_dir(f.0.join("a")).unwrap_or_else(|e| panic!("replacement: {e}"));
    assert!(
        scan_scope_resumable_with_progress(
            &f.scope(),
            &options(100),
            &LocalFilesystem,
            Some(&first.checkpoint),
            &mut |_| {}
        )
        .is_err()
    );
}
#[test]
fn resumed_inventory_recollects_head_instead_of_reusing_saved_git_observations() {
    let f = Fixture::new();
    let repo = f.repo(Path::new("a-repo"));
    fs::create_dir(f.0.join("z-pending")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(2),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    assert_eq!(first.inventory.families.len(), 1);
    let old_head = first.inventory.families[0].worktrees[0].head.clone();
    let serialized = first
        .checkpoint
        .to_bytes()
        .unwrap_or_else(|e| panic!("save: {e}"));
    assert!(
        !String::from_utf8_lossy(&serialized)
            .contains(old_head.as_deref().unwrap_or("unavailable"))
    );
    f.git(
        &repo,
        &["commit", "--allow-empty", "-m", "changed after checkpoint"],
    );
    let resumed = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(resumed.inventory.coverage.traversal_complete);
    assert_ne!(resumed.inventory.families[0].worktrees[0].head, old_head);
}
#[test]
fn replaced_root_symlink_is_refused_without_following_it() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("child")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    let other = Fixture::new();
    let old = f.0.with_extension("original");
    fs::rename(&f.0, &old).unwrap_or_else(|e| panic!("rename owned root: {e}"));
    std::os::unix::fs::symlink(&other.0, &f.0).unwrap_or_else(|e| panic!("symlink: {e}"));
    assert!(
        scan_scope_resumable_with_progress(
            &f.scope(),
            &options(1000),
            &LocalFilesystem,
            Some(&first.checkpoint),
            &mut |_| {}
        )
        .is_err()
    );
    fs::remove_file(&f.0).unwrap_or_else(|e| panic!("remove owned link: {e}"));
    fs::rename(old, &f.0).unwrap_or_else(|e| panic!("restore owned root: {e}"));
}

struct StopAfterMetadata {
    stop: Cell<bool>,
    target: PathBuf,
}
impl DiscoveryProvider for StopAfterMetadata {
    fn metadata_batch(&self, paths: &[PathBuf], _: usize) -> Vec<io::Result<fs::Metadata>> {
        let results = paths.iter().map(|path| self.metadata(path)).collect();
        if paths.iter().any(|path| path == &self.target) {
            self.stop.set(true);
        }
        results
    }
    fn cancelled(&self) -> bool {
        self.stop.get()
    }
}
#[test]
fn metadata_ready_results_are_discarded_and_pending_paths_are_read_freshly() {
    let f = Fixture::new();
    let pending = f.0.join("a-pending");
    fs::write(&pending, b"initially a file").unwrap_or_else(|e| panic!("file: {e}"));
    fs::create_dir(f.0.join("z-pending")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let provider = StopAfterMetadata {
        stop: Cell::new(false),
        target: pending.clone(),
    };
    let settings = DiscoveryOptions {
        workers: 2,
        ..options(1000)
    };
    let first =
        scan_scope_resumable_with_progress(&f.scope(), &settings, &provider, None, &mut |_| {})
            .unwrap_or_else(|e| panic!("first: {e}"));
    assert!(first.inventory.coverage.remaining.is_some());
    fs::remove_file(&pending).unwrap_or_else(|e| panic!("remove owned file: {e}"));
    let repo = f.repo(Path::new("a-pending"));
    let next = scan_scope_resumable_with_progress(
        &f.scope(),
        &settings,
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(next.inventory.coverage.traversal_complete);
    assert_eq!(
        family_paths(&next.inventory),
        [repo.join(".git")].into_iter().collect()
    );
}
#[test]
fn retargeted_observed_symlink_refuses_the_saved_frontier() {
    let f = Fixture::new();
    for name in ["a-target", "b-other", "z-pending"] {
        fs::create_dir(f.0.join(name)).unwrap_or_else(|e| panic!("mkdir: {e}"));
    }
    let alias = f.0.join(".alias");
    std::os::unix::fs::symlink(f.0.join("a-target"), &alias)
        .unwrap_or_else(|e| panic!("link: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(2),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    fs::rename(&alias, f.0.join("old-link")).unwrap_or_else(|e| panic!("rename owned link: {e}"));
    std::os::unix::fs::symlink(f.0.join("b-other"), &alias)
        .unwrap_or_else(|e| panic!("new link: {e}"));
    assert!(
        scan_scope_resumable_with_progress(
            &f.scope(),
            &options(1000),
            &LocalFilesystem,
            Some(&first.checkpoint),
            &mut |_| {}
        )
        .is_err()
    );
}
#[test]
fn oversized_checkpoint_and_malformed_path_are_refused_before_filesystem_reads() {
    struct NoIo;
    impl DiscoveryProvider for NoIo {
        fn metadata(&self, _: &Path) -> io::Result<fs::Metadata> {
            panic!("invalid state performed filesystem I/O");
        }
    }
    assert!(
        DiscoveryCheckpoint::from_bytes(&vec![
            b'x';
            wt_sweep::discover::resume::MAX_CHECKPOINT_BYTES + 1
        ])
        .is_err()
    );
    let invalid_scope = ScanScope::Roots {
        paths: vec![BytePath { hex: "zz".into() }],
    };
    assert!(
        scan_scope_resumable_with_progress(&invalid_scope, &options(1), &NoIo, None, &mut |_| {})
            .is_err()
    );
}
#[test]
fn cancelled_validation_retains_checkpoint_and_emits_finished_progress() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("child")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    struct Cancelled;
    impl DiscoveryProvider for Cancelled {
        fn cancelled(&self) -> bool {
            true
        }
    }
    let mut phases = Vec::new();
    let stopped = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &Cancelled,
        Some(&first.checkpoint),
        &mut |event| phases.push(event.phase.clone()),
    )
    .unwrap_or_else(|e| panic!("stop: {e}"));
    assert!(!stopped.inventory.coverage.traversal_complete);
    assert_eq!(
        phases,
        [
            wt_sweep::discover::DiscoveryPhase::Start,
            wt_sweep::discover::DiscoveryPhase::Finished
        ]
    );
    let next = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&stopped.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(next.inventory.coverage.traversal_complete);
}

#[test]
fn replay_after_git_validation_deduplicates_marker_observations() {
    let f = Fixture::new();
    let repo = f.repo(Path::new("repo"));
    let provider = StopAfterListing {
        target: repo.join(".git/refs"),
        stop: Cell::new(false),
    };
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &provider,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    assert_eq!(first.inventory.families.len(), 1);
    assert!(first.inventory.coverage.remaining.is_some());
    let next = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(next.inventory.coverage.traversal_complete);
    let marker = BytePath::from_path(&repo.join(".git"));
    assert_eq!(
        next.inventory
            .markers
            .iter()
            .filter(|m| m.path == marker && m.kind == "git_marker")
            .count(),
        1
    );
}
#[test]
fn checkpoint_preserves_unicode_and_newline_pending_path_bytes() {
    use std::os::unix::ffi::OsStringExt;
    let f = Fixture::new();
    let relative = PathBuf::from(std::ffi::OsString::from_vec(
        ".hidden-Δ\n".as_bytes().to_vec(),
    ));
    let repo = f.repo(&relative);
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    let checkpoint = DiscoveryCheckpoint::from_bytes(
        &first
            .checkpoint
            .to_bytes()
            .unwrap_or_else(|e| panic!("save: {e}")),
    )
    .unwrap_or_else(|e| panic!("load: {e}"));
    let resumed = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(resumed.inventory.coverage.traversal_complete);
    assert_eq!(
        family_paths(&resumed.inventory),
        [repo.join(".git")].into_iter().collect()
    );
}

#[test]
fn root_scope_larger_than_allowance_is_refused_before_identity_io() {
    struct NoIo;
    impl DiscoveryProvider for NoIo {
        fn metadata(&self, _: &Path) -> io::Result<fs::Metadata> {
            panic!("root preflight exceeded allowance");
        }
    }
    let scope = ScanScope::Roots {
        paths: vec![
            BytePath::from_path(Path::new("/private/tmp/root-a")),
            BytePath::from_path(Path::new("/private/tmp/root-b")),
        ],
    };
    let settings = DiscoveryOptions {
        max_entries: 1,
        ..options(1)
    };
    assert!(
        scan_scope_resumable_with_progress(&scope, &settings, &NoIo, None, &mut |_| {}).is_err()
    );
}
#[test]
fn resumable_progress_starts_before_root_identity_reads() {
    let f = Fixture::new();
    struct StartBeforeIo<'a>(&'a Cell<bool>);
    impl DiscoveryProvider for StartBeforeIo<'_> {
        fn metadata(&self, path: &Path) -> io::Result<fs::Metadata> {
            assert!(self.0.get(), "root identity read before start event");
            fs::symlink_metadata(path)
        }
    }
    let started = Cell::new(false);
    let provider = StartBeforeIo(&started);
    let mut starts = 0;
    let batch = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &provider,
        None,
        &mut |event| {
            if event.phase == wt_sweep::discover::DiscoveryPhase::Start {
                started.set(true);
                starts += 1;
            }
        },
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(batch.inventory.coverage.traversal_complete);
    assert_eq!(starts, 1);
}

#[test]
fn new_repository_in_completed_directory_is_historical_until_a_fresh_scan() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("a-completed")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fs::create_dir(f.0.join("z-pending")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let first = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(2),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("first: {e}"));
    assert!(first.inventory.coverage.remaining.is_some());
    let late = f.repo(Path::new("a-completed/late"));
    let continued = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1000),
        &LocalFilesystem,
        Some(&first.checkpoint),
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("resume: {e}"));
    assert!(continued.inventory.coverage.traversal_complete);
    assert!(continued.inventory.families.is_empty());
    assert_eq!(continued.inventory.coverage.discovery_intervals.len(), 2);
    assert_eq!(
        continued.inventory.coverage.temporal_consistency,
        "multiple_observation_intervals_no_snapshot"
    );
    let fresh = wt_sweep::discover::scan_scope_with(&f.scope(), &options(1000), &LocalFilesystem)
        .unwrap_or_else(|e| panic!("fresh scan: {e}"));
    assert!(fresh.coverage.traversal_complete);
    assert_eq!(
        family_paths(&fresh),
        [late.join(".git")].into_iter().collect()
    );
}
#[test]
fn recomputed_integrity_digest_does_not_allow_an_invalid_observation_interval() {
    use sha2::{Digest, Sha256};
    let f = Fixture::new();
    let batch = scan_scope_resumable_with_progress(
        &f.scope(),
        &options(1),
        &LocalFilesystem,
        None,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    let bytes = batch
        .checkpoint
        .to_bytes()
        .unwrap_or_else(|e| panic!("save: {e}"));
    let source = String::from_utf8(bytes).unwrap_or_else(|e| panic!("JSON encoding: {e}"));
    let value: serde_json::Value =
        serde_json::from_str(&source).unwrap_or_else(|e| panic!("JSON: {e}"));
    let finish = value["state"]["intervals"][0]["finished_unix_ms"]
        .as_u64()
        .unwrap_or_else(|| panic!("interval missing"));
    let changed = source.replace(
        &format!("\"finished_unix_ms\":{finish}"),
        "\"finished_unix_ms\":0",
    );
    assert_ne!(source, changed);
    let state_start = changed
        .find("\"state\":")
        .unwrap_or_else(|| panic!("state missing"))
        + "\"state\":".len();
    let state_bytes = &changed.as_bytes()[state_start..changed.len() - 1];
    let digest = format!("{:x}", Sha256::digest(state_bytes));
    let original_digest = value["sha256"]
        .as_str()
        .unwrap_or_else(|| panic!("digest missing"));
    let resealed = changed.replace(original_digest, &digest);
    let error = DiscoveryCheckpoint::from_bytes(resealed.as_bytes())
        .err()
        .unwrap_or_else(|| panic!("invalid interval accepted"));
    assert!(
        error.message.contains("invalid observation interval"),
        "wrong refusal: {error}"
    );
}
