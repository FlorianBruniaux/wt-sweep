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
