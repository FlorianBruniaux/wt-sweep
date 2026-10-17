#![cfg(unix)]
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{
    git::scan_family,
    local::comparison::{self, Document, ResultKind},
    model::{BytePath, Review},
    output::canonical_digest,
    review::{
        ReviewOptions, enrich_with_options, judge,
        targets::{FamilyTargets, TargetInput},
    },
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-comparison-rust-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("{e}"));
        let f = Self(path);
        f.git(&["init", "-b", "main"]);
        fs::write(f.0.join("tracked"), b"base\n").unwrap_or_else(|e| panic!("{e}"));
        f.git(&["add", "tracked"]);
        f.git(&["commit", "-m", "base"]);
        f.git(&["worktree", "add", "-b", "topic", "linked"]);
        f
    }
    fn git(&self, args: &[&str]) {
        let out = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(&self.0)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(out.status.success(), "{:?}", out.stderr);
    }
    fn run(&self, limits: Option<usize>) -> (Document, Review) {
        let (doc, review, _) = self.evidence(limits);
        (doc, review)
    }
    fn evidence(&self, limits: Option<usize>) -> (Document, Review, wt_sweep::model::Inventory) {
        let inv = scan_family(&self.0).unwrap_or_else(|e| panic!("{e}"));
        let targets = vec![BytePath::from_bytes(b"refs/heads/main")];
        let store = comparison::CaptureStore::default();
        let mut options = ReviewOptions {
            comparison: Some(store.clone()),
            worktree: Some(BytePath::from_path(&self.0.join("linked"))),
            ..Default::default()
        };
        if let Some(n) = limits {
            options.local.max_objects = n;
        }
        options.context.enabled = false;
        let obs = enrich_with_options(
            &inv,
            TargetInput::Simple(&targets),
            None,
            &options,
            &wt_sweep::local::NativeHooks,
            &mut |_| {},
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let review = judge(&obs).unwrap_or_else(|e| panic!("{e}"));
        let bound = FamilyTargets::bind(&inv, TargetInput::Simple(&targets))
            .unwrap_or_else(|e| panic!("{e}"));
        let doc = comparison::collect(
            &inv,
            &review,
            &bound,
            &store,
            None,
            options.worktree.as_ref(),
            4096,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        (doc, review, inv)
    }
    fn dirty(&self) {
        fs::write(self.0.join("linked/tracked"), b"different\n").unwrap_or_else(|e| panic!("{e}"));
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn rehash(doc: &mut Document) {
    doc.digest_sha256 = canonical_digest(doc).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn rehashed_wrong_result_is_rejected_against_persisted_reference_values() {
    let f = Fixture::new();
    f.dirty();
    let (mut doc, review) = f.run(None);
    let item = doc
        .items
        .iter_mut()
        .find(|i| !i.rows.is_empty())
        .unwrap_or_else(|| panic!("no positive rows"));
    assert_eq!(item.rows[0].result, ResultKind::Different);
    assert!(!item.rows[0].lookups.is_empty());
    item.rows[0].result = ResultKind::MatchesTarget;
    rehash(&mut doc);
    assert!(comparison::verify(&doc, &review).is_err());
}
#[test]
fn complete_empty_comparison_cannot_hide_positive_source_counts() {
    let f = Fixture::new();
    f.dirty();
    let (mut doc, review) = f.run(None);
    let item = doc
        .items
        .iter_mut()
        .find(|i| !i.rows.is_empty())
        .unwrap_or_else(|| panic!("no rows"));
    item.rows.clear();
    item.path_totals = Default::default();
    item.layer_totals = Default::default();
    for c in item.coverage.values_mut() {
        c.emitted = 0;
        c.omitted = c.observed;
    }
    item.complete = true;
    rehash(&mut doc);
    assert!(comparison::verify(&doc, &review).is_err());
}
#[test]
fn rehashed_missing_disk_absence_witness_is_rejected() {
    let f = Fixture::new();
    f.git(&["rm", "tracked"]);
    f.git(&["commit", "-m", "delete"]);
    fs::remove_file(f.0.join("linked/tracked")).unwrap_or_else(|e| panic!("{e:?}"));
    let (mut doc, review) = f.run(None);
    let row = doc
        .items
        .iter_mut()
        .flat_map(|i| &mut i.rows)
        .find(|r| r.change.layer == comparison::Layer::Disk)
        .unwrap_or_else(|| panic!("no disk absence row"));
    assert!(matches!(
        &row.change.source,
        comparison::Value::Absent { parent: Some(_) }
    ));
    row.change.source = comparison::Value::Absent { parent: None };
    rehash(&mut doc);
    assert!(comparison::verify(&doc, &review).is_err());
}
#[test]
fn comparison_object_cap_leaves_source_summary_and_verdict_intact() {
    let f = Fixture::new();
    f.dirty();
    // Source HEAD tree + blob use three objects; reference cannot take a fourth.
    let (doc, review) = f.run(Some(3));
    let item = doc
        .items
        .iter()
        .find(|i| i.collection == "collected")
        .unwrap_or_else(|| panic!("no collected item"));
    assert!(!item.complete);
    assert!(doc.partial());
    assert_eq!(item.path_totals.unknown, 1);
    let source = review
        .items
        .iter()
        .find(|i| i.path == item.path)
        .unwrap_or_else(|| panic!("missing source"));
    assert_eq!(source.classification, "keep");
    let local = source
        .local_summary
        .as_ref()
        .unwrap_or_else(|| panic!("missing summary"));
    assert!(
        local.head_complete && local.index_complete && local.disk_complete,
        "{local:?}"
    );
    assert_eq!(local.unstaged.observed, 1);
}
#[test]
fn rehashed_wrong_source_review_binding_is_rejected() {
    let f = Fixture::new();
    let (mut doc, review) = f.run(None);
    doc.review_sha256 = "0".repeat(64);
    rehash(&mut doc);
    assert!(comparison::verify(&doc, &review).is_err());
}

#[test]
fn bounded_html_and_text_refuse_instead_of_allocating_an_unbounded_report() {
    let f = Fixture::new();
    f.dirty();
    let (doc, _) = f.run(None);
    assert!(doc.html_bounded(16).is_err());
    assert!(doc.text_bounded(16).is_err());
}

#[test]
fn final_source_payload_mutation_cannot_reuse_a_previous_positive_capture() {
    let f = Fixture::new();
    f.dirty();
    let inv = scan_family(&f.0).unwrap_or_else(|e| panic!("{e}"));
    let refs = vec![BytePath::from_bytes(b"refs/heads/main")];
    let store = comparison::CaptureStore::default();
    let options = ReviewOptions {
        comparison: Some(store.clone()),
        worktree: Some(BytePath::from_path(&f.0.join("linked"))),
        ..Default::default()
    };
    let facts = enrich_with_options(
        &inv,
        TargetInput::Simple(&refs),
        None,
        &options,
        &wt_sweep::local::NativeHooks,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let review = judge(&facts).unwrap_or_else(|e| panic!("{e}"));
    fs::write(f.0.join("linked/tracked"), b"late writer\n").unwrap_or_else(|e| panic!("{e}"));
    let bound =
        FamilyTargets::bind(&inv, TargetInput::Simple(&refs)).unwrap_or_else(|e| panic!("{e}"));
    let doc = comparison::collect(
        &inv,
        &review,
        &bound,
        &store,
        None,
        options.worktree.as_ref(),
        4096,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let item = doc
        .items
        .iter()
        .find(|i| i.collection == "collected")
        .unwrap_or_else(|| panic!("missing item"));
    assert_eq!(item.path_totals.unknown, 1);
    assert!(!item.complete);
}

#[test]
fn omitted_projection_paths_never_become_a_complete_zero() {
    let f = Fixture::new();
    f.dirty();
    let inv = scan_family(&f.0).unwrap_or_else(|e| panic!("{e}"));
    let refs = vec![BytePath::from_bytes(b"refs/heads/main")];
    let store = comparison::CaptureStore::default();
    let options = ReviewOptions {
        comparison: Some(store.clone()),
        worktree: Some(BytePath::from_path(&f.0.join("linked"))),
        ..Default::default()
    };
    let facts = enrich_with_options(
        &inv,
        TargetInput::Simple(&refs),
        None,
        &options,
        &wt_sweep::local::NativeHooks,
        &mut |_| {},
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let review = judge(&facts).unwrap_or_else(|e| panic!("{e}"));
    for capture in store.borrow_mut().captures.values_mut() {
        capture.omitted += capture.rows.len();
        capture.rows.clear();
    }
    let bound =
        FamilyTargets::bind(&inv, TargetInput::Simple(&refs)).unwrap_or_else(|e| panic!("{e}"));
    let doc = comparison::collect(
        &inv,
        &review,
        &bound,
        &store,
        None,
        options.worktree.as_ref(),
        4096,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let item = doc
        .items
        .iter()
        .find(|i| i.collection == "collected")
        .unwrap_or_else(|| panic!("missing item"));
    assert!(item.rows.is_empty());
    assert!(!item.complete);
    assert_eq!(item.coverage[&comparison::Category::Unstaged].omitted, 1);
}

#[test]
fn rehashed_corrupted_positive_reference_hash_is_rejected() {
    let f = Fixture::new();
    f.dirty();
    f.git(&["-C", "linked", "add", "tracked"]);
    fs::write(f.0.join("linked/tracked"), b"base\n").unwrap_or_else(|e| panic!("{e}"));
    let (mut doc, review) = f.run(None);
    let row = doc
        .items
        .iter_mut()
        .flat_map(|i| &mut i.rows)
        .find(|r| r.result == ResultKind::MatchesTarget)
        .unwrap_or_else(|| panic!("missing match"));
    if let comparison::Value::Present { sha256, .. } = &mut row.lookups[0].value {
        *sha256 = "0".repeat(64);
    } else {
        panic!("missing reference value");
    }
    rehash(&mut doc);
    assert!(comparison::verify(&doc, &review).is_err());
}

#[test]
fn sidecar_sync_failure_cannot_publish_a_receipt_or_success() {
    use wt_sweep::run::{Artifact, PublicationIo, PublicationPhase};
    struct FailSync;
    impl PublicationIo for FailSync {
        fn sync(&mut self, _file: &fs::File, phase: PublicationPhase) -> std::io::Result<()> {
            if phase == PublicationPhase::ComparisonSync {
                Err(std::io::Error::other("injected sidecar sync failure"))
            } else {
                Ok(())
            }
        }
    }
    let f = Fixture::new();
    f.dirty();
    let (doc, review, inventory) = f.evidence(None);
    let base = f.0.with_extension("reports");
    fs::create_dir(&base).unwrap_or_else(|e| panic!("{e}"));
    let data = serde_json::to_vec(&review).unwrap_or_else(|e| panic!("{e}"));
    let inv = serde_json::to_vec(&inventory).unwrap_or_else(|e| panic!("{e}"));
    let cmp = serde_json::to_vec(&doc).unwrap_or_else(|e| panic!("{e}"));
    let result = wt_sweep::run::publish_with_comparison_io(
        &base,
        Artifact {
            command: "review",
            name: "review.json",
            json: &data,
            report: "fixture",
            html: None,
            partial: true,
        },
        &inventory,
        &inv,
        &cmp,
        &mut FailSync,
    );
    assert!(result.is_err());
    for entry in fs::read_dir(&base).unwrap_or_else(|e| panic!("{e}")) {
        assert!(
            !entry
                .unwrap_or_else(|e| panic!("{e}"))
                .path()
                .join("run.json")
                .exists()
        );
    }
    fs::remove_dir_all(&base).unwrap_or_else(|e| panic!("{e}"));
}

#[test]
fn sidecar_publication_refuses_a_different_saved_inventory_before_writing() {
    let f = Fixture::new();
    f.dirty();
    let (doc, review, inventory) = f.evidence(None);
    let other = Fixture::new();
    let wrong = scan_family(&other.0).unwrap_or_else(|e| panic!("{e}"));
    let base = f.0.with_extension("wrong-inventory-reports");
    fs::create_dir(&base).unwrap_or_else(|e| panic!("{e}"));
    let data = serde_json::to_vec(&review).unwrap_or_else(|e| panic!("{e}"));
    let inv = serde_json::to_vec(&wrong).unwrap_or_else(|e| panic!("{e}"));
    let cmp = serde_json::to_vec(&doc).unwrap_or_else(|e| panic!("{e}"));
    let result = wt_sweep::run::publish_with_comparison(
        &base,
        wt_sweep::run::Artifact {
            command: "review",
            name: "review.json",
            json: &data,
            report: "fixture",
            html: None,
            partial: true,
        },
        &inventory,
        &inv,
        &cmp,
    );
    assert!(result.is_err());
    assert_eq!(
        fs::read_dir(&base)
            .unwrap_or_else(|e| panic!("{e}"))
            .count(),
        0
    );
    fs::remove_dir_all(&base).unwrap_or_else(|e| panic!("{e}"));
}
