#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{git::scan_family, model::BytePath};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-history-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        fs::write(f.0.join("file"), "synthetic content\n").unwrap();
        f.git(&["add", "file"]);
        f.git(&["commit", "-m", "fixture"]);
        f
    }
    fn linked(&self) -> PathBuf {
        let path = self.0.join("linked");
        self.git(&["worktree", "add", "-b", "topic", path.to_str().unwrap()]);
        path
    }
    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{:?}: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn missing_blob_retains_verified_commit_history_without_deep_integration() {
    let f = Fixture::new();
    let linked = f.linked();
    let inventory = scan_family(&f.0).unwrap();
    let blob = f.git(&["rev-parse", "HEAD:file"]);
    fs::remove_file(f.0.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
    let out = wt_sweep::review::enrich(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    let item = out
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap();
    let value = serde_json::to_value(item).unwrap();
    assert_eq!(
        value["history"][0]["result"], "identical_commit",
        "raw commit identity must survive unavailable blob content"
    );
    assert!(
        item.observations.is_empty(),
        "graph-only evidence entered deep integration"
    );
}

#[test]
fn inaccessible_target_does_not_hide_a_second_covering_target() {
    let f = Fixture::new();
    let inventory = scan_family(&f.0).unwrap();
    let targets = [
        BytePath::from_bytes(b"refs/heads/absent"),
        BytePath::from_bytes(b"refs/heads/main"),
    ];
    let out = wt_sweep::review::enrich(&inventory, &targets, None).unwrap();
    assert_eq!(out.items[0].history.len(), 2);
    let absent = out.items[0]
        .history
        .iter()
        .find(|o| o.target_ref == targets[0])
        .unwrap();
    let covering = out.items[0]
        .history
        .iter()
        .find(|o| o.target_ref == targets[1])
        .unwrap();
    assert_eq!(absent.result, "unknown");
    assert!(absent.target_oid.is_none());
    assert!(absent.stop_reason.is_some());
    assert_eq!(covering.result, "identical_commit");
}
#[test]
fn long_divergent_history_is_unknown_at_default_cap_and_complete_when_injected() {
    let f = Fixture::new();
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let mut parent = f.git(&["commit-tree", &tree, "-m", "independent root"]);
    for n in 0..520 {
        parent = f.git(&[
            "commit-tree",
            &tree,
            "-p",
            &parent,
            "-m",
            &format!("synthetic {n}"),
        ]);
    }
    f.git(&["update-ref", "refs/heads/integration", &parent]);
    let inventory = scan_family(&f.0).unwrap();
    let targets = [BytePath::from_bytes(b"refs/heads/integration")];
    let normal = wt_sweep::review::enrich(&inventory, &targets, None).unwrap();
    assert_eq!(normal.items[0].history[0].result, "unknown");
    assert!(
        normal.items[0].history[0]
            .stop_reason
            .as_deref()
            .unwrap()
            .contains("per-target")
    );
    let limits = wt_sweep::review::ReviewLimits {
        per_target_steps: 600,
        deep_object_read_attempts: 1,
        ..Default::default()
    };
    let complete =
        wt_sweep::review::enrich_with_limits(&inventory, &targets, None, &limits).unwrap();
    assert_eq!(complete.items[0].history[0].result, "not_ancestor");
    assert!(complete.items[0].history[0].stop_reason.is_none());
    assert!(complete.items[0].observations.is_empty());
}
#[test]
fn final_target_read_failure_retains_previous_graph_result_without_claiming_drift() {
    let f = Fixture::new();
    let linked = f.linked();
    f.git(&["branch", "integration"]);
    let inventory = scan_family(&f.0).unwrap();
    let mut removed = false;
    let out = wt_sweep::review::enrich_with_limits_and_progress(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/integration")],
        None,
        &Default::default(),
        &mut |event| {
            if event.phase == "history_revalidate" {
                fs::remove_file(f.0.join(".git/refs/heads/integration")).unwrap();
                removed = true;
            }
        },
    )
    .unwrap();
    assert!(removed);
    let item = out
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap();
    let o = &item.history[0];
    assert_eq!(o.result, "unknown");
    assert_eq!(o.observed_result.as_deref(), Some("identical_commit"));
    assert!(
        o.stop_reason
            .as_deref()
            .unwrap()
            .contains("revalidation incomplete")
    );
    assert!(!o.stop_reason.as_deref().unwrap().contains("changed"));
    assert!(
        item.observations
            .iter()
            .all(|o| o.target_role != "integration")
    );
}
#[test]
fn unknown_source_head_stays_absent_with_explicit_unknown_history_row() {
    let f = Fixture::new();
    let mut inventory = scan_family(&f.0).unwrap();
    inventory.families[0].worktrees[0].head = None;
    let out = wt_sweep::review::enrich(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    assert_eq!(out.items[0].history[0].result, "unknown");
    assert!(out.items[0].history[0].source_oid.is_none());
    assert_eq!(out.history_metrics.object_read_attempts, 0);
    assert_eq!(out.history_metrics.target_resolution_attempts, 0);
}
#[test]
fn deep_target_omitted_from_graph_jobs_is_invalidated_after_final_mutation() {
    let f = Fixture::new();
    let linked = f.linked();
    f.git(&["branch", "a-integration"]);
    f.git(&["branch", "b-integration"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let changed = f.git(&["commit-tree", &tree, "-m", "independent new target"]);
    let inventory = scan_family(&f.0).unwrap();
    let a = BytePath::from_bytes(b"refs/heads/a-integration");
    let b = BytePath::from_bytes(b"refs/heads/b-integration");
    let limits = wt_sweep::review::ReviewLimits {
        target_jobs: 2,
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_limits_and_progress(
        &inventory,
        &[a, b.clone()],
        None,
        &limits,
        &mut |event| {
            if event.phase == "history_revalidate" {
                f.git(&["update-ref", "refs/heads/b-integration", &changed]);
            }
        },
    )
    .unwrap();
    let item = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(item.history_omitted_targets, 1);
    assert!(
        item.observations.iter().all(|o| o.target_ref != b),
        "stale deep result stayed current when its target had no graph job"
    );
}
#[test]
fn completed_deep_intermediate_ref_is_still_supported_after_final_validation() {
    let f = Fixture::new();
    let linked = f.linked();
    f.git(&["branch", "other"]);
    let inventory = scan_family(&f.0).unwrap();
    let out = wt_sweep::review::enrich(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    let item = out
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap();
    assert!(item.observations.iter().any(|o| o.target_ref
        == BytePath::from_bytes(b"refs/heads/other")
        && o.target_role == "intermediate"));
}
#[test]
fn wide_parent_frontier_refuses_before_loading_unbounded_parent_sets() {
    let f = Fixture::new();
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let parents: Vec<_> = (0..8)
        .map(|n| f.git(&["commit-tree", &tree, "-m", &format!("root {n}")]))
        .collect();
    let mut args = vec!["commit-tree", tree.as_str()];
    for parent in &parents {
        args.extend(["-p", parent.as_str()]);
    }
    args.extend(["-m", "wide merge"]);
    let target = f.git(&args);
    f.git(&["update-ref", "refs/heads/integration", &target]);
    let inventory = scan_family(&f.0).unwrap();
    let limits = wt_sweep::review::ReviewLimits {
        traversal_steps: 4,
        deep_object_read_attempts: 1,
        ..Default::default()
    };
    let out = wt_sweep::review::enrich_with_limits(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/integration")],
        None,
        &limits,
    )
    .unwrap();
    assert_eq!(out.items[0].history[0].result, "unknown");
    assert_eq!(
        out.history_metrics.object_read_attempts, 2,
        "oversized frontier was expanded into additional object reads"
    );
    assert_eq!(out.history_metrics.traversal_steps, 1);
    assert!(
        out.items[0].history[0]
            .stop_reason
            .as_deref()
            .unwrap()
            .contains("frontier")
    );
    assert!(
        out.history_metrics.stop_reason.is_some(),
        "history stop cause not exposed"
    );
}
#[test]
fn final_source_head_mutation_preserves_graph_and_local_counts_as_incomplete() {
    let f = Fixture::new();
    f.git(&["branch", "integration"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let changed = f.git(&["commit-tree", &tree, "-m", "new source"]);
    let inventory = scan_family(&f.0).unwrap();
    let facts = wt_sweep::review::enrich_with_limits_and_progress(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/integration")],
        None,
        &Default::default(),
        &mut |event| {
            if event.phase == "history_revalidate" {
                f.git(&["update-ref", "refs/heads/main", &changed]);
            }
        },
    )
    .unwrap();
    let history = &facts.items[0].history[0];
    assert_eq!(history.result, "unknown");
    assert_eq!(history.observed_result.as_deref(), Some("identical_commit"));
    let summary = facts.items[0].local_summary.as_ref().unwrap();
    assert!(
        !summary.head_complete && !summary.index_complete && !summary.disk_complete,
        "final observed HEAD change left summary counts current and complete"
    );
    assert_eq!(summary.temporal_consistency, "change_observed");
    assert_eq!(summary.staged.observed, 0);
    assert!(facts.items[0].observations.is_empty());
}
#[test]
fn first_pass_counts_only_rows_with_an_actual_history_turn() {
    let f = Fixture::new();
    let linked = f.0.join("linked");
    f.git(&[
        "worktree",
        "add",
        "--detach",
        linked.to_str().unwrap(),
        "HEAD",
    ]);
    let inventory = scan_family(&f.0).unwrap();
    assert_eq!(inventory.families[0].worktrees.len(), 2);
    let no_targets = wt_sweep::review::enrich(&inventory, &[], None).unwrap();
    assert_eq!(
        no_targets.history_metrics.first_pass_items, 0,
        "rows without a job counted as processed history"
    );
    let limits = wt_sweep::review::ReviewLimits {
        target_jobs: 1,
        ..Default::default()
    };
    let one = wt_sweep::review::enrich_with_limits(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &limits,
    )
    .unwrap();
    assert_eq!(one.history_metrics.items_total, 2);
    assert_eq!(
        one.history_metrics.first_pass_items, 1,
        "graph-omitted row counted as a first turn"
    );
    assert_eq!(
        one.items
            .iter()
            .map(|i| i.history_omitted_targets)
            .sum::<usize>(),
        1
    );
}
#[test]
fn timed_out_graph_family_spawns_once_and_cannot_starve_healthy_family() {
    let blocked = Fixture::new();
    let healthy = Fixture::new();
    let tree = blocked.git(&["rev-parse", "HEAD^{tree}"]);
    for n in 1..6 {
        let head = blocked.git(&["commit-tree", &tree, "-m", &format!("distinct source {n}")]);
        let path = blocked.0.join(format!("linked-{n}"));
        blocked.git(&["worktree", "add", "--detach", path.to_str().unwrap(), &head]);
    }
    let mut inventory = scan_family(&blocked.0).unwrap();
    let good = scan_family(&healthy.0).unwrap();
    assert_eq!(inventory.families[0].worktrees.len(), 6);
    assert_eq!(
        inventory.families[0]
            .worktrees
            .iter()
            .filter_map(|w| w.head.as_ref())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        6
    );
    inventory.families.extend(good.families);
    let config = blocked.0.join(".git/config");
    let saved_config = blocked.0.join(".git/config.before");
    fs::rename(&config, &saved_config).unwrap();
    let status = Command::new("/usr/bin/mkfifo")
        .arg(&config)
        .status()
        .unwrap();
    assert!(status.success());
    // Lowering the total allowance keeps the RED refusal bounded. The oracle is
    // exact work/process counts and the healthy positive, never elapsed speed.
    let limits = wt_sweep::review::ReviewLimits {
        timeout: std::time::Duration::from_secs(20),
        ..Default::default()
    };
    let target_map = wt_sweep::model::TargetMap {
        schema: wt_sweep::model::TARGET_MAP_SCHEMA.into(),
        families: inventory
            .families
            .iter()
            .map(|family| wt_sweep::model::TargetMapFamily {
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
                targets: vec![BytePath::from_bytes(b"refs/heads/main")],
            })
            .collect(),
    };
    let result = wt_sweep::review::enrich_with_target_input_and_controls(
        &inventory,
        wt_sweep::review::targets::TargetInput::Map(&target_map),
        None,
        &limits,
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |_| {},
    );
    fs::remove_file(&config).unwrap();
    fs::rename(&saved_config, &config).unwrap();
    let facts = result.unwrap();
    let healthy_item = facts
        .items
        .iter()
        .find(|i| i.path.to_path().unwrap() == healthy.0)
        .unwrap();
    assert_eq!(
        healthy_item.history[0].result, "identical_commit",
        "blocked family exhausted history before healthy source"
    );
    assert_eq!(
        facts.history_metrics.object_read_attempts, 1,
        "dependent child repeated across distinct HEADs in timed-out family"
    );
    assert_eq!(
        facts.history_metrics.target_resolution_attempts, 2,
        "quarantined family performed a target read"
    );
    // Shared initial controls quarantine the blocked family before graph work.
    // History owns only the healthy source plus initial and final target reads.
    assert_eq!(
        facts.history_metrics.spawned_processes, 3,
        "unexpected history/final child after family timeout"
    );
    for item in facts
        .items
        .iter()
        .filter(|i| i.path.to_path().unwrap() != healthy.0)
    {
        assert_eq!(item.history[0].result, "unknown");
        assert!(item.observations.is_empty());
        let summary = item.local_summary.as_ref().unwrap();
        assert!(!summary.head_complete && !summary.index_complete && !summary.disk_complete);
        assert!(
            summary.sources.is_empty(),
            "local summary launched reads in quarantined family"
        );
        assert!(item.unknowns.join(" ").contains("timeout"));
    }
}
#[test]
fn target_resolution_timeout_preserves_prior_graph_positive_and_quarantines_family() {
    let blocked = Fixture::new();
    let healthy = Fixture::new();
    blocked.git(&["branch", "z-covering"]);
    healthy.git(&["branch", "z-covering"]);
    let mut inventory = scan_family(&blocked.0).unwrap();
    inventory
        .families
        .extend(scan_family(&healthy.0).unwrap().families);
    let config = blocked.0.join(".git/config");
    let saved_config = blocked.0.join(".git/config.before");
    let mut installed = false;
    let limits = wt_sweep::review::ReviewLimits {
        timeout: std::time::Duration::from_secs(20),
        ..Default::default()
    };
    let targets = [
        BytePath::from_bytes(b"refs/heads/main"),
        BytePath::from_bytes(b"refs/heads/z-covering"),
    ];
    let target_map = wt_sweep::model::TargetMap {
        schema: wt_sweep::model::TARGET_MAP_SCHEMA.into(),
        families: inventory
            .families
            .iter()
            .map(|family| wt_sweep::model::TargetMapFamily {
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
                targets: targets.to_vec(),
            })
            .collect(),
    };
    let result = wt_sweep::review::enrich_with_target_input_and_controls(
        &inventory,
        wt_sweep::review::targets::TargetInput::Map(&target_map),
        None,
        &limits,
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "history_first_pass" {
                fs::rename(&config, &saved_config).unwrap();
                assert!(
                    Command::new("/usr/bin/mkfifo")
                        .arg(&config)
                        .status()
                        .unwrap()
                        .success()
                );
                installed = true;
            }
        },
    );
    assert!(installed);
    fs::remove_file(&config).unwrap();
    fs::rename(&saved_config, &config).unwrap();
    let facts = result.unwrap();
    let item = facts
        .items
        .iter()
        .find(|item| item.path.to_path().unwrap() == blocked.0)
        .unwrap();
    assert_eq!(item.history[0].result, "unknown");
    assert_eq!(
        item.history[0].observed_result.as_deref(),
        Some("identical_commit"),
        "quarantine erased prior observed positive"
    );
    assert_eq!(item.history[1].result, "unknown");
    assert!(item.history[1].observed_result.is_none());
    assert!(item.local_summary.as_ref().unwrap().sources.is_empty());
    assert!(item.observations.is_empty());
    let good = facts
        .items
        .iter()
        .find(|item| item.path.to_path().unwrap() == healthy.0)
        .unwrap();
    assert!(
        good.history
            .iter()
            .all(|observation| observation.result == "identical_commit")
    );
    assert_eq!(facts.history_metrics.object_read_attempts, 2);
    assert_eq!(facts.history_metrics.target_resolution_attempts, 6);
    assert_eq!(
        facts.history_metrics.spawned_processes, 8,
        "fresh child launched after typed target timeout quarantine"
    );
}
#[test]
fn final_ref_timeout_quarantines_family_and_preserves_earlier_checked_result() {
    let blocked = Fixture::new();
    let healthy = Fixture::new();
    let names = ["a-safe", "b-blocked", "c-blocked"];
    for name in names {
        blocked.git(&["branch", name]);
        healthy.git(&["branch", name]);
    }
    let mut inventory = scan_family(&blocked.0).unwrap();
    inventory
        .families
        .extend(scan_family(&healthy.0).unwrap().families);
    let targets: Vec<_> = names
        .iter()
        .map(|name| BytePath::from_bytes(format!("refs/heads/{name}").as_bytes()))
        .collect();
    let limits = wt_sweep::review::ReviewLimits {
        timeout: std::time::Duration::from_secs(20),
        ..Default::default()
    };
    let target_map = wt_sweep::model::TargetMap {
        schema: wt_sweep::model::TARGET_MAP_SCHEMA.into(),
        families: inventory
            .families
            .iter()
            .map(|family| wt_sweep::model::TargetMapFamily {
                common_dir: family.common_dir.clone(),
                identity: family.identity.clone(),
                targets: targets.to_vec(),
            })
            .collect(),
    };
    let result = wt_sweep::review::enrich_with_target_input_and_controls(
        &inventory,
        wt_sweep::review::targets::TargetInput::Map(&target_map),
        None,
        &limits,
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "history_revalidate" {
                for name in &names[1..] {
                    let path = blocked.0.join(".git/refs/heads").join(name);
                    fs::rename(&path, path.with_extension("before")).unwrap();
                    assert!(
                        Command::new("/usr/bin/mkfifo")
                            .arg(&path)
                            .status()
                            .unwrap()
                            .success()
                    );
                }
            }
        },
    );
    for name in &names[1..] {
        let path = blocked.0.join(".git/refs/heads").join(name);
        fs::remove_file(&path).unwrap();
        fs::rename(path.with_extension("before"), &path).unwrap();
    }
    let facts = result.unwrap();
    let bad = facts
        .items
        .iter()
        .find(|i| i.path.to_path().unwrap() == blocked.0)
        .unwrap();
    assert!(
        bad.history.iter().all(|o| o.result == "unknown"),
        "final timeout left an earlier same-family result current"
    );
    assert!(
        bad.history
            .iter()
            .all(|o| o.observed_result.as_deref() == Some("identical_commit"))
    );
    assert!(bad.observations.is_empty());
    assert!(bad.unknowns.join(" ").contains("timeout"));
    let good = facts
        .items
        .iter()
        .find(|i| i.path.to_path().unwrap() == healthy.0)
        .unwrap();
    assert!(
        good.history.iter().all(|o| o.result == "identical_commit"),
        "blocked final refs starved healthy family validation"
    );
    assert_eq!(facts.history_metrics.object_read_attempts, 2);
    assert_eq!(
        facts.history_metrics.target_resolution_attempts, 11,
        "more than one final child entered blocked refs"
    );
    assert_eq!(facts.history_metrics.spawned_processes, 13);
}
#[test]
fn staged_blob_timeout_quarantine_preserves_independently_verified_index_flags() {
    let f = Fixture::new();
    fs::write(f.0.join("file"), "new staged synthetic content\n").unwrap();
    f.git(&["add", "file"]);
    let blob = f.git(&["rev-parse", ":file"]);
    let inventory = scan_family(&f.0).unwrap();
    let object = f.0.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
    let saved = object.with_extension("before");
    fs::rename(&object, &saved).unwrap();
    assert!(
        Command::new("/usr/bin/mkfifo")
            .arg(&object)
            .status()
            .unwrap()
            .success()
    );
    let result = wt_sweep::review::enrich(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    );
    fs::remove_file(&object).unwrap();
    fs::rename(&saved, &object).unwrap();
    let facts = result.unwrap();
    let item = &facts.items[0];
    let summary = item.local_summary.as_ref().unwrap();
    assert!(
        summary.index_complete,
        "skipped family validation erased independent raw index verification"
    );
    assert_eq!(summary.staged.observed, 1);
    assert!(
        !summary.staged.complete,
        "quarantine must not upgrade the collector's incomplete staged comparison"
    );
    assert!(!summary.unstaged.complete);
    assert_eq!(summary.temporal_consistency, "not_guaranteed");
    assert_eq!(
        summary.digest_sha256,
        wt_sweep::output::canonical_digest(&{
            let mut original = summary.clone();
            original.digest_sha256 = String::new();
            original
        })
        .unwrap()
    );
    assert_eq!(item.history[0].result, "unknown");
    assert_eq!(
        item.history[0].observed_result.as_deref(),
        Some("identical_commit")
    );
    assert!(item.observations.is_empty());
    assert!(item.unknowns.join(" ").contains("timeout"));
}

fn raw_graph_commit(f: &Fixture, tree: &str, headers: &str, literal: bool) -> String {
    let path = f.0.join(".git/t09-raw-commit");
    fs::write(&path, format!("tree {tree}\n{headers}author Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nSynthetic graph\n")).unwrap();
    if literal {
        f.git(&[
            "hash-object",
            "--literally",
            "-t",
            "commit",
            "-w",
            path.to_str().unwrap(),
        ])
    } else {
        f.git(&["hash-object", "-t", "commit", "-w", path.to_str().unwrap()])
    }
}
#[test]
fn uppercase_parent_is_ancestor_in_required_and_deep_graph_without_false_keep() {
    let f = Fixture::new();
    let linked = f.linked();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let child = raw_graph_commit(
        &f,
        &tree,
        &format!("parent {}\n", base.to_ascii_uppercase()),
        false,
    );
    f.git(&["update-ref", "refs/heads/main", &child]);
    f.git(&["fsck", "--no-reflogs", "--no-dangling"]);
    f.git(&["merge-base", "--is-ancestor", &base, &child]);
    assert_eq!(f.git(&["rev-list", &base, "--not", &child]), "");
    let facts = wt_sweep::review::enrich(
        &scan_family(&f.0).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    let row = facts
        .items
        .iter()
        .find(|row| row.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(
        row.history[0].result, "ancestor",
        "valid uppercase parent became a complete negative"
    );
    assert!(row.history[0].stop_reason.is_none());
    assert!(
        row.observations
            .iter()
            .any(|observation| observation.relation == "ancestor"
                && observation.source_oid == base
                && observation.target_oid == child),
        "deep graph fell back to identical_tree instead of finding the parent"
    );
    let judged = wt_sweep::review::judge(&facts).unwrap();
    let value = serde_json::to_value(
        judged
            .items
            .iter()
            .find(|row| row.path == BytePath::from_path(&linked))
            .unwrap(),
    )
    .unwrap();
    assert!(
        !value["decision"]["keep_reasons"]
            .as_array()
            .unwrap()
            .iter()
            .any(|reason| reason == "head_not_integrated")
    );
}
#[test]
fn mixed_case_shared_parent_deduplicates_within_exact_traversal_budget() {
    let f = Fixture::new();
    let linked = f.linked();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let left = raw_graph_commit(
        &f,
        &tree,
        &format!("parent {}\n", base.to_ascii_uppercase()),
        false,
    );
    let mixed: String = base
        .chars()
        .enumerate()
        .map(|(i, c)| {
            if i % 2 == 0 {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    let right = raw_graph_commit(&f, &tree, &format!("parent {mixed}\n"), false);
    let merge = f.git(&[
        "commit-tree",
        &tree,
        "-p",
        &left,
        "-p",
        &right,
        "-m",
        "merge",
    ]);
    let unrelated = f.git(&["commit-tree", &tree, "-m", "unrelated"]);
    f.git(&["update-ref", "refs/heads/main", &merge]);
    f.git(&[
        "-C",
        linked.to_str().unwrap(),
        "reset",
        "--hard",
        &unrelated,
    ]);
    f.git(&["fsck", "--no-reflogs", "--no-dangling"]);
    let limits = wt_sweep::review::ReviewLimits {
        per_target_steps: 4,
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_limits(
        &scan_family(&f.0).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &limits,
    )
    .unwrap();
    let row = facts
        .items
        .iter()
        .find(|row| row.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(
        row.history[0].result, "not_ancestor",
        "one object spelled twice consumed graph identity budget"
    );
    assert!(row.history[0].stop_reason.is_none());
}
#[test]
fn malformed_graph_parent_header_refuses_required_and_deep_complete_claims() {
    let f = Fixture::new();
    let linked = f.linked();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let child = raw_graph_commit(&f, &tree, &format!("parent\t{base}\n"), true);
    f.git(&["update-ref", "refs/heads/main", &child]);
    let facts = wt_sweep::review::enrich(
        &scan_family(&f.0).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    let row = facts
        .items
        .iter()
        .find(|row| row.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(
        row.history[0].result, "unknown",
        "malformed parent became absent graph edge"
    );
    assert!(row.history[0].stop_reason.is_some());
    assert!(row.observations.is_empty());
}

#[test]
fn full_oid_case_alias_in_saved_head_keeps_provenance_and_current_identity() {
    let f = Fixture::new();
    let linked = f.linked();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let child = f.git(&["commit-tree", &tree, "-p", &base, "-m", "child"]);
    f.git(&["update-ref", "refs/heads/main", &child]);
    let mut inventory = scan_family(&f.0).unwrap();
    let saved = base.to_ascii_uppercase();
    inventory.families[0]
        .worktrees
        .iter_mut()
        .find(|row| row.path == BytePath::from_path(&linked))
        .unwrap()
        .head = Some(saved.clone());
    let facts = wt_sweep::review::enrich(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
    )
    .unwrap();
    let row = facts
        .items
        .iter()
        .find(|row| row.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(
        row.history[0].result, "ancestor",
        "case alias became false source drift"
    );
    assert_eq!(row.head.as_deref(), Some(saved.as_str()));
    assert_eq!(row.history[0].source_oid.as_deref(), Some(saved.as_str()));
    assert!(row.observations.iter().any(|o| o.relation == "ancestor"));
}

// Restoring per-source traversal makes this exceed the single target walk budget.
fn shared_history_fixture() -> (Fixture, Vec<String>, Vec<String>) {
    let f = Fixture::new();
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let mut chain = vec![f.git(&["rev-parse", "HEAD"])];
    for n in 1..12 {
        chain.push(f.git(&[
            "commit-tree",
            &tree,
            "-p",
            chain.last().unwrap(),
            "-m",
            &format!("linear {n}"),
        ]));
    }
    f.git(&["update-ref", "refs/heads/main", chain.last().unwrap()]);
    for n in [0, 3, 6, 9] {
        let path = f.0.join(format!("ancestor-{n}"));
        f.git(&[
            "worktree",
            "add",
            "--detach",
            path.to_str().unwrap(),
            &chain[n],
        ]);
    }
    let divergent: Vec<_> = (0..2)
        .map(|n| {
            let oid = f.git(&["commit-tree", &tree, "-m", &format!("divergent {n}")]);
            let path = f.0.join(format!("divergent-{n}"));
            f.git(&["worktree", "add", "--detach", path.to_str().unwrap(), &oid]);
            oid
        })
        .collect();
    (f, chain, divergent)
}

#[test]
fn distinct_sources_share_one_verified_target_walk_at_exact_total_budget() {
    let (f, chain, divergent) = shared_history_fixture();
    let inventory = scan_family(&f.0).unwrap();
    let limits = wt_sweep::review::ReviewLimits {
        traversal_steps: chain.len(),
        per_target_steps: chain.len(),
        object_read_attempts: chain.len() + divergent.len(),
        slice_traversal_steps: 2,
        deep_object_read_attempts: 1,
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_limits(
        &inventory,
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &limits,
    )
    .unwrap();
    assert_eq!(
        facts.history_metrics.first_pass_items,
        inventory.families[0].worktrees.len() as u64
    );
    for row in &facts.items {
        let source = row.head.as_ref().unwrap();
        let expected = if source == chain.last().unwrap() {
            "identical_commit"
        } else if divergent.contains(source) {
            "not_ancestor"
        } else {
            "ancestor"
        };
        assert_eq!(
            row.history[0].result, expected,
            "source {source} was not decided from a shared verified graph"
        );
        assert!(row.history[0].stop_reason.is_none());
    }
    assert_eq!(
        facts.history_metrics.traversal_steps,
        chain.len() as u64,
        "target graph was expanded more than once"
    );
    assert_eq!(
        facts.history_metrics.object_read_attempts,
        (chain.len() + divergent.len()) as u64
    );
    assert_eq!(facts.history_metrics.incomplete_targets, 0);
}

#[test]
fn reduced_shared_target_cap_retains_seen_positives_and_explicit_unknowns() {
    let (f, chain, _) = shared_history_fixture();
    let limits = wt_sweep::review::ReviewLimits {
        per_target_steps: 4,
        slice_traversal_steps: 1,
        deep_object_read_attempts: 1,
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_limits(
        &scan_family(&f.0).unwrap(),
        &[BytePath::from_bytes(b"refs/heads/main")],
        None,
        &limits,
    )
    .unwrap();
    for row in &facts.items {
        let source = row.head.as_ref().unwrap();
        if source == chain.last().unwrap() {
            assert_eq!(row.history[0].result, "identical_commit");
        } else if source == &chain[9] {
            assert_eq!(row.history[0].result, "ancestor");
        } else {
            assert_eq!(row.history[0].result, "unknown");
            assert!(
                row.history[0]
                    .stop_reason
                    .as_deref()
                    .unwrap()
                    .contains("per-target")
            );
        }
    }
    assert_eq!(
        facts.history_metrics.traversal_steps, 4,
        "cap was spent again for another source"
    );
}

#[test]
fn target_ref_aliases_share_graph_but_revalidate_their_original_refs() {
    let (f, chain, divergent) = shared_history_fixture();
    f.git(&["branch", "alias"]);
    let targets = [
        BytePath::from_bytes(b"refs/heads/alias"),
        BytePath::from_bytes(b"refs/heads/main"),
    ];
    let limits = wt_sweep::review::ReviewLimits {
        per_target_steps: chain.len(),
        slice_traversal_steps: 2,
        deep_object_read_attempts: 1,
        ..Default::default()
    };
    let facts = wt_sweep::review::enrich_with_limits_and_progress(
        &scan_family(&f.0).unwrap(),
        &targets,
        None,
        &limits,
        &mut |event| {
            if event.phase == "history_revalidate" {
                f.git(&["update-ref", "refs/heads/alias", &divergent[0]]);
            }
        },
    )
    .unwrap();
    for row in &facts.items {
        let changed = row
            .history
            .iter()
            .find(|o| o.target_ref == targets[0])
            .unwrap();
        let stable = row
            .history
            .iter()
            .find(|o| o.target_ref == targets[1])
            .unwrap();
        assert_eq!(changed.result, "unknown");
        assert!(changed.observed_result.is_some());
        assert!(changed.stop_reason.as_deref().unwrap().contains("changed"));
        let expected = if row.head.as_ref().unwrap() == chain.last().unwrap() {
            "identical_commit"
        } else if divergent.contains(row.head.as_ref().unwrap()) {
            "not_ancestor"
        } else {
            "ancestor"
        };
        assert_eq!(stable.result, expected);
    }
    assert_eq!(facts.history_metrics.traversal_steps, chain.len() as u64);
    assert_eq!(facts.history_metrics.target_resolution_attempts, 4);
}
