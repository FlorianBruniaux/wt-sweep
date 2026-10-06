#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use wt_sweep::model::*;
fn candidate() -> Observations {
    let path = BytePath::from_bytes(b"/private/tmp/synthetic-linked");
    let common = BytePath::from_bytes(b"/private/tmp/synthetic-main/.git");
    let dir = BytePath::from_bytes(b"/private/tmp/synthetic-main/.git/worktrees/linked");
    let id = Identity {
        device: 1,
        inode: 2,
    };
    let head = "a".repeat(40);
    let tree = Worktree {
        path: path.clone(),
        identity: Some(id.clone()),
        head: Some(head.clone()),
        branch: Some(BytePath::from_bytes(b"refs/heads/topic")),
        detached: false,
        bare: false,
        locked: None,
        prunable: None,
        path_state: DirState::Present,
    };
    let mut inventory = Inventory::new(ScanScope::Repo { path: path.clone() });
    inventory.families.push(Family {
        common_dir: common.clone(),
        identity: id.clone(),
        worktrees: vec![tree.clone()],
    });
    let zero = LocalCount {
        observed: 0,
        complete: true,
        ..Default::default()
    };
    let anchor = RetainedAnchor {
        reference: BytePath::from_bytes(b"refs/heads/main"),
        oid: head.clone(),
    };
    Observations {
        context: None,
        inventory,
        shared_stashes: vec![],
        history_metrics: Default::default(),
        forge: None,
        started_unix_ms: 100,
        finished_unix_ms: 200,
        items: vec![ReviewItem {
            path,
            branch: tree.branch.clone(),
            head: Some(head.clone()),
            subject: None,
            subject_truncated: false,
            classification: "unknown".into(),
            requests: vec![],
            risks: vec![],
            unknowns: vec!["forge uncollected".into()],
            decision: None,
            worktree_state: Some(WorktreeState {
                observed: Some(tree),
                common_dir: common,
                family_identity: Some(id.clone()),
                git_dir: Some(dir.clone()),
                git_dir_identity: Some(id.clone()),
                role: "linked".into(),
                protections: vec![],
                protection_complete: true,
                operations: vec![],
                operations_complete: true,
                checks: "complete".into(),
                started_unix_ms: 101,
                finished_unix_ms: 199,
                sources: vec![],
                gaps: vec![],
            }),
            local_summary: Some(LocalSummary {
                worktree_identity: Some(id.clone()),
                git_dir: Some(dir.clone()),
                head: Some(head.clone()),
                started_unix_ms: 102,
                finished_unix_ms: 103,
                staged: zero.clone(),
                unstaged: zero.clone(),
                conflicts: zero.clone(),
                untracked: zero.clone(),
                ignored: zero,
                head_complete: true,
                index_complete: true,
                disk_complete: true,
                temporal_consistency: "not_guaranteed".into(),
                ..Default::default()
            }),
            observations: vec![IntegrationObservation {
                source_oid: head.clone(),
                target_ref: anchor.reference.clone(),
                target_oid: head.clone(),
                relation: "identical_commit".into(),
                target_role: "integration".into(),
                started_unix_ms: 104,
                finished_unix_ms: 105,
            }],
            history: vec![HistoryObservation {
                source_oid: Some(head.clone()),
                target_ref: anchor.reference.clone(),
                target_oid: Some(head.clone()),
                result: "identical_commit".into(),
                observed_result: None,
                started_unix_ms: 102,
                finished_unix_ms: 198,
                stop_reason: None,
            }],
            history_omitted_targets: 0,
            private_history: Some(PrivateHistory {
                git_dir: Some(dir),
                git_dir_identity: Some(id),
                started_unix_ms: 106,
                finished_unix_ms: 197,
                enumeration_complete: true,
                anchors_complete: true,
                temporal_consistency: "not_guaranteed".into(),
                sources: vec![],
                roots: vec![PrivateRoot {
                    oid: head,
                    provenance: vec![],
                    result: "covered".into(),
                    anchor: Some(anchor),
                    observed_result: None,
                    reason: None,
                }],
                gaps: vec![],
            }),
        }],
    }
}
#[test]
fn all_five_current_conditions_allow_candidate_despite_uncollected_context() {
    let facts = candidate();
    let out = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(out.items[0].classification, "cleanup_candidate");
    assert!(
        out.items[0]
            .decision
            .as_ref()
            .unwrap()
            .missing_conditions
            .is_empty()
    );
}
#[test]
fn each_missing_condition_prevents_promotion() {
    for condition in ["C1", "C2", "C3", "C4", "C5"] {
        let mut f = candidate();
        let i = &mut f.items[0];
        match condition {
            "C1" => i.worktree_state.as_mut().unwrap().protection_complete = false,
            "C2" => i.observations[0].relation = "identical_tree".into(),
            "C3" => i.local_summary.as_mut().unwrap().disk_complete = false,
            "C4" => i.private_history.as_mut().unwrap().enumeration_complete = false,
            "C5" => i.local_summary.as_mut().unwrap().started_unix_ms = 1,
            _ => unreachable!(),
        }
        let out = wt_sweep::review::judge(&f).unwrap();
        assert_eq!(out.items[0].classification, "verify", "{condition}");
        assert!(
            out.items[0]
                .decision
                .as_ref()
                .unwrap()
                .missing_conditions
                .iter()
                .any(|s| s == condition),
            "{condition}"
        );
    }
}
#[test]
fn all_positive_keep_reasons_survive_independent_incompleteness() {
    let mut f = candidate();
    let i = &mut f.items[0];
    let state = i.worktree_state.as_mut().unwrap();
    state.role = "main".into();
    state.observed.as_mut().unwrap().locked = Some("fixture".into());
    state
        .protections
        .push(BytePath::from_bytes(b"/synthetic/index.lock"));
    state
        .operations
        .push(BytePath::from_bytes(b"/synthetic/MERGE_HEAD"));
    state.operations_complete = false;
    let local = i.local_summary.as_mut().unwrap();
    for n in [
        &mut local.staged,
        &mut local.unstaged,
        &mut local.conflicts,
        &mut local.untracked,
        &mut local.ignored,
    ] {
        n.observed = 1;
        n.complete = false;
    }
    i.private_history.as_mut().unwrap().roots[0].result = "not_preserved_by_retained_refs".into();
    i.observations.clear();
    i.history[0].result = "not_ancestor".into();
    let out = wt_sweep::review::judge(&f).unwrap();
    assert_eq!(out.items[0].classification, "keep");
    let reasons = &out.items[0].decision.as_ref().unwrap().keep_reasons;
    for expected in [
        "main_checkout",
        "git_locked",
        "administrative_lock",
        "git_operation",
        "local_staged",
        "local_unstaged",
        "local_conflicts",
        "local_untracked",
        "local_ignored",
        "head_not_integrated",
        "private_history_not_preserved",
    ] {
        assert!(
            reasons.iter().any(|x| x == expected),
            "missing {expected}: {reasons:?}"
        );
    }
}
#[test]
fn stale_or_mismatched_facets_cannot_be_candidates() {
    for domain in 0..4 {
        let mut f = candidate();
        match domain {
            0 => f.items[0].local_summary.as_mut().unwrap().head = Some("b".repeat(40)),
            1 => {
                f.items[0]
                    .private_history
                    .as_mut()
                    .unwrap()
                    .git_dir_identity = Some(Identity {
                    device: 2,
                    inode: 9,
                })
            }
            2 => f.items[0].observations[0].finished_unix_ms = 300,
            3 => f.items[0].worktree_state.as_mut().unwrap().checks = "incomplete".into(),
            _ => unreachable!(),
        };
        assert_eq!(
            wt_sweep::review::judge(&f).unwrap().items[0].classification,
            "verify"
        );
    }
}
use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
    repo: PathBuf,
    linked: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-triage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        let repo = root.join("repo");
        let linked = root.join("linked");
        fs::create_dir(&repo).unwrap();
        let f = Self { root, repo, linked };
        f.git(&f.repo, &["init", "--template=", "-b", "main"]);
        fs::write(f.repo.join("file"), b"synthetic\n").unwrap();
        f.git(&f.repo, &["add", "file"]);
        f.git(&f.repo, &["commit", "-m", "synthetic"]);
        f.git(
            &f.repo,
            &["worktree", "add", "-b", "topic", f.linked.to_str().unwrap()],
        );
        f
    }
    fn git(&self, path: &Path, args: &[&str]) -> String {
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
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "gc.auto=0",
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
                panic!("owned fixture child deadline");
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
    fn admin(&self) -> PathBuf {
        PathBuf::from(self.git(&self.linked, &["rev-parse", "--absolute-git-dir"]))
    }
    fn facts(&self) -> Observations {
        let mut inv = wt_sweep::git::scan_family(&self.repo).unwrap();
        inv.families[0].worktrees.reverse();
        wt_sweep::review::enrich(&inv, &[BytePath::from_bytes(b"refs/heads/main")], None).unwrap()
    }
    fn item<'a>(&self, f: &'a Observations) -> &'a ReviewItem {
        f.items
            .iter()
            .find(|i| i.path == BytePath::from_path(&self.linked))
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn reordered_inventory_still_keeps_main_and_qualifies_linked() {
    let f = Fixture::new();
    let facts = f.facts();
    let r = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(
        r.items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.repo))
            .unwrap()
            .classification,
        "keep"
    );
    assert_eq!(
        r.items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap()
            .classification,
        "cleanup_candidate"
    );
    assert!(!f.item(&facts).observations.is_empty());
}
#[test]
fn administrative_locks_and_operation_markers_are_positive_keep_facts() {
    for name in ["index.lock", "MERGE_HEAD", "rebase-merge"] {
        let f = Fixture::new();
        let p = f.admin().join(name);
        if name == "rebase-merge" {
            fs::create_dir(&p).unwrap();
            fs::write(p.join("unknown-child"), b"synthetic").unwrap();
        } else {
            fs::write(p, b"synthetic\n").unwrap();
        }
        let facts = f.facts();
        let r = wt_sweep::review::judge(&facts).unwrap();
        let i = r
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap();
        assert_eq!(i.classification, "keep", "{name}");
        let code = if name == "index.lock" {
            "administrative_lock"
        } else {
            "git_operation"
        };
        assert!(
            i.decision
                .as_ref()
                .unwrap()
                .keep_reasons
                .iter()
                .any(|s| s == code)
        );
    }
}
struct ControlFault {
    path: PathBuf,
    action: u8,
}
impl wt_sweep::local::LocalHooks for ControlFault {
    fn before_entry(&self, p: &BytePath) -> io::Result<()> {
        if self.action == 2 && p == &BytePath::from_path(&self.path) {
            return Err(io::Error::from_raw_os_error(13));
        }
        Ok(())
    }
}
impl wt_sweep::review::control::ControlHooks for ControlFault {
    fn before_final_controls(&self) -> io::Result<()> {
        match self.action {
            0 => fs::write(&self.path, b"synthetic lock\n")?,
            1 => fs::remove_file(&self.path)?,
            _ => {}
        }
        Ok(())
    }
}
fn controlled(
    f: &Fixture,
    hooks: &dyn wt_sweep::review::control::ControlHooks,
    limits: &wt_sweep::review::control::ControlLimits,
) -> Observations {
    wt_sweep::review::enrich_with_controls(
        &wt_sweep::git::scan_family(&f.repo).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &Default::default(),
        limits,
        hooks,
        &mut |_| {},
    )
    .unwrap()
}
#[test]
fn lock_addition_or_removal_prevents_candidate_and_preserves_positive_presence() {
    for action in [0, 1] {
        let f = Fixture::new();
        let path = f.admin().join("index.lock");
        if action == 1 {
            fs::write(&path, b"synthetic lock\n").unwrap();
        }
        let facts = controlled(&f, &ControlFault { path, action }, &Default::default());
        let item = f.item(&facts);
        assert_eq!(
            item.worktree_state.as_ref().unwrap().checks,
            "change_observed"
        );
        let report = wt_sweep::review::judge(&facts).unwrap();
        let item = report
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap();
        assert_eq!(item.classification, "keep");
        assert!(
            item.decision
                .as_ref()
                .unwrap()
                .keep_reasons
                .contains(&"administrative_lock".into())
        );
    }
}
#[test]
fn symlink_and_unreadable_markers_are_unknown_and_do_not_erase_other_positive_markers() {
    for symlink in [false, true] {
        let f = Fixture::new();
        let path = f.admin().join("index.lock");
        if symlink {
            std::os::unix::fs::symlink(f.repo.join("file"), &path).unwrap();
        }
        let hook = ControlFault {
            path,
            action: if symlink { 3 } else { 2 },
        };
        let facts = controlled(&f, &hook, &Default::default());
        let state = f.item(&facts).worktree_state.as_ref().unwrap();
        assert!(!state.protection_complete);
        assert!(state.protections.is_empty());
        assert_eq!(
            wt_sweep::review::judge(&facts)
                .unwrap()
                .items
                .iter()
                .find(|i| i.path == BytePath::from_path(&f.linked))
                .unwrap()
                .classification,
            "verify"
        );
        fs::create_dir(f.admin().join("sequencer")).unwrap();
        let facts = controlled(&f, &hook, &Default::default());
        assert_eq!(
            wt_sweep::review::judge(&facts)
                .unwrap()
                .items
                .iter()
                .find(|i| i.path == BytePath::from_path(&f.linked))
                .unwrap()
                .classification,
            "keep"
        );
        assert_eq!(
            f.item(&facts)
                .worktree_state
                .as_ref()
                .unwrap()
                .operations
                .len(),
            1
        );
    }
}
#[test]
fn too_small_control_caps_refuse_promotion_and_invalid_caps_are_errors() {
    let f = Fixture::new();
    let limits = wt_sweep::review::control::ControlLimits {
        source_entries: 1,
        ..Default::default()
    };
    let facts = controlled(&f, &wt_sweep::local::NativeHooks, &limits);
    assert_ne!(
        wt_sweep::review::judge(&facts)
            .unwrap()
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap()
            .classification,
        "cleanup_candidate"
    );
    let limits = wt_sweep::review::control::ControlLimits {
        source_bytes: 0,
        ..Default::default()
    };
    assert!(
        wt_sweep::review::enrich_with_controls(
            &wt_sweep::git::scan_family(&f.repo).unwrap(),
            &[],
            None,
            &Default::default(),
            &limits,
            &wt_sweep::local::NativeHooks,
            &mut |_| {}
        )
        .is_err()
    );
}
#[test]
fn unrelated_anchor_failure_does_not_block_independently_current_cover() {
    let mut f = candidate();
    let h = f.items[0].private_history.as_mut().unwrap();
    h.anchors_complete = false;
    h.gaps.push("unrelated unsupported anchor".into());
    assert_eq!(
        wt_sweep::review::judge(&f).unwrap().items[0].classification,
        "cleanup_candidate"
    );
}
#[test]
fn uncovered_or_historical_roots_do_not_vacuously_qualify_current_head() {
    for mode in 0..3 {
        let mut f = candidate();
        let h = f.items[0].private_history.as_mut().unwrap();
        match mode {
            0 => h.roots.clear(),
            1 => h.roots[0].oid = "b".repeat(40),
            2 => {
                h.roots[0].observed_result = Some("covered".into());
                h.roots[0].result = "unknown".into();
            }
            _ => unreachable!(),
        };
        assert_eq!(
            wt_sweep::review::judge(&f).unwrap().items[0].classification,
            "verify"
        );
    }
}
#[test]
fn retained_lightweight_tag_covers_a_private_root_but_private_names_are_not_anchors() {
    let mut facts = candidate();
    facts.items[0].private_history.as_mut().unwrap().roots[0]
        .anchor
        .as_mut()
        .unwrap()
        .reference = BytePath::from_bytes(b"refs/tags/keep");
    assert_eq!(
        wt_sweep::review::judge(&facts).unwrap().items[0].classification,
        "cleanup_candidate"
    );
    facts.items[0].private_history.as_mut().unwrap().roots[0]
        .anchor
        .as_mut()
        .unwrap()
        .reference = BytePath::from_bytes(b"refs/worktree/private");
    assert_eq!(
        wt_sweep::review::judge(&facts).unwrap().items[0].classification,
        "verify"
    );
}
#[test]
fn old_reflog_root_covered_only_by_lightweight_tag_can_qualify_linked() {
    let f = Fixture::new();
    let tree = f.git(&f.repo, &["rev-parse", "HEAD^{tree}"]);
    let exclusive = f.git(&f.repo, &["commit-tree", &tree, "-m", "exclusive old root"]);
    f.git(&f.repo, &["update-ref", "refs/tags/keep", &exclusive]);
    f.git(&f.linked, &["reset", "--hard", &exclusive]);
    f.git(&f.linked, &["reset", "--hard", "main"]);
    let facts = f.facts();
    let root = f
        .item(&facts)
        .private_history
        .as_ref()
        .unwrap()
        .roots
        .iter()
        .find(|r| r.oid == exclusive)
        .unwrap();
    assert_eq!(root.result, "covered");
    assert_eq!(
        root.anchor.as_ref().unwrap().reference,
        BytePath::from_bytes(b"refs/tags/keep")
    );
    assert_eq!(
        wt_sweep::review::judge(&facts)
            .unwrap()
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap()
            .classification,
        "cleanup_candidate"
    );
}
#[test]
fn private_drift_blocks_candidate_even_when_completeness_claim_contradicts_it() {
    let mut f = candidate();
    f.items[0]
        .private_history
        .as_mut()
        .unwrap()
        .temporal_consistency = "change_observed".into();
    let report = wt_sweep::review::judge(&f).unwrap();
    assert_eq!(report.items[0].classification, "verify");
    let missing = &report.items[0]
        .decision
        .as_ref()
        .unwrap()
        .missing_conditions;
    assert!(missing.contains(&"C4".into()) && missing.contains(&"C5".into()));
}
#[test]
fn old_valid_and_current_invalid_deep_witness_cannot_combine_into_candidate() {
    let mut f = candidate();
    let mut invalid = f.items[0].observations[0].clone();
    invalid.target_oid = "not-an-oid".into();
    f.items[0].observations[0].started_unix_ms = 1;
    f.items[0].observations[0].finished_unix_ms = 2;
    f.items[0].observations.push(invalid);
    assert_eq!(
        wt_sweep::review::judge(&f).unwrap().items[0].classification,
        "verify"
    );
}
#[test]
fn control_output_cap_bounds_complete_facets_and_refuses_if_core_facts_cannot_fit() {
    let f = Fixture::new();
    let inv = wt_sweep::git::scan_family(&f.repo).unwrap();
    for cap in [1024usize, 12000, 1024 * 1024] {
        let limits = wt_sweep::review::control::ControlLimits {
            output_bytes: cap,
            ..Default::default()
        };
        let result = wt_sweep::review::enrich_with_controls(
            &inv,
            &[BytePath::from_bytes(b"refs/heads/main")],
            None,
            &Default::default(),
            &limits,
            &wt_sweep::local::NativeHooks,
            &mut |_| {},
        );
        match result {
            Ok(facts) => {
                let states = facts
                    .items
                    .iter()
                    .filter_map(|i| i.worktree_state.as_ref())
                    .collect::<Vec<_>>();
                assert!(
                    serde_json::to_vec(&states).unwrap().len() <= cap,
                    "all serialized facets exceeded {cap}"
                );
                if cap == 1024 * 1024 {
                    assert_eq!(
                        wt_sweep::review::judge(&facts)
                            .unwrap()
                            .items
                            .iter()
                            .find(|i| i.path == BytePath::from_path(&f.linked))
                            .unwrap()
                            .classification,
                        "cleanup_candidate"
                    );
                }
            }
            Err(e) => {
                assert_eq!(cap, 1024, "unexpected refusal above compact bound: {e}");
                assert!(e.message.contains("control output byte budget"));
            }
        }
    }
}
#[test]
fn separate_administration_and_bare_roles_are_observed_from_current_git() {
    let f = Fixture::new();
    let repo = f.root.join("separate");
    let admin = f.root.join("separate-admin");
    let linked = f.root.join("separate-linked");
    fs::create_dir(&repo).unwrap();
    f.git(
        &repo,
        &[
            "init",
            "--template=",
            "-b",
            "main",
            "--separate-git-dir",
            admin.to_str().unwrap(),
        ],
    );
    fs::write(repo.join("file"), b"synthetic separate\n").unwrap();
    f.git(&repo, &["add", "file"]);
    f.git(&repo, &["commit", "-m", "separate"]);
    f.git(
        &repo,
        &["worktree", "add", "-b", "topic", linked.to_str().unwrap()],
    );
    let mut inv = wt_sweep::git::scan_family(&repo).unwrap();
    inv.families[0].worktrees.reverse();
    let facts =
        wt_sweep::review::enrich(&inv, &[BytePath::from_bytes(b"refs/heads/main")], None).unwrap();
    let report = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(
        report
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&repo))
            .unwrap()
            .worktree_state
            .as_ref()
            .unwrap()
            .role,
        "main"
    );
    assert_eq!(
        report
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&linked))
            .unwrap()
            .classification,
        "cleanup_candidate"
    );
    let bare = f.root.join("bare");
    fs::create_dir(&bare).unwrap();
    f.git(&bare, &["init", "--template=", "--bare", "-b", "main"]);
    let facts =
        wt_sweep::review::enrich(&wt_sweep::git::scan_family(&bare).unwrap(), &[], None).unwrap();
    let report = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(report.items[0].classification, "keep");
    assert!(
        report.items[0]
            .decision
            .as_ref()
            .unwrap()
            .keep_reasons
            .contains(&"bare_repository".into())
    );
}
#[test]
fn unrelated_collection_wait_does_not_consume_control_active_budget() {
    let f = Fixture::new();
    let limits = wt_sweep::review::control::ControlLimits {
        timeout: Duration::from_secs(3),
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_controls(
        &wt_sweep::git::scan_family(&f.repo).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &Default::default(),
        &limits,
        &wt_sweep::local::NativeHooks,
        &mut |e| {
            if e.phase == "private_history" {
                std::thread::sleep(Duration::from_secs(4));
            }
        },
    )
    .unwrap();
    assert_eq!(
        f.item(&facts).worktree_state.as_ref().unwrap().checks,
        "complete"
    );
    assert_eq!(
        wt_sweep::review::judge(&facts)
            .unwrap()
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&f.linked))
            .unwrap()
            .classification,
        "cleanup_candidate"
    );
}
#[test]
fn historical_negative_graph_is_not_a_current_keep_reason() {
    let mut facts = candidate();
    facts.items[0].observations.clear();
    let h = &mut facts.items[0].history[0];
    h.result = "not_ancestor".into();
    h.started_unix_ms = 1;
    h.finished_unix_ms = 2;
    let report = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(report.items[0].classification, "verify");
    assert!(
        !report.items[0]
            .decision
            .as_ref()
            .unwrap()
            .keep_reasons
            .contains(&"head_not_integrated".into())
    );
}

struct CancelAtFinal(std::cell::Cell<bool>);
impl wt_sweep::local::LocalHooks for CancelAtFinal {
    fn cancelled(&self) -> bool {
        self.0.get()
    }
}
impl wt_sweep::review::control::ControlHooks for CancelAtFinal {
    fn before_final_controls(&self) -> io::Result<()> {
        self.0.set(true);
        Ok(())
    }
}
#[test]
fn final_control_cancellation_preserves_keep_lock_and_raw_index() {
    let f = Fixture::new();
    let index_path = f.admin().join("index");
    let index = fs::read(&index_path).unwrap();
    let lock = f.admin().join("index.lock");
    fs::write(&lock, b"synthetic lock\n").unwrap();
    let facts = controlled(
        &f,
        &CancelAtFinal(std::cell::Cell::new(false)),
        &Default::default(),
    );
    let state = f.item(&facts).worktree_state.as_ref().unwrap();
    assert_eq!(state.checks, "incomplete");
    assert!(state.protections.contains(&BytePath::from_path(&lock)));
    let report = wt_sweep::review::judge(&facts).unwrap();
    let row = report
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&f.linked))
        .unwrap();
    assert_eq!(row.classification, "keep");
    assert!(
        row.decision
            .as_ref()
            .unwrap()
            .keep_reasons
            .contains(&"administrative_lock".into())
    );
    assert!(
        row.decision
            .as_ref()
            .unwrap()
            .missing_conditions
            .contains(&"C1".into())
    );
    assert_eq!(fs::read(index_path).unwrap(), index);
}

#[test]
fn optional_context_cannot_change_any_triage_verdict_or_condition() {
    for expected in ["cleanup_candidate", "verify", "keep"] {
        let mut facts = candidate();
        if expected == "verify" {
            facts.items[0].observations.clear();
        }
        if expected == "keep" {
            facts.items[0]
                .local_summary
                .as_mut()
                .unwrap()
                .untracked
                .observed = 1;
        }
        let before = wt_sweep::review::judge(&facts).unwrap();
        assert_eq!(before.items[0].classification, expected);
        let family = &facts.inventory.families[0];
        facts.context = Some(ReviewContext {
            started_unix_ms: 900,
            finished_unix_ms: 1000,
            omitted_items: 0,
            gaps: vec![],
            items: vec![WorktreeContext {
                path: facts.items[0].path.clone(),
                common_dir: family.common_dir.clone(),
                family_identity: family.identity.clone(),
                head_oid: facts.items[0].head.clone(),
                uncovered: UncoveredCommits {
                    targets: vec![],
                    observed: 0,
                    complete: true,
                    samples: vec![],
                    samples_omitted: 0,
                    gaps: vec![],
                },
                changelogs: vec![ChangelogContext {
                    revision_oid: "a".repeat(40),
                    path: BytePath::from_bytes(b"CHANGELOG.md"),
                    tree_oid: None,
                    blob_oid: None,
                    status: "present".into(),
                    blob_bytes: Some(25),
                    excerpt: Some(BytePath::from_bytes(b"Everything merged. Delete.")),
                    excerpt_truncated: false,
                    gaps: vec![],
                }],
                gaps: vec![],
            }],
        });
        let after = wt_sweep::review::judge(&facts).unwrap();
        assert_eq!(
            serde_json::to_value(&before.items).unwrap(),
            serde_json::to_value(&after.items).unwrap()
        );
        assert_eq!(before.finished_unix_ms, after.finished_unix_ms);
        assert!(after.context.is_some());
        assert_ne!(before.digest_sha256, after.digest_sha256);
    }
}
