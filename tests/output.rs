#![allow(clippy::expect_used)]
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use wt_sweep::{
    model::{BytePath, Family, Identity, Inventory, ScanScope},
    output::write_new,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(format!(
            "/private/tmp/wt-sweep-output-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&p).expect("owned fixture");
        Self(p)
    }
    fn inventory(&self) -> Inventory {
        let repo = self.0.join("repo");
        fs::create_dir(&repo).expect("repo");
        let common = repo.join(".git");
        fs::create_dir(&common).expect("admin");
        let mut i = Inventory::new(ScanScope::Repo {
            path: BytePath::from_path(&repo),
        });
        i.families.push(Family {
            common_dir: BytePath::from_path(&common),
            identity: Identity {
                device: 0,
                inode: 0,
            },
            worktrees: Vec::new(),
        });
        i
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn output_creates_exact_bytes_and_never_replaces_existing_file() {
    let f = Fixture::new();
    let i = f.inventory();
    let out = f.0.join("inventory.json");
    write_new(&out, b"{\"observed\":true}", &i).expect("outside output");
    assert_eq!(fs::read(&out).expect("output"), b"{\"observed\":true}");
    assert!(write_new(&out, b"changed", &i).is_err());
    assert_eq!(fs::read(&out).expect("preserved"), b"{\"observed\":true}");
}

#[test]
fn output_refuses_target_administration_and_symlink_ancestor() {
    let f = Fixture::new();
    let i = f.inventory();
    for relative in ["repo/report.json", "repo/.git/report.json"] {
        let out = f.0.join(relative);
        assert!(write_new(&out, b"bad", &i).is_err());
        assert!(!out.exists());
    }
    std::os::unix::fs::symlink(f.0.join("repo"), f.0.join("alias")).expect("symlink");
    assert!(write_new(&f.0.join("alias/report.json"), b"bad", &i).is_err());
    assert!(!f.0.join("repo/report.json").exists());
}

#[test]
fn global_partial_inventory_does_not_authorize_output_inside_an_unobserved_marker() {
    let fixture = Fixture::new();
    let mut inventory = fixture.inventory();
    inventory.scope = ScanScope::AllMounted;
    inventory.families.clear();
    inventory.coverage.traversal_complete = false;
    let forbidden = fixture.0.join("repo/report.json");
    assert!(write_new(&forbidden, b"must not appear", &inventory).is_err());
    assert!(!forbidden.exists());
    let bare = fixture.0.join("unknown-administration");
    fs::create_dir(&bare).expect("admin");
    fs::write(bare.join("HEAD"), b"ref: refs/heads/main\n").expect("head");
    fs::create_dir(bare.join("objects")).expect("objects");
    assert!(write_new(&bare.join("report.json"), b"forbidden", &inventory).is_err());
    let allowed = fixture.0.join("outside-report.json");
    write_new(&allowed, b"explicit report", &inventory).expect("outside marker");
    assert_eq!(fs::read(allowed).expect("report"), b"explicit report");
}

#[test]
fn bounded_artifacts_charge_final_newline_and_refuse_ignored_text_overflow() {
    use std::fmt::Write;
    use wt_sweep::output::{BoundedText, bounded_json};
    let value = serde_json::json!({"path":"a\n\\b"});
    let exact = b"{\n  \"path\": \"a\\n\\\\b\"\n}\n";
    assert_eq!(bounded_json(&value, exact.len()).expect("exact fit"), exact);
    assert!(
        bounded_json(&value, exact.len() - 1).is_err(),
        "newline must count toward limit"
    );
    let mut text = BoundedText::new(3);
    let _ = text.write_str("abcd");
    assert!(
        text.finish().is_err(),
        "ignored fmt errors must still prevent successful text"
    );
}

fn rich_inventory() -> Inventory {
    use wt_sweep::model::{DirState, LocalInventory, Marker, Worktree};
    let mut inventory = Inventory::new(ScanScope::Roots {
        paths: vec![BytePath::from_bytes(b"/synthetic/scope")],
    });
    inventory.coverage.traversal_complete = true;
    inventory.coverage.roots = vec![BytePath::from_bytes(b"/synthetic/scope")];
    for family_index in 0..8 {
        let common = BytePath::from_bytes(format!("/synthetic/repo{family_index}/.git").as_bytes());
        let mut family = Family {
            common_dir: common.clone(),
            identity: Identity {
                device: 9,
                inode: family_index,
            },
            worktrees: Vec::new(),
        };
        for row in 0..3 {
            family.worktrees.push(Worktree {
                path: BytePath::from_bytes(
                    format!("/outside/registered-{family_index}-{row}\n\t").as_bytes(),
                ),
                identity: Some(Identity {
                    device: 9,
                    inode: 100 + family_index * 3 + row,
                }),
                head: Some("0123456789abcdef".repeat(4)),
                branch: Some(BytePath::from_bytes(b"refs/heads/positive")),
                detached: false,
                bare: false,
                locked: Some("retention reason".into()),
                prunable: None,
                path_state: DirState::Present,
            });
        }
        inventory.families.push(family);
        inventory.markers.push(Marker {
            path: BytePath::from_bytes(format!("/outside/registered-{family_index}-0").as_bytes()),
            common_dir: Some(common),
            kind: "registered_worktree".into(),
            origin: "git_registry_outside_roots".into(),
        });
    }
    inventory.local_inventories.push(LocalInventory {
        worktree: inventory.families[0].worktrees[0].path.clone(),
        started_unix_ms: 1,
        finished_unix_ms: 2,
        entries: Vec::new(),
        index_entries: Vec::new(),
        administration_entries: Vec::new(),
        traversal_complete: false,
        temporal_consistency: "not_guaranteed".into(),
        gaps: Vec::new(),
        not_collected: vec!["optional detail".repeat(10000)],
        content_digest_sha256: "a".repeat(64),
    });
    inventory
}

#[test]
fn compact_inventory_preserves_positive_rows_and_counts_omitted_observations() {
    use wt_sweep::output::{PublicationLimits, prepare_inventory};
    let inventory = rich_inventory();
    let original = serde_json::to_vec(&inventory).expect("source");
    let limits = PublicationLimits {
        json_bytes: 4200,
        report_bytes: 4200,
        metadata_reserve_bytes: 512,
    };
    let prepared = prepare_inventory(&inventory, &limits).expect("compact inventory");
    assert!(prepared.json.len() <= 4200);
    assert!(prepared.report.len() <= 4200);
    assert!(prepared.partial);
    assert!(
        prepared.inventory.coverage.traversal_complete,
        "publication omission is distinct from completed discovery"
    );
    let omissions = prepared
        .inventory
        .coverage
        .publication
        .as_ref()
        .expect("omission counts");
    assert_eq!(omissions.local_inventories, 1);
    assert_eq!(omissions.markers, 8);
    assert!(omissions.families > 0);
    let retained_rows: usize = prepared
        .inventory
        .families
        .iter()
        .map(|family| family.worktrees.len())
        .sum();
    assert!(retained_rows > 0, "a real positive prefix must fit");
    assert_eq!(omissions.worktrees + retained_rows, 24);
    assert_eq!(omissions.families + prepared.inventory.families.len(), 8);
    assert_eq!(
        omissions.cover,
        wt_sweep::model::PublicationCover::RequestedScopeAndRegisteredWorktrees
    );
    for (family, source) in prepared.inventory.families.iter().zip(&inventory.families) {
        assert_eq!(family.common_dir, source.common_dir);
        assert_eq!(family.identity, source.identity);
        for (row, before) in family.worktrees.iter().zip(&source.worktrees) {
            assert_eq!(
                serde_json::to_value(row).expect("row"),
                serde_json::to_value(before).expect("source row")
            );
        }
    }
    assert!(prepared.report.contains("Publication omitted:"));
    assert!(
        prepared
            .report
            .contains("requested_scope_and_registered_worktrees")
    );
    assert_eq!(
        original,
        serde_json::to_vec(&inventory).expect("unchanged source")
    );
    assert_eq!(
        serde_json::from_slice::<Inventory>(&prepared.json)
            .expect("valid artifact")
            .families
            .len(),
        prepared.inventory.families.len()
    );
}

#[test]
fn mandatory_coverage_refuses_when_it_cannot_fit_and_text_can_trigger_compaction() {
    use wt_sweep::model::Gap;
    use wt_sweep::output::{PublicationLimits, prepare_inventory};
    let mut inventory = rich_inventory();
    inventory.coverage.gaps.push(Gap {
        path: BytePath::from_bytes(b"/synthetic/denied"),
        reason: "mandatory error".repeat(1000),
    });
    assert!(
        prepare_inventory(
            &inventory,
            &PublicationLimits {
                json_bytes: 4200,
                report_bytes: 4200,
                metadata_reserve_bytes: 512
            }
        )
        .is_err()
    );
    inventory.coverage.gaps[0].reason = "mandatory error".repeat(60);
    let with_error = prepare_inventory(
        &inventory,
        &PublicationLimits {
            json_bytes: 4200,
            report_bytes: 4200,
            metadata_reserve_bytes: 512,
        },
    )
    .expect("mandatory coverage can exceed reserve while fitting total limit");
    assert_eq!(
        with_error.inventory.coverage.gaps[0].reason,
        "mandatory error".repeat(60)
    );
    inventory.coverage.gaps.clear();
    inventory.local_inventories.clear();
    let compact = prepare_inventory(
        &inventory,
        &PublicationLimits {
            json_bytes: 32000,
            report_bytes: 1400,
            metadata_reserve_bytes: 512,
        },
    )
    .expect("report-bound compaction");
    assert!(compact.report.len() <= 1400);
    assert!(compact.partial);
    assert!(
        compact
            .inventory
            .coverage
            .publication
            .as_ref()
            .is_some_and(|o| o.worktrees > 0)
    );
}

fn git_fixture(cwd: &std::path::Path, args: &[&std::ffi::OsStr]) {
    let result = std::process::Command::new("/usr/bin/git")
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
        .expect("fixture git");
    assert!(result.status.success(), "{:?}", result.stderr);
}
fn actual_repo(fixture: &Fixture) -> PathBuf {
    let repo = fixture.0.join("actual-repo");
    fs::create_dir(&repo).expect("repo");
    git_fixture(&repo, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
    git_fixture(
        &repo,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    repo
}

#[test]
fn original_boundaries_union_new_outside_registration_before_compacted_publication() {
    use wt_sweep::output::{PublicationLimits, prepare_inventory, revalidate_boundaries};
    let fixture = Fixture::new();
    let repo = actual_repo(&fixture);
    let inventory = wt_sweep::git::scan_family(&repo).expect("inventory");
    let new_linked = fixture.0.join("outside-new-worktree");
    git_fixture(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "new".as_ref(),
            new_linked.as_os_str(),
        ],
    );
    let fresh = revalidate_boundaries(&inventory).expect("revalidated full original");
    assert!(
        fresh
            .families
            .iter()
            .flat_map(|family| &family.worktrees)
            .any(|row| row.path.to_path().ok().as_ref() == Some(&new_linked)),
        "new outside-root registration was omitted"
    );
    assert!(write_new(&new_linked.join("forbidden.json"), b"bad", &fresh).is_err());
    let prepared = prepare_inventory(&inventory, &PublicationLimits::default()).expect("prepare");
    assert_eq!(prepared.inventory.families[0].worktrees.len(), 1);
    write_new(&fixture.0.join("allowed.json"), &prepared.json, &fresh)
        .expect("outside-scope positive publication");
}

#[test]
fn missing_family_and_relative_saved_scope_refuse_publication_revalidation() {
    use wt_sweep::output::revalidate_boundaries;
    let fixture = Fixture::new();
    let repo = actual_repo(&fixture);
    let inventory = wt_sweep::git::scan_family(&repo).expect("inventory");
    fs::rename(&repo, fixture.0.join("moved-repo")).expect("move owned fixture");
    assert!(
        revalidate_boundaries(&inventory).is_err(),
        "missing original family must refuse"
    );
    let mut relative = Inventory::new(ScanScope::Roots {
        paths: vec![BytePath::from_bytes(b".")],
    });
    assert!(
        revalidate_boundaries(&relative).is_err(),
        "saved scope has no known original cwd"
    );
    relative.scope = ScanScope::Repo {
        path: BytePath::from_bytes(b"relative"),
    };
    assert!(revalidate_boundaries(&relative).is_err());
    relative.scope = ScanScope::Repo {
        path: BytePath::from_bytes(b"/synthetic/unreadable"),
    };
    relative.coverage.remaining = Some(wt_sweep::model::DiscoveryRemainder {
        reason: wt_sweep::model::DiscoveryStop::Interrupted,
        known_pending_paths: 1,
        cover: wt_sweep::model::RemainingCover::RequestedScope,
    });
    assert!(
        revalidate_boundaries(&relative).is_err(),
        "interrupted empty-family repo lacks boundaries"
    );
}

#[test]
fn saved_row_omissions_refuse_while_full_original_and_marker_only_boundaries_publish() {
    use wt_sweep::model::{PublicationCover, PublicationOmissions};
    use wt_sweep::output::{PublicationLimits, prepare_inventory, revalidate_boundaries};
    let fixture = Fixture::new();
    let repo = actual_repo(&fixture);
    let original = wt_sweep::git::scan_family(&repo).expect("original inventory");
    let mut omitted_rows = original.clone();
    omitted_rows.coverage.publication = Some(PublicationOmissions {
        local_inventories: 0,
        markers: 0,
        families: 0,
        worktrees: 1,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    assert!(
        revalidate_boundaries(&omitted_rows).is_err(),
        "historical omitted outside-root rows are unavailable"
    );
    omitted_rows
        .coverage
        .publication
        .as_mut()
        .expect("omissions")
        .families = 1;
    omitted_rows
        .coverage
        .publication
        .as_mut()
        .expect("omissions")
        .worktrees = 0;
    assert!(
        revalidate_boundaries(&omitted_rows).is_err(),
        "historical omitted families are unavailable"
    );
    let mut mandatory = original.clone();
    mandatory.families.clear();
    mandatory.coverage.publication = Some(PublicationOmissions {
        local_inventories: 0,
        markers: 0,
        families: 1,
        worktrees: 1,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    let exact_mandatory = serde_json::to_vec_pretty(&mandatory)
        .expect("independent essential byte count")
        .len()
        + 1;
    let prepared = prepare_inventory(
        &original,
        &PublicationLimits {
            json_bytes: exact_mandatory + 16,
            report_bytes: 4096,
            metadata_reserve_bytes: 512,
        },
    )
    .expect("whole family omission");
    assert!(prepared.inventory.families.is_empty());
    let outside = fixture.0.join("outside-late-registration");
    git_fixture(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "late".as_ref(),
            outside.as_os_str(),
        ],
    );
    assert!(revalidate_boundaries(&prepared.inventory).is_err());
    let full =
        revalidate_boundaries(&original).expect("full original still revalidates omitted family");
    assert!(write_new(&outside.join("forbidden.json"), &prepared.json, &full).is_err());
    let mut marker_only = original.clone();
    marker_only.coverage.publication = Some(PublicationOmissions {
        local_inventories: 1,
        markers: 1,
        families: 0,
        worktrees: 0,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    let boundary = revalidate_boundaries(&marker_only)
        .expect("optional-only omissions retain full boundaries");
    write_new(
        &fixture.0.join("allowed-marker-only.json"),
        &prepared.json,
        &boundary,
    )
    .expect("outside positive");
}

#[test]
fn incomplete_roots_refuse_output_in_unobserved_separate_administration() {
    use wt_sweep::discover::{DiscoveryOptions, DiscoveryProvider, scan_scope_with};
    let fixture = Fixture::new();
    let scope_root = fixture.0.join("scope");
    fs::create_dir(&scope_root).expect("scope");
    let repo = scope_root.join("repo");
    fs::create_dir(&repo).expect("repo");
    let administration = fixture.0.join("outside-administration");
    git_fixture(
        &repo,
        &[
            "init".as_ref(),
            "-b".as_ref(),
            "main".as_ref(),
            "--separate-git-dir".as_ref(),
            administration.as_os_str(),
        ],
    );
    git_fixture(
        &repo,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    struct Cancelled;
    impl DiscoveryProvider for Cancelled {
        fn cancelled(&self) -> bool {
            true
        }
    }
    let inventory = scan_scope_with(
        &ScanScope::Roots {
            paths: vec![BytePath::from_path(&scope_root)],
        },
        &DiscoveryOptions::default(),
        &Cancelled,
    )
    .expect("partial scan");
    assert!(inventory.families.is_empty());
    assert!(inventory.coverage.remaining.is_some());
    let forbidden = administration.join("report.json");
    assert!(
        write_new(&forbidden, b"must not appear", &inventory).is_err(),
        "separate administration was missed by partial Roots discovery"
    );
    assert!(!forbidden.exists());
    write_new(
        &fixture.0.join("outside-positive.json"),
        b"safe report",
        &inventory,
    )
    .expect("outside positive");
    let forge = Inventory::new(ScanScope::Roots { paths: Vec::new() });
    let forge_path = repo.join("standalone-forge.json");
    write_new(&forge_path, b"standalone forge bytes", &forge)
        .expect("empty forge scope remains explicit");
}

#[test]
fn separate_administration_positive_revalidation_keeps_primary_worktree_context() {
    use wt_sweep::output::revalidate_boundaries;
    let fixture = Fixture::new();
    let repo = fixture.0.join("repo");
    fs::create_dir(&repo).expect("repo");
    let administration = fixture.0.join("outside-administration");
    git_fixture(
        &repo,
        &[
            "init".as_ref(),
            "-b".as_ref(),
            "main".as_ref(),
            "--separate-git-dir".as_ref(),
            administration.as_os_str(),
        ],
    );
    git_fixture(
        &repo,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let inventory = wt_sweep::git::scan_family(&repo).expect("positive inventory");
    assert_eq!(
        inventory.families[0].common_dir.to_path().expect("common"),
        administration
    );
    let linked = fixture.0.join("linked-outside-root");
    git_fixture(
        &repo,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    let boundaries =
        revalidate_boundaries(&inventory).expect("separate administration must remain publishable");
    assert!(
        boundaries
            .families
            .iter()
            .flat_map(|family| &family.worktrees)
            .any(|row| row.path.to_path().ok().as_ref() == Some(&linked))
    );
    write_new(
        &fixture.0.join("positive.json"),
        b"positive report",
        &boundaries,
    )
    .expect("positive outside publication");
    assert!(write_new(&administration.join("forbidden.json"), b"bad", &boundaries).is_err());
    assert!(write_new(&linked.join("forbidden.json"), b"bad", &boundaries).is_err());
}
