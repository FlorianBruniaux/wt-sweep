#![cfg(unix)]
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use wt_sweep::discover::{
    DirectoryListing, DiscoveryOptions, DiscoveryProvider, LocalFilesystem, MountedRoot,
    scan_scope, scan_scope_with,
};
use wt_sweep::model::{BytePath, ScanScope};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-discovery-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        Self(path)
    }
    fn git(&self, cwd: &Path, args: &[&std::ffi::OsStr]) {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(cwd)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("git: {e}"));
        assert!(output.status.success(), "git: {:?}", output.stderr);
    }
    fn repo(&self, path: &Path) {
        fs::create_dir_all(path).unwrap_or_else(|e| panic!("mkdir: {e}"));
        self.git(path, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
        self.git(
            path,
            &[
                "commit".as_ref(),
                "--allow-empty".as_ref(),
                "-m".as_ref(),
                "initial".as_ref(),
            ],
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn discovers_exact_nonempty_families_through_hidden_ignored_and_nested_paths() {
    let fixture = Fixture::new();
    let scope = fixture.0.join("scope");
    let outer = scope.join("outer");
    let nested = outer.join("nested");
    let hidden = scope.join(".hidden/repo");
    let ignored = outer.join("ignored/cache");
    for repo in [&outer, &nested, &hidden, &ignored] {
        fixture.repo(repo);
    }
    fs::write(outer.join(".gitignore"), b"ignored/\n").unwrap_or_else(|e| panic!("ignore: {e}"));
    let bare = scope.join("arbitrary-name");
    fixture.git(
        &scope,
        &["init".as_ref(), "--bare".as_ref(), bare.as_os_str()],
    );
    let separate = scope.join("separate");
    let admin = fixture.0.join("external-admin");
    fs::create_dir(&separate).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(
        &separate,
        &[
            "init".as_ref(),
            "-b".as_ref(),
            "main".as_ref(),
            "--separate-git-dir".as_ref(),
            admin.as_os_str(),
        ],
    );
    fixture.git(
        &separate,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let linked = fixture.0.join("outside-linked");
    fixture.git(
        &outer,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    let inventory = scan_scope(&ScanScope::Roots {
        paths: vec![BytePath::from_path(&scope)],
    })
    .unwrap_or_else(|e| panic!("discover: {e}"));
    let actual: BTreeSet<_> = inventory
        .families
        .iter()
        .map(|family| {
            family
                .common_dir
                .to_path()
                .unwrap_or_else(|e| panic!("path: {e}"))
        })
        .collect();
    let expected: BTreeSet<_> = [&outer, &nested, &hidden, &ignored]
        .into_iter()
        .map(|p| p.join(".git"))
        .chain([bare, admin])
        .collect();
    assert_eq!(actual, expected);
    assert_eq!(inventory.families.len(), 6);
    assert!(
        inventory.coverage.traversal_complete,
        "{:?}",
        inventory.coverage.gaps
    );
    assert!(
        inventory
            .markers
            .iter()
            .any(|marker| marker.path.to_path().ok() == Some(separate.join(".git")))
    );
    assert!(
        inventory
            .families
            .iter()
            .flat_map(|family| &family.worktrees)
            .any(|tree| tree.path.to_path().ok() == Some(linked.clone()))
    );
    assert!(
        inventory
            .markers
            .iter()
            .any(|marker| marker.path.to_path().ok() == Some(linked.clone())
                && marker.origin == "git_registry_outside_roots")
    );
}

#[test]
fn broken_markers_and_symlink_cycles_are_retained_as_gaps() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fixture.repo(&repo);
    let broken = fixture.0.join("broken");
    fs::create_dir(&broken).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fs::write(broken.join(".git"), b"gitdir: /no/synthetic/missing\n")
        .unwrap_or_else(|e| panic!("marker: {e}"));
    std::os::unix::fs::symlink(&fixture.0, fixture.0.join("loop"))
        .unwrap_or_else(|e| panic!("link: {e}"));
    let report = scan_scope(&ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    })
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(report.families.len(), 1);
    assert!(
        report.markers.iter().any(
            |marker| marker.path.to_path().ok() == Some(broken.join(".git"))
                && marker.common_dir.is_none()
        )
    );
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.path.to_path().ok() == Some(fixture.0.join("loop")))
    );
    assert!(!report.coverage.traversal_complete);
}

struct ReadErrorProvider {
    path: PathBuf,
    errno: i32,
}
impl DiscoveryProvider for ReadErrorProvider {
    fn read_directory(&self, path: &Path, limit: usize) -> std::io::Result<DirectoryListing> {
        if path == self.path {
            Err(std::io::Error::from_raw_os_error(self.errno))
        } else {
            LocalFilesystem.read_directory(path, limit)
        }
    }
}

#[test]
fn permission_and_io_failures_remain_gaps_while_other_roots_succeed() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fixture.repo(&repo);
    let denied = fixture.0.join("denied");
    fs::create_dir(&denied).unwrap_or_else(|e| panic!("mkdir: {e}"));
    for errno in [1, 13, 5] {
        let provider = ReadErrorProvider {
            path: denied.clone(),
            errno,
        };
        let scope = ScanScope::Roots {
            paths: vec![BytePath::from_path(&denied), BytePath::from_path(&repo)],
        };
        let report = scan_scope_with(&scope, &DiscoveryOptions::default(), &provider)
            .unwrap_or_else(|e| panic!("scan: {e}"));
        assert_eq!(report.families.len(), 1);
        assert!(!report.coverage.traversal_complete);
        assert!(
            report
                .coverage
                .gaps
                .iter()
                .any(|gap| gap.path.to_path().ok() == Some(denied.clone())
                    && gap.reason.contains("read failed"))
        );
    }
}

struct CancelledProvider;

#[test]
fn linked_root_recovers_family_before_a_denied_principal_directory_walk() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("principal");
    let linked = fixture.0.join("linked first");
    fixture.repo(&repo);
    fixture.git(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    let report = scan_scope_with(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&linked), BytePath::from_path(&repo)],
        },
        &DiscoveryOptions::default(),
        &ReadErrorProvider {
            path: repo.clone(),
            errno: 13,
        },
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(report.families.len(), 1);
    let family = &report.families[0];
    assert_eq!(family.common_dir, BytePath::from_path(&repo.join(".git")));
    let paths = family
        .worktrees
        .iter()
        .map(|tree| tree.path.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        paths,
        BTreeSet::from([BytePath::from_path(&repo), BytePath::from_path(&linked)])
    );
    assert_eq!(family.worktrees.len(), 2);
    assert!(!report.coverage.traversal_complete);
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.path == BytePath::from_path(&repo))
    );
    assert!(
        report
            .markers
            .iter()
            .any(|marker| marker.path == BytePath::from_path(&linked.join(".git")))
    );
}

#[test]
fn outside_root_symlink_is_reported_without_expanding_scope() {
    let fixture = Fixture::new();
    let root = fixture.0.join("scope");
    fs::create_dir(&root).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let external = fixture.0.join("external");
    fixture.repo(&external);
    std::os::unix::fs::symlink(&external, root.join("alias"))
        .unwrap_or_else(|e| panic!("symlink: {e}"));
    let report = scan_scope(&ScanScope::Roots {
        paths: vec![BytePath::from_path(&root)],
    })
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(report.families.is_empty());
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("outside explicit roots"))
    );
    assert_eq!(report.coverage.aliases.len(), 1);
}

struct FixedVolumes {
    roots: Vec<MountedRoot>,
}
impl DiscoveryProvider for FixedVolumes {
    fn mounted_roots(&self) -> std::io::Result<Vec<MountedRoot>> {
        Ok(self.roots.clone())
    }
    fn mount_inventory_complete(&self) -> bool {
        true
    }
}
#[test]
fn registered_tree_on_unavailable_volume_is_never_absent() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fixture.repo(&repo);
    let volume = fixture.0.join("volume");
    fs::create_dir(&volume).unwrap_or_else(|e| panic!("mkdir: {e}"));
    let linked = volume.join("linked");
    fixture.git(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    fs::remove_dir_all(&linked).unwrap_or_else(|e| panic!("owned deletion: {e}"));
    let provider = FixedVolumes {
        roots: vec![
            MountedRoot {
                path: repo,
                available: true,
                kind: "fixture".into(),
                native: None,
            },
            MountedRoot {
                path: volume,
                available: false,
                kind: "fixture".into(),
                native: None,
            },
        ],
    };
    let report = scan_scope_with(
        &ScanScope::AllMounted,
        &DiscoveryOptions::default(),
        &provider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    let tree = report
        .families
        .iter()
        .flat_map(|family| &family.worktrees)
        .find(|tree| tree.path.to_path().ok() == Some(linked.clone()))
        .unwrap_or_else(|| panic!("missing registration"));
    assert!(matches!(
        tree.path_state,
        wt_sweep::model::DirState::VolumeUnavailable { .. }
    ));
}
impl DiscoveryProvider for CancelledProvider {
    fn cancelled(&self) -> bool {
        true
    }
}
#[test]
fn cancellation_deadline_and_budgets_never_report_complete_coverage() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("deep/repo");
    fixture.repo(&repo);
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    let cancelled = scan_scope_with(&scope, &DiscoveryOptions::default(), &CancelledProvider)
        .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(cancelled.families.is_empty());
    assert!(!cancelled.coverage.traversal_complete);
    for options in [
        DiscoveryOptions {
            timeout: std::time::Duration::ZERO,
            ..DiscoveryOptions::default()
        },
        DiscoveryOptions {
            max_entries: 1,
            ..DiscoveryOptions::default()
        },
        DiscoveryOptions {
            max_depth: 0,
            ..DiscoveryOptions::default()
        },
        DiscoveryOptions {
            max_directories: 1,
            ..DiscoveryOptions::default()
        },
    ] {
        let report = scan_scope_with(&scope, &options, &LocalFilesystem)
            .unwrap_or_else(|e| panic!("scan: {e}"));
        assert!(!report.coverage.traversal_complete);
        assert!(!report.coverage.gaps.is_empty());
    }
}

#[test]
fn metadata_worker_count_preserves_results_at_entry_boundaries() {
    let fixture = Fixture::new();
    fixture.repo(&fixture.0.join("a/repo"));
    fixture.repo(&fixture.0.join("b/repo"));
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    for max_entries in [5, 25, 10_000] {
        let mut results = Vec::new();
        for workers in [1, 4] {
            let report = scan_scope_with(
                &scope,
                &DiscoveryOptions {
                    workers,
                    max_entries,
                    ..DiscoveryOptions::default()
                },
                &LocalFilesystem,
            )
            .unwrap_or_else(|e| panic!("scan: {e}"));
            let mut value = serde_json::to_value(report).unwrap_or_else(|e| panic!("json: {e}"));
            let object = value
                .as_object_mut()
                .unwrap_or_else(|| panic!("inventory object"));
            object.remove("started_unix_ms");
            object.remove("finished_unix_ms");
            let metrics = value["coverage"]["discovery_metrics"]
                .as_object_mut()
                .unwrap_or_else(|| panic!("discovery metrics"));
            metrics.remove("elapsed_ms");
            metrics.remove("workers");
            results.push(value);
        }
        assert_eq!(
            results[0], results[1],
            "different results at budget {max_entries}"
        );
    }
}

struct DisappearingVolume {
    repo: PathBuf,
    volume: PathBuf,
    reads: std::cell::Cell<usize>,
}
impl DiscoveryProvider for DisappearingVolume {
    fn mounted_roots(&self) -> std::io::Result<Vec<MountedRoot>> {
        let mut mounts = vec![MountedRoot {
            path: self.repo.clone(),
            available: true,
            kind: "fixture".into(),
            native: None,
        }];
        if self.reads.replace(self.reads.get() + 1) == 0 {
            mounts.push(MountedRoot {
                path: self.volume.clone(),
                available: true,
                kind: "fixture".into(),
                native: None,
            });
        }
        Ok(mounts)
    }
    fn mount_inventory_complete(&self) -> bool {
        true
    }
}

#[test]
fn disappeared_mount_invalidates_registered_path_availability() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    let volume = fixture.0.join("volume");
    let linked = volume.join("linked");
    fixture.repo(&repo);
    fs::create_dir(&volume).unwrap_or_else(|e| panic!("volume: {e}"));
    fixture.git(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    let report = scan_scope_with(
        &ScanScope::AllMounted,
        &DiscoveryOptions::default(),
        &DisappearingVolume {
            repo,
            volume: volume.clone(),
            reads: std::cell::Cell::new(0),
        },
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    let tree = report
        .families
        .iter()
        .flat_map(|f| &f.worktrees)
        .find(|tree| tree.path.to_path().ok() == Some(linked.clone()))
        .unwrap_or_else(|| panic!("registered worktree missing"));
    assert!(matches!(
        tree.path_state,
        wt_sweep::model::DirState::VolumeUnavailable { .. }
    ));
    assert!(
        report
            .coverage
            .volumes
            .iter()
            .any(|v| v.path.to_path().ok() == Some(volume.clone()) && !v.available)
    );
    assert!(!report.coverage.traversal_complete);
}

struct VolumesProvider {
    root: PathBuf,
    calls: std::cell::Cell<usize>,
}
impl DiscoveryProvider for VolumesProvider {
    fn mounted_roots(&self) -> std::io::Result<Vec<MountedRoot>> {
        let count = self.calls.get();
        self.calls.set(count + 1);
        Ok(vec![
            MountedRoot {
                path: self.root.clone(),
                available: true,
                kind: "fixture_volume".into(),
                native: None,
            },
            MountedRoot {
                path: self.root.join("unmounted"),
                available: count != 0,
                kind: "fixture_volume".into(),
                native: None,
            },
        ])
    }
    fn mount_inventory_complete(&self) -> bool {
        true
    }
}
#[test]
fn all_mode_uses_injected_mounts_and_reports_unavailable_and_changed_volumes() {
    let fixture = Fixture::new();
    fixture.repo(&fixture.0.join("repo"));
    let provider = VolumesProvider {
        root: fixture.0.clone(),
        calls: std::cell::Cell::new(0),
    };
    let report = scan_scope_with(
        &ScanScope::AllMounted,
        &DiscoveryOptions::default(),
        &provider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(report.families.len(), 1);
    assert!(
        report
            .coverage
            .volumes
            .iter()
            .any(|volume| !volume.available)
    );
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("volume_unavailable"))
    );
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("changed"))
    );
    assert_eq!(provider.calls.get(), 2);
}

struct DatalessProvider;

struct ChangingFamily {
    repo: PathBuf,
    changed: std::cell::Cell<bool>,
}
impl DiscoveryProvider for ChangingFamily {
    fn read_directory(&self, path: &Path, limit: usize) -> std::io::Result<DirectoryListing> {
        if path == self.repo.join(".git") && !self.changed.replace(true) {
            fs::write(
                self.repo.join(".git/refs/heads/main"),
                b"0000000000000000000000000000000000000000\n",
            )?;
        }
        LocalFilesystem.read_directory(path, limit)
    }
}
#[test]
fn repeated_family_observations_keep_later_gaps_and_contradictory_facts() {
    let fixture = Fixture::new();
    fixture.repo(&fixture.0);
    let provider = ChangingFamily {
        repo: fixture.0.clone(),
        changed: std::cell::Cell::new(false),
    };
    let report = scan_scope_with(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&fixture.0)],
        },
        &DiscoveryOptions::default(),
        &provider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(report.families.len(), 1);
    assert!(!report.coverage.traversal_complete);
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("unknown HEAD"))
    );
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("contradictory family observations"))
    );
}

struct DatalessHead {
    inode: u64,
}
impl DiscoveryProvider for DatalessHead {
    fn is_dataless(&self, metadata: &fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        metadata.ino() == self.inode
    }
}
#[test]
fn dataless_git_control_file_refuses_candidate_before_git_validation() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new();
    fixture.repo(&fixture.0);
    let provider = DatalessHead {
        inode: fs::metadata(fixture.0.join(".git/HEAD"))
            .unwrap_or_else(|e| panic!("head metadata: {e}"))
            .ino(),
    };
    let report = scan_scope_with(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&fixture.0)],
        },
        &DiscoveryOptions::default(),
        &provider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(report.families.is_empty());
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("dataless"))
    );
}
impl DiscoveryProvider for DatalessProvider {
    fn is_dataless(&self, _: &fs::Metadata) -> bool {
        true
    }
}
#[test]
fn nonresident_paths_are_refused_before_reads_or_git() {
    let fixture = Fixture::new();
    fixture.repo(&fixture.0.join("repo"));
    let report = scan_scope_with(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&fixture.0)],
        },
        &DiscoveryOptions::default(),
        &DatalessProvider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(report.families.is_empty());
    assert!(
        report
            .coverage
            .gaps
            .iter()
            .any(|gap| gap.reason.contains("dataless"))
    );
}

fn independent_manifest(
    root: &Path,
) -> std::collections::BTreeMap<PathBuf, (u32, u64, i64, i64, Vec<u8>)> {
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    let mut pending = vec![root.to_path_buf()];
    let mut manifest = std::collections::BTreeMap::new();
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap_or_else(|e| panic!("manifest stat: {e}"));
        let bytes = if metadata.is_file() {
            fs::read(&path).unwrap_or_else(|e| panic!("manifest bytes: {e}"))
        } else if metadata.file_type().is_symlink() {
            fs::read_link(&path)
                .unwrap_or_else(|e| panic!("manifest link: {e}"))
                .as_os_str()
                .as_bytes()
                .to_vec()
        } else {
            Vec::new()
        };
        if metadata.is_dir() {
            for entry in fs::read_dir(&path).unwrap_or_else(|e| panic!("manifest walk: {e}")) {
                pending.push(
                    entry
                        .unwrap_or_else(|e| panic!("manifest entry: {e}"))
                        .path(),
                );
            }
        }
        manifest.insert(
            path,
            (
                metadata.mode(),
                metadata.len(),
                metadata.mtime(),
                metadata.mtime_nsec(),
                bytes,
            ),
        );
    }
    manifest
}

#[test]
fn discovers_submodule_family_and_preserves_independent_manifest() {
    let fixture = Fixture::new();
    let source = fixture.0.join("source");
    let scope = fixture.0.join("scope");
    fixture.repo(&source);
    fixture.repo(&scope);
    fixture.git(
        &scope,
        &[
            "-c".as_ref(),
            "protocol.file.allow=always".as_ref(),
            "submodule".as_ref(),
            "add".as_ref(),
            source.as_os_str(),
            "sub".as_ref(),
        ],
    );
    fs::create_dir(scope.join("ignored")).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fs::write(
        scope.join("ignored/witness"),
        b"synthetic ignored witness\n",
    )
    .unwrap_or_else(|e| panic!("witness: {e}"));
    fs::write(scope.join(".gitignore"), b"ignored/\n").unwrap_or_else(|e| panic!("ignore: {e}"));
    let before = independent_manifest(&fixture.0);
    let report = scan_scope(&ScanScope::Roots {
        paths: vec![BytePath::from_path(&scope), BytePath::from_path(&scope)],
    })
    .unwrap_or_else(|e| panic!("scan: {e}"));
    let actual: BTreeSet<_> = report
        .families
        .iter()
        .map(|family| {
            family
                .common_dir
                .to_path()
                .unwrap_or_else(|e| panic!("path: {e}"))
        })
        .collect();
    assert_eq!(
        actual,
        [scope.join(".git"), scope.join(".git/modules/sub")]
            .into_iter()
            .collect()
    );
    assert_eq!(report.families.len(), 2);
    assert!(
        report.coverage.traversal_complete,
        "{:?}",
        report.coverage.gaps
    );
    assert_eq!(independent_manifest(&fixture.0), before);
}

struct CancelAfterFinalRead {
    cancelled: std::cell::Cell<bool>,
}
impl DiscoveryProvider for CancelAfterFinalRead {
    fn read_directory(&self, _: &Path, _: usize) -> std::io::Result<DirectoryListing> {
        self.cancelled.set(true);
        Ok(DirectoryListing {
            entries: Vec::new(),
            complete: true,
            errors: Vec::new(),
        })
    }
    fn cancelled(&self) -> bool {
        self.cancelled.get()
    }
}

#[test]
fn cancellation_during_final_empty_directory_read_preserves_remaining_cover() {
    let fixture = Fixture::new();
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    let provider = CancelAfterFinalRead {
        cancelled: std::cell::Cell::new(false),
    };
    let inventory = scan_scope_with(&scope, &DiscoveryOptions::default(), &provider)
        .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(
        !inventory.coverage.traversal_complete,
        "last operation cancellation was lost"
    );
    let remaining = inventory
        .coverage
        .remaining
        .as_ref()
        .unwrap_or_else(|| panic!("remaining cover missing"));
    assert_eq!(
        remaining.reason,
        wt_sweep::model::DiscoveryStop::Interrupted
    );
    assert_eq!(
        remaining.cover,
        wt_sweep::model::RemainingCover::RequestedScope
    );
    assert_eq!(
        inventory.coverage.roots,
        vec![BytePath::from_path(&fixture.0)]
    );
}

#[test]
fn progress_starts_before_io_and_finishes_with_prefetched_pending_scope() {
    use wt_sweep::discover::{DiscoveryPhase, scan_scope_with_progress};
    let fixture = Fixture::new();
    for name in ["a", "b", "c"] {
        fs::create_dir(fixture.0.join(name)).unwrap_or_else(|e| panic!("mkdir: {e}"));
    }
    let started = std::cell::Cell::new(false);
    let cancel = std::cell::Cell::new(false);
    struct Provider<'a> {
        started: &'a std::cell::Cell<bool>,
        cancel: &'a std::cell::Cell<bool>,
    }
    impl DiscoveryProvider for Provider<'_> {
        fn canonicalize(&self, path: &Path) -> std::io::Result<PathBuf> {
            assert!(self.started.get(), "scope I/O preceded start progress");
            fs::canonicalize(path)
        }
        fn cancelled(&self) -> bool {
            self.cancel.get()
        }
        fn metadata_batch(
            &self,
            paths: &[PathBuf],
            _: usize,
        ) -> Vec<std::io::Result<fs::Metadata>> {
            let result = paths.iter().map(|path| self.metadata(path)).collect();
            if paths.len() >= 2 {
                self.cancel.set(true);
            }
            result
        }
    }
    let mut events = Vec::new();
    let inventory = scan_scope_with_progress(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&fixture.0)],
        },
        &DiscoveryOptions {
            workers: 3,
            ..DiscoveryOptions::default()
        },
        &Provider {
            started: &started,
            cancel: &cancel,
        },
        &mut |event| {
            if event.phase == DiscoveryPhase::Start {
                started.set(true);
            }
            events.push(event.clone());
        },
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(
        events.first().map(|e| &e.phase),
        Some(&DiscoveryPhase::Start)
    );
    let last = events
        .last()
        .unwrap_or_else(|| panic!("terminal progress absent"));
    assert_eq!(last.phase, DiscoveryPhase::Finished);
    assert_eq!(last.stop, Some(wt_sweep::model::DiscoveryStop::Interrupted));
    assert!(last.metadata_ready_paths >= 2);
    assert!(
        inventory
            .coverage
            .remaining
            .as_ref()
            .is_some_and(|r| r.known_pending_paths >= 2)
    );
}

#[test]
fn directory_budget_stops_globally_and_depth_only_leaves_subtree_gaps() {
    let fixture = Fixture::new();
    for name in ["a", "b", "c"] {
        fs::create_dir(fixture.0.join(name)).unwrap_or_else(|e| panic!("mkdir: {e}"));
    }
    struct Provider(std::cell::Cell<usize>);
    impl DiscoveryProvider for Provider {
        fn read_directory(&self, path: &Path, limit: usize) -> std::io::Result<DirectoryListing> {
            self.0.set(self.0.get() + 1);
            LocalFilesystem.read_directory(path, limit)
        }
    }
    let provider = Provider(std::cell::Cell::new(0));
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    let limited = scan_scope_with(
        &scope,
        &DiscoveryOptions {
            max_directories: 1,
            ..DiscoveryOptions::default()
        },
        &provider,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(provider.0.get(), 1);
    assert_eq!(
        limited.coverage.remaining.as_ref().map(|r| &r.reason),
        Some(&wt_sweep::model::DiscoveryStop::DirectoryBudget)
    );
    assert_eq!(
        limited.coverage.gaps.len(),
        1,
        "one global stop must replace one gap per pending directory"
    );
    let depth = scan_scope_with(
        &scope,
        &DiscoveryOptions {
            max_depth: 0,
            ..DiscoveryOptions::default()
        },
        &LocalFilesystem,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(depth.coverage.remaining.is_none());
    assert_eq!(depth.coverage.gaps.len(), 3);
}

#[test]
fn observation_budget_retains_crossing_family_and_stops_before_more_io() {
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fixture.repo(&repo);
    let inventory = scan_scope_with(
        &ScanScope::Repo {
            path: BytePath::from_path(&repo),
        },
        &DiscoveryOptions {
            max_observation_bytes: 1,
            ..DiscoveryOptions::default()
        },
        &LocalFilesystem,
    )
    .unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(
        inventory.families.len(),
        1,
        "crossing bounded observation must remain present"
    );
    assert_eq!(inventory.families[0].worktrees.len(), 1);
    assert_eq!(
        inventory.coverage.remaining.as_ref().map(|r| &r.reason),
        Some(&wt_sweep::model::DiscoveryStop::ObservationBudget)
    );
    assert!(!inventory.coverage.traversal_complete);
}

#[test]
fn discovery_metrics_count_actual_walk_work_even_when_incomplete() {
    let fixture = Fixture::new();
    for directory in ["data", ".hidden"] {
        fs::create_dir(fixture.0.join(directory))
            .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
        fs::write(fixture.0.join(directory).join("leaf"), b"fixture")
            .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
    }
    fs::write(fixture.0.join("root.txt"), b"fixture")
        .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    for workers in [1, 4] {
        let options = DiscoveryOptions {
            workers,
            ..DiscoveryOptions::default()
        };
        let started = std::time::Instant::now();
        let inventory = scan_scope_with(&scope, &options, &LocalFilesystem)
            .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
        let value =
            serde_json::to_value(&inventory).unwrap_or_else(|e| panic!("fixture or metric: {e}"));
        let metrics = &value["coverage"]["discovery_metrics"];
        assert_eq!(metrics["directory_read_attempts"], 3);
        assert_eq!(metrics["entries_listed"], 5);
        assert_eq!(metrics["workers"], workers);
        assert!(
            metrics["elapsed_ms"]
                .as_u64()
                .unwrap_or_else(|| panic!("missing elapsed time"))
                <= started.elapsed().as_millis() as u64
        );
        assert!(inventory.coverage.traversal_complete);
        let limited = scan_scope_with(
            &scope,
            &DiscoveryOptions {
                max_directories: 1,
                ..options
            },
            &LocalFilesystem,
        )
        .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
        let value =
            serde_json::to_value(&limited).unwrap_or_else(|e| panic!("fixture or metric: {e}"));
        assert_eq!(
            value["coverage"]["discovery_metrics"]["directory_read_attempts"],
            1
        );
        assert_eq!(value["coverage"]["discovery_metrics"]["entries_listed"], 3);
        assert!(!limited.coverage.traversal_complete);
        assert!(limited.coverage.remaining.is_some());
        let report = wt_sweep::output::inventory_table(&limited);
        assert!(report.contains("DISCOVERY PERFORMANCE"), "{report}");
        assert!(report.contains("incomplete"), "{report}");
    }
}

#[test]
fn repo_metrics_do_not_claim_a_filesystem_walk_and_legacy_reports_still_read() {
    let fixture = Fixture::new();
    fixture.repo(&fixture.0);
    let inventory = scan_scope(&ScanScope::Repo {
        path: BytePath::from_path(&fixture.0),
    })
    .unwrap_or_else(|e| panic!("fixture or metric: {e}"));
    let mut json =
        serde_json::to_value(&inventory).unwrap_or_else(|e| panic!("fixture or metric: {e}"));
    assert_eq!(
        json["coverage"]["discovery_metrics"]["directory_read_attempts"],
        0
    );
    assert_eq!(json["coverage"]["discovery_metrics"]["entries_listed"], 0);
    assert_eq!(inventory.families.len(), 1);
    json["coverage"]
        .as_object_mut()
        .unwrap_or_else(|| panic!("coverage object"))
        .remove("discovery_metrics");
    let legacy: wt_sweep::model::Inventory =
        serde_json::from_value(json).unwrap_or_else(|e| panic!("fixture or metric: {e}"));
    assert!(!wt_sweep::output::inventory_table(&legacy).contains("DISCOVERY PERFORMANCE"));
}

/// Opt-in measurement: no timing threshold, and every sample must find the exact corpus.
#[test]
#[ignore = "run the discovery benchmark explicitly with --release --ignored --nocapture"]
fn benchmark_discovery() {
    if cfg!(debug_assertions) {
        panic!("use cargo test --release for comparable measurements");
    }
    let fixture = Fixture::new();
    let mut expected = BTreeSet::new();
    for relative in ["repo", ".hidden/repo"] {
        let repo = fixture.0.join(relative);
        fixture.repo(&repo);
        let linked = repo.with_file_name("linked");
        fixture.git(
            &repo,
            &[
                "worktree".as_ref(),
                "add".as_ref(),
                "--detach".as_ref(),
                linked.as_os_str(),
            ],
        );
        expected.extend([repo, linked]);
    }
    for directory in 0..64 {
        let path = fixture.0.join(format!("tree/directory-{directory:03}"));
        fs::create_dir_all(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        for file in 0..32 {
            fs::write(
                path.join(format!("file-{file:03}")),
                format!("synthetic {directory} {file}\n"),
            )
            .unwrap_or_else(|e| panic!("fixture: {e}"));
        }
    }
    fn manifest(path: &Path) -> std::collections::BTreeMap<PathBuf, (u32, u64, i64, i64, Vec<u8>)> {
        use std::os::unix::fs::MetadataExt;
        let mut found = std::collections::BTreeMap::new();
        let mut queue = vec![path.to_path_buf()];
        while let Some(path) = queue.pop() {
            let meta = fs::symlink_metadata(&path).unwrap_or_else(|e| panic!("manifest: {e}"));
            let bytes = if meta.is_file() {
                fs::read(&path).unwrap_or_else(|e| panic!("read fixture: {e}"))
            } else {
                Vec::new()
            };
            if meta.is_dir() {
                for entry in fs::read_dir(&path).unwrap_or_else(|e| panic!("fixture dir: {e}")) {
                    queue.push(
                        entry
                            .unwrap_or_else(|e| panic!("fixture entry: {e}"))
                            .path(),
                    );
                }
            }
            assert!(
                !meta.file_type().is_symlink(),
                "unexpected symlink in synthetic corpus"
            );
            found.insert(
                path,
                (
                    meta.mode(),
                    meta.ino(),
                    meta.mtime(),
                    meta.mtime_nsec(),
                    bytes,
                ),
            );
        }
        found
    }
    let before = manifest(&fixture.0);
    let scope = ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    };
    let mut samples = [Vec::new(), Vec::new()];
    let mut observed_work = None;
    println!(
        "Discovery benchmark: {} {}; release; 2 families, 4 worktrees, 2048 synthetic files; 1 warm-up per worker count, 3 measured samples; OS cache not cleared",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for round in 0..4 {
        // Alternate ordering to avoid always giving the same variant the first run.
        for workers in if round % 2 == 0 { [1, 4] } else { [4, 1] } {
            let start = std::time::Instant::now();
            let inventory = scan_scope_with(
                &scope,
                &DiscoveryOptions {
                    workers,
                    ..DiscoveryOptions::default()
                },
                &LocalFilesystem,
            )
            .unwrap_or_else(|e| panic!("scan: {e}"));
            let elapsed = start.elapsed().as_secs_f64();
            assert!(
                inventory.coverage.traversal_complete,
                "incomplete sample cannot count as faster"
            );
            assert!(inventory.coverage.gaps.is_empty());
            assert!(inventory.coverage.remaining.is_none());
            assert_eq!(inventory.families.len(), 2);
            let paths: BTreeSet<_> = inventory
                .families
                .iter()
                .flat_map(|f| &f.worktrees)
                .map(|w| w.path.to_path().unwrap_or_else(|e| panic!("path: {e}")))
                .collect();
            assert_eq!(paths, expected);
            let metrics = inventory
                .coverage
                .discovery_metrics
                .as_ref()
                .unwrap_or_else(|| panic!("missing metrics"));
            let work = (metrics.directory_read_attempts, metrics.entries_listed);
            if let Some(expected_work) = observed_work {
                assert_eq!(work, expected_work);
            }
            observed_work = Some(work);
            if round > 0 {
                samples[usize::from(workers == 4)].push(elapsed);
            }
            assert_eq!(
                manifest(&fixture.0),
                before,
                "benchmark changed fixture bytes or metadata"
            );
        }
    }
    let (directories, entries) = observed_work.unwrap_or_else(|| panic!("no work measured"));
    let mut medians = Vec::new();
    for (workers, times) in [1, 4].into_iter().zip(&mut samples) {
        times.sort_by(f64::total_cmp);
        let median = times[1];
        println!(
            "workers={workers}: min={:.3}s median={median:.3}s max={:.3}s; {directories} directory attempts; {entries} entries; {:.1} entries/s; coverage complete",
            times[0],
            times[2],
            entries as f64 / median
        );
        medians.push(median);
    }
    println!(
        "workers=4 / workers=1 median duration ratio: {:.2} (below 1 means faster on this corpus)",
        medians[1] / medians[0]
    );
}
