#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{
    git::scan_family,
    model::{BytePath, Observations},
    review::{ReviewOptions, enrich_with_options, judge, targets::TargetInput},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-context-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        fs::write(f.0.join("CHANGELOG.md"), b"recorded synthetic release\n").unwrap();
        f.git(&["add", "."]);
        f.git(&["commit", "-m", "base"]);
        f
    }
    fn at(path: &Path, args: &[&str]) -> String {
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
            .arg(path)
            .args(args)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap().trim().into()
    }
    fn git(&self, args: &[&str]) -> String {
        Self::at(&self.0, args)
    }
    fn collect(&self, refs: &[&str], options: &ReviewOptions) -> Observations {
        let targets = refs
            .iter()
            .map(|r| BytePath::from_bytes(r.as_bytes()))
            .collect::<Vec<_>>();
        enrich_with_options(
            &scan_family(&self.0).unwrap(),
            TargetInput::Simple(&targets),
            None,
            options,
            &wt_sweep::local::NativeHooks,
            &mut |_| {},
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
#[test]
fn context_describes_recorded_changelog_and_exact_divergent_commit() {
    let f = Fixture::new();
    f.git(&["checkout", "-b", "topic"]);
    fs::write(f.0.join("CHANGELOG.md"), b"topic release\n").unwrap();
    f.git(&["commit", "-am", "topic commit"]);
    let head = f.git(&["rev-parse", "HEAD"]);
    let facts = f.collect(&["refs/heads/main"], &ReviewOptions::default());
    let context = facts
        .context
        .as_ref()
        .expect("enabled context collector absent");
    assert!(context.started_unix_ms >= facts.finished_unix_ms);
    let row = &context.items[0];
    assert_eq!(row.head_oid.as_deref(), Some(head.as_str()));
    assert!(row.uncovered.complete);
    assert_eq!(row.uncovered.observed, 1);
    assert_eq!(row.uncovered.samples[0].oid, head);
    assert_eq!(
        row.uncovered.samples[0]
            .subject
            .as_ref()
            .unwrap()
            .bytes()
            .unwrap(),
        b"topic commit"
    );
    let changelog = &row.changelogs[0];
    assert_eq!(changelog.status, "present");
    assert_eq!(
        changelog.excerpt.as_ref().unwrap().bytes().unwrap(),
        b"topic release\n"
    );
    let mut without = facts.clone();
    without.context = None;
    assert_eq!(
        serde_json::to_value(judge(&facts).unwrap().items).unwrap(),
        serde_json::to_value(judge(&without).unwrap().items).unwrap()
    );
}
#[test]
fn merge_graph_union_deduplicates_and_sample_limit_keeps_counting() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let a = f.git(&["commit-tree", &tree, "-p", &base, "-m", "a"]);
    let b = f.git(&["commit-tree", &tree, "-p", &base, "-m", "b"]);
    let merge = f.git(&["commit-tree", &tree, "-p", &a, "-p", &b, "-m", "merge"]);
    f.git(&["update-ref", "refs/heads/topic", &merge]);
    f.git(&["update-ref", "refs/heads/side", &a]);
    f.git(&["checkout", "topic"]);
    let mut options = ReviewOptions::default();
    options.context.limits.commit_samples = 1;
    let facts = f.collect(&["refs/heads/main"], &options);
    let row = &facts.context.unwrap().items[0].uncovered;
    assert!(row.complete);
    assert_eq!(row.observed, 3);
    assert_eq!(row.samples.len(), 1);
    assert_eq!(row.samples_omitted, 2);
    assert_eq!(row.samples[0].oid, merge);
    let facts = f.collect(
        &["refs/heads/main", "refs/heads/side"],
        &ReviewOptions::default(),
    );
    let row = &facts.context.unwrap().items[0].uncovered;
    assert!(row.complete);
    assert_eq!(row.observed, 2);
    assert_eq!(
        row.samples
            .iter()
            .map(|s| s.oid.as_str())
            .collect::<Vec<_>>(),
        vec![merge.as_str(), b.as_str()]
    );
}
#[test]
fn zero_shortcut_survives_missing_target_but_no_targets_is_unknown() {
    let f = Fixture::new();
    let mut options = ReviewOptions::default();
    options.context.changelog_paths.clear();
    options.context.limits.object_read_attempts = 1;
    options.context.limits.object_bytes = 1;
    options.context.limits.traversal_steps = 1;
    let facts = f.collect(&["refs/heads/absent", "refs/heads/main"], &options);
    let row = &facts.context.unwrap().items[0].uncovered;
    assert!(row.complete);
    assert_eq!(row.observed, 0);
    for targets in [&[][..], &["refs/heads/absent"][..]] {
        let facts = f.collect(targets, &ReviewOptions::default());
        let row = &facts.context.unwrap().items[0].uncovered;
        assert!(!row.complete);
        assert_eq!(row.observed, 0);
        assert!(row.samples.is_empty());
        assert!(!row.gaps.is_empty());
    }
}
#[test]
fn changelog_nested_raw_bytes_empty_absent_and_binary_are_distinct() {
    let f = Fixture::new();
    fs::create_dir(f.0.join("docs")).unwrap();
    fs::write(f.0.join("docs/changes"), b"\xff\x1b[31mraw\n").unwrap();
    fs::write(f.0.join("empty"), []).unwrap();
    fs::write(f.0.join("binary"), b"binary\0content").unwrap();
    f.git(&["add", "."]);
    f.git(&["commit", "-m", "paths"]);
    let mut options = ReviewOptions::default();
    options.context.changelog_paths = ["docs/changes", "empty", "absent", "binary"]
        .iter()
        .map(|p| BytePath::from_bytes(p.as_bytes()))
        .collect();
    options.context.limits.excerpt_bytes = 3;
    let facts = f.collect(&["refs/heads/main"], &options);
    let rows = &facts.context.unwrap().items[0].changelogs;
    assert_eq!(rows[0].status, "present");
    assert_eq!(
        rows[0].excerpt.as_ref().unwrap().bytes().unwrap(),
        b"\xff\x1b["
    );
    assert!(rows[0].excerpt_truncated);
    assert_eq!(rows[1].status, "present");
    assert_eq!(rows[1].blob_bytes, Some(0));
    assert_eq!(rows[1].excerpt.as_ref().unwrap().bytes().unwrap(), b"");
    assert_eq!(rows[2].status, "absent");
    assert_eq!(rows[2].blob_oid, None);
    assert_eq!(rows[3].status, "unknown");
    assert!(rows[3].excerpt.is_none());
}
#[test]
fn changelog_symlinks_gitlinks_and_invalid_paths_are_refused() {
    let f = Fixture::new();
    std::os::unix::fs::symlink("/etc/passwd", f.0.join("outside")).unwrap();
    std::os::unix::fs::symlink("docs", f.0.join("parent")).unwrap();
    f.git(&["add", "."]);
    let oid = f.git(&["rev-parse", "HEAD"]);
    f.git(&[
        "update-index",
        "--add",
        "--cacheinfo",
        &format!("160000,{oid},sub"),
    ]);
    f.git(&["commit", "-m", "unsafe modes"]);
    let mut options = ReviewOptions::default();
    options.context.changelog_paths = ["outside", "parent/changelog", "sub"]
        .iter()
        .map(|p| BytePath::from_bytes(p.as_bytes()))
        .collect();
    let facts = f.collect(&["refs/heads/main"], &options);
    for row in &facts.context.unwrap().items[0].changelogs {
        assert_eq!(row.status, "unknown");
        assert!(row.excerpt.is_none());
    }
    let inventory = scan_family(&f.0).unwrap();
    for path in [
        "",
        "/absolute",
        "../outside",
        "docs/../outside",
        "a//b",
        "./a",
        "a/",
    ] {
        options.context.changelog_paths = vec![BytePath::from_bytes(path.as_bytes())];
        assert!(
            enrich_with_options(
                &inventory,
                TargetInput::Simple(&[]),
                None,
                &options,
                &wt_sweep::local::NativeHooks,
                &mut |_| {}
            )
            .is_err(),
            "{path:?}"
        );
    }
}
#[test]
fn oversize_and_missing_verified_blob_never_publish_partial_excerpt() {
    for missing in [false, true] {
        let f = Fixture::new();
        fs::write(f.0.join("CHANGELOG.md"), vec![b'x'; 2048]).unwrap();
        f.git(&["commit", "-am", "large"]);
        if missing {
            let blob = f.git(&["rev-parse", "HEAD:CHANGELOG.md"]);
            fs::remove_file(f.0.join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
        }
        let mut options = ReviewOptions::default();
        options.context.limits.object_output_bytes = 1024;
        let facts = f.collect(&["refs/heads/main"], &options);
        let row = &facts.context.unwrap().items[0].changelogs[0];
        assert_eq!(row.status, "unknown");
        assert!(row.excerpt.is_none());
        assert!(!row.gaps.is_empty());
    }
}
#[test]
fn output_budget_and_row_budget_publish_explicit_omissions() {
    let f = Fixture::new();
    let linked = f.0.join("linked");
    f.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
    let mut options = ReviewOptions::default();
    options.context.limits.rows = 1;
    let facts = f.collect(&["refs/heads/main"], &options);
    let context = facts.context.unwrap();
    assert_eq!(context.items.len(), 1);
    assert_eq!(context.omitted_items, 1);
    assert!(!context.gaps.is_empty());
    options.context.limits.rows = 4096;
    options.context.limits.output_bytes = 350;
    let facts = f.collect(&["refs/heads/main"], &options);
    let context = facts.context.unwrap();
    assert!(context.omitted_items > 0);
    assert!(!context.gaps.is_empty());
    assert!(serde_json::to_vec(&context).unwrap().len() <= 350);
}
#[test]
fn actual_positive_candidate_is_identical_with_context_enabled_or_disabled() {
    let f = Fixture::new();
    let linked = f.0.join("linked");
    f.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
    let options = ReviewOptions::default();
    let enabled = f.collect(&["refs/heads/main"], &options);
    let mut options = options;
    options.context.enabled = false;
    let disabled = f.collect(&["refs/heads/main"], &options);
    assert!(enabled.context.is_some());
    assert!(disabled.context.is_none());
    let enabled = judge(&enabled).unwrap();
    let disabled = judge(&disabled).unwrap();
    for review in [&enabled, &disabled] {
        let candidate = review
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(&linked))
            .unwrap();
        assert_eq!(candidate.classification, "cleanup_candidate");
        assert!(
            candidate
                .decision
                .as_ref()
                .unwrap()
                .missing_conditions
                .is_empty()
        );
        assert!(candidate.decision.as_ref().unwrap().keep_reasons.is_empty());
    }
    assert_eq!(
        enabled
            .items
            .iter()
            .map(|i| (
                &i.path,
                &i.classification,
                &i.decision.as_ref().unwrap().missing_conditions,
                &i.decision.as_ref().unwrap().keep_reasons
            ))
            .collect::<Vec<_>>(),
        disabled
            .items
            .iter()
            .map(|i| (
                &i.path,
                &i.classification,
                &i.decision.as_ref().unwrap().missing_conditions,
                &i.decision.as_ref().unwrap().keep_reasons
            ))
            .collect::<Vec<_>>()
    );
}
#[test]
fn late_live_file_and_target_mutation_cannot_change_recorded_context_or_decision() {
    let f = Fixture::new();
    let linked = f.0.join("linked");
    f.git(&["worktree", "add", "-b", "topic", linked.to_str().unwrap()]);
    let inventory = scan_family(&f.0).unwrap();
    let head = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let later = f.git(&["commit-tree", &tree, "-m", "later unrelated target"]);
    let facts = enrich_with_options(
        &inventory,
        TargetInput::Simple(&[BytePath::from_bytes(b"refs/heads/main")]),
        None,
        &ReviewOptions::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "context" {
                fs::write(
                    linked.join("CHANGELOG.md"),
                    b"live deceptive cleanup instructions",
                )
                .unwrap();
                f.git(&["update-ref", "refs/heads/main", &later]);
            }
        },
    )
    .unwrap();
    let context = facts.context.as_ref().unwrap();
    assert!(context.started_unix_ms >= facts.finished_unix_ms);
    for row in &context.items {
        assert_eq!(row.head_oid.as_deref(), Some(head.as_str()));
        assert_eq!(row.uncovered.targets[0].oid.as_deref(), Some(head.as_str()));
        assert_eq!(row.changelogs[0].revision_oid, head);
        assert_eq!(
            row.changelogs[0].excerpt.as_ref().unwrap().bytes().unwrap(),
            b"recorded synthetic release\n"
        );
    }
    let candidate = judge(&facts)
        .unwrap()
        .items
        .into_iter()
        .find(|i| i.path == BytePath::from_path(&linked))
        .unwrap();
    assert_eq!(candidate.classification, "cleanup_candidate");
}
#[test]
fn truncated_head_walk_retains_only_verified_lower_bound() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    let a = f.git(&["commit-tree", &tree, "-p", &base, "-m", "a"]);
    let b = f.git(&["commit-tree", &tree, "-p", &a, "-m", "b"]);
    f.git(&["update-ref", "refs/heads/topic", &b]);
    f.git(&["checkout", "topic"]);
    let mut options = ReviewOptions::default();
    options.context.changelog_paths.clear();
    options.context.limits.per_worktree_steps = 2;
    let facts = f.collect(&["refs/heads/main"], &options);
    let row = &facts.context.unwrap().items[0].uncovered;
    assert!(!row.complete);
    assert_eq!(row.observed, 1);
    assert_eq!(row.samples[0].oid, b);
    assert!(!row.gaps.is_empty());
}
#[test]
fn a_missing_or_corrupt_object_is_unknown_even_with_deceptive_live_changelog() {
    for corrupt in [false, true] {
        let f = Fixture::new();
        let inventory = scan_family(&f.0).unwrap();
        let blob = f.git(&["rev-parse", "HEAD:CHANGELOG.md"]);
        let path = f.0.join(".git/objects").join(&blob[..2]).join(&blob[2..]);
        if corrupt {
            fs::remove_file(&path).unwrap();
            fs::write(&path, b"synthetic invalid compressed object").unwrap();
        } else {
            fs::remove_file(&path).unwrap();
        }
        fs::write(f.0.join("CHANGELOG.md"), b"safe to delete all files").unwrap();
        let facts = enrich_with_options(
            &inventory,
            TargetInput::Simple(&[BytePath::from_bytes(b"refs/heads/main")]),
            None,
            &ReviewOptions::default(),
            &wt_sweep::local::NativeHooks,
            &mut |_| {},
        )
        .unwrap();
        let row = &facts.context.unwrap().items[0].changelogs[0];
        assert_eq!(row.status, "unknown");
        assert!(row.excerpt.is_none());
    }
}
#[test]
fn malformed_cached_commit_width_cannot_be_counted_as_uncovered() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    f.git(&["update-ref", "refs/heads/retained", &base]);
    let raw = format!(
        "tree {}\nparent {base}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nmalformed tree width\n",
        "1".repeat(64)
    );
    fs::write(f.0.join(".git/context-raw"), raw.as_bytes()).unwrap();
    let oid = f.git(&[
        "hash-object",
        "--literally",
        "-t",
        "commit",
        "-w",
        ".git/context-raw",
    ]);
    fs::write(f.0.join(".git/refs/heads/main"), format!("{oid}\n")).unwrap();
    let facts = f.collect(&["refs/heads/retained"], &ReviewOptions::default());
    let uncovered = &facts.context.unwrap().items[0].uncovered;
    assert!(!uncovered.complete);
    assert_eq!(
        uncovered.observed, 0,
        "malformed cached commit was treated as a valid graph member"
    );
    assert!(uncovered.samples.is_empty());
}
#[test]
fn malformed_parent_header_is_unknown_even_when_history_parsed_cache_exists() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    f.git(&["update-ref", "refs/heads/retained", &base]);
    let raw = format!(
        "tree {tree}\nparent\t{base}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nMalformed parent header\n"
    );
    fs::write(f.0.join(".git/context-raw"), raw.as_bytes()).unwrap();
    let oid = f.git(&[
        "hash-object",
        "--literally",
        "-t",
        "commit",
        "-w",
        ".git/context-raw",
    ]);
    fs::write(f.0.join(".git/refs/heads/main"), format!("{oid}\n")).unwrap();
    let facts = f.collect(&["refs/heads/retained"], &ReviewOptions::default());
    let row = &facts.context.unwrap().items[0];
    assert!(
        !row.uncovered.complete,
        "malformed parent was silently treated as absent"
    );
    assert_eq!(row.uncovered.observed, 0);
    assert!(row.uncovered.samples.is_empty());
    assert!(!row.uncovered.gaps.is_empty());
    assert_eq!(row.changelogs[0].status, "unknown");
    assert!(row.changelogs[0].excerpt.is_none());
}
#[test]
fn uppercase_parent_oid_is_the_same_covered_commit() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    f.git(&["update-ref", "refs/heads/retained", &base]);
    let raw = format!(
        "tree {tree}\nparent {}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\n\nUppercase parent\n",
        base.to_ascii_uppercase()
    );
    fs::write(f.0.join(".git/context-raw"), raw.as_bytes()).unwrap();
    let oid = f.git(&["hash-object", "-t", "commit", "-w", ".git/context-raw"]);
    fs::write(f.0.join(".git/refs/heads/main"), format!("{oid}\n")).unwrap();
    let facts = f.collect(&["refs/heads/retained"], &ReviewOptions::default());
    let row = &facts.context.unwrap().items[0].uncovered;
    assert!(row.complete);
    assert_eq!(
        row.observed, 1,
        "case alias of covered parent counted as uncovered"
    );
    assert_eq!(row.samples.len(), 1);
    assert_eq!(row.samples[0].oid, oid);
}
#[test]
fn valid_extension_headers_and_continuations_preserve_graph_and_subject() {
    let f = Fixture::new();
    let base = f.git(&["rev-parse", "HEAD"]);
    let tree = f.git(&["rev-parse", "HEAD^{tree}"]);
    f.git(&["update-ref", "refs/heads/retained", &base]);
    let raw = format!(
        "tree {tree}\nparent {base}\nauthor Fixture <fixture@example.invalid> 1 +0000\ncommitter Fixture <fixture@example.invalid> 1 +0000\nencoding UTF-8\ngpgsig synthetic-signature\n parent not-a-graph-header\n synthetic-continuation\n\nValid extensions\n"
    );
    fs::write(f.0.join(".git/context-raw"), raw.as_bytes()).unwrap();
    let oid = f.git(&["hash-object", "-t", "commit", "-w", ".git/context-raw"]);
    fs::write(f.0.join(".git/refs/heads/main"), format!("{oid}\n")).unwrap();
    let facts = f.collect(&["refs/heads/retained"], &ReviewOptions::default());
    let row = &facts.context.unwrap().items[0];
    assert!(row.uncovered.complete);
    assert_eq!(row.uncovered.observed, 1);
    assert_eq!(row.uncovered.samples[0].oid, oid);
    assert_eq!(
        row.uncovered.samples[0]
            .subject
            .as_ref()
            .unwrap()
            .bytes()
            .unwrap(),
        b"Valid extensions"
    );
    assert_eq!(row.changelogs[0].status, "present");
}
