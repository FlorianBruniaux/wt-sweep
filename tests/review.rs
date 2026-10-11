// Panics are assertion failures in these isolated tests, never product handling.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;
use wt_sweep::model::{BytePath, Inventory, Observations, Review, ReviewItem, ScanScope};

fn observations() -> Observations {
    let path = BytePath::from_bytes(b"/private/tmp/synthetic-pure-judgement-input");
    let inventory = Inventory::new(ScanScope::Repo { path: path.clone() });
    Observations {
        context: None,
        shared_stashes: Vec::new(),
        inventory,
        history_metrics: Default::default(),
        collection_metrics: None,
        items: vec![ReviewItem {
            worktree_state: None,
            decision: None,
            private_history: None,
            path,
            local_summary: None,
            branch: None,
            head: None,
            subject: None,
            subject_truncated: false,
            classification: "to_triage".into(),
            observations: Vec::new(),
            history: Vec::new(),
            history_omitted_targets: 0,
            requests: Vec::new(),
            risks: Vec::new(),
            unknowns: vec!["integration target unknown".into()],
        }],
        forge: None,
        started_unix_ms: 100,
        finished_unix_ms: 200,
    }
}

#[test]
fn judgement_is_deterministic_for_fixed_observations() {
    let input = observations();
    let first = wt_sweep::review::judge(&input).expect("pure judgement");
    let second = wt_sweep::review::judge(&input).expect("repeated judgement");
    assert_eq!(
        first.started_unix_ms, 100,
        "judgement invented a clock reading"
    );
    assert_eq!(
        first.finished_unix_ms, 200,
        "judgement invented a clock reading"
    );
    assert_eq!(
        serde_json::to_value(first).unwrap(),
        serde_json::to_value(second).unwrap()
    );
}

#[test]
fn review_schema_accepts_positive_report_but_refuses_action_and_suggestion_fields() {
    let report = wt_sweep::review::judge(&observations()).expect("valid report");
    let valid = serde_json::to_value(report).expect("serialize report");
    assert!(serde_json::from_value::<Review>(valid.clone()).is_ok());
    for field in ["actions", "suggestions", "approval", "apply_plan"] {
        let mut invalid = valid.clone();
        invalid[field] = json!([{"delete": "/synthetic/never-executed"}]);
        assert!(
            serde_json::from_value::<Review>(invalid).is_err(),
            "accepted {field}"
        );
    }
    let mut invalid = valid;
    invalid["items"][0]["action"] = "remove".into();
    assert!(serde_json::from_value::<Review>(invalid).is_err());
}

#[test]
fn current_local_counts_render_partial_and_unknown_without_false_zero() {
    use wt_sweep::model::{LocalCount, LocalSummary};
    let mut facts = observations();
    facts.items[0].local_summary = Some(LocalSummary {
        started_unix_ms: 101,
        finished_unix_ms: 102,
        staged: LocalCount {
            observed: 2,
            complete: true,
            ..Default::default()
        },
        unstaged: LocalCount::default(),
        ignored: LocalCount {
            observed: 1,
            ..Default::default()
        },
        temporal_consistency: "not_guaranteed".into(),
        ..Default::default()
    });
    let report = wt_sweep::review::judge(&facts).unwrap();
    let rendered = wt_sweep::presentation::detail(&report);
    assert!(
        rendered.contains("staged=2"),
        "exact local count missing: {rendered}"
    );
    assert!(
        rendered.contains("unstaged=UNKNOWN"),
        "incomplete zero rendered as exact: {rendered}"
    );
    assert!(
        rendered.contains("ignored=>=1"),
        "positive partial count missing: {rendered}"
    );
    assert!(
        rendered.contains("101..102"),
        "current observation interval absent: {rendered}"
    );
    assert_eq!(
        report.items[0].classification, "keep",
        "observed local work must be retained despite another incomplete count"
    );
}

#[test]
fn current_review_uses_version_two_with_live_summary_contract() {
    let report = wt_sweep::review::judge(&observations()).unwrap();
    assert_eq!(report.schema, "wt-sweep.review/2");
    assert_eq!(report.policy_version, "triage/1");
}

#[test]
fn current_review_references_large_historical_manifest_without_copying_entries() {
    use wt_sweep::model::{IndexEntry, LocalInventory};
    let mut facts = observations();
    facts.inventory.local_inventories.push(LocalInventory {
        worktree: facts.items[0].path.clone(),
        started_unix_ms: 10,
        finished_unix_ms: 20,
        entries: Vec::new(),
        administration_entries: Vec::new(),
        index_entries: (0..2000)
            .map(|n| IndexEntry {
                path: BytePath::from_bytes(format!("old-file-{n:04}").as_bytes()),
                mode: 0o100644,
                oid: "a".repeat(40),
                stage: 0,
            })
            .collect(),
        traversal_complete: true,
        temporal_consistency: "not_guaranteed".into(),
        gaps: Vec::new(),
        not_collected: vec!["ACLs".into()],
        content_digest_sha256: "b".repeat(64),
    });
    let report = wt_sweep::review::judge(&facts).unwrap();
    let value = serde_json::to_value(report).unwrap();
    assert!(
        value.get("local_inventories").is_none(),
        "historical arrays duplicated"
    );
    assert_eq!(
        value["local_inventory_refs"][0]["content_digest_sha256"],
        "b".repeat(64)
    );
    assert_eq!(value["local_inventory_refs"][0]["index_entry_count"], 2000);
    assert_eq!(value["local_inventory_refs"][0]["started_unix_ms"], 10);
    assert_eq!(value["local_inventory_refs"][0]["not_collected_count"], 1);
    assert!(serde_json::to_vec(&value).unwrap().len() < 4096);
}

#[test]
fn disk_observations_are_digested_and_rendered_without_changing_verdicts() {
    use wt_sweep::disk::{DiskBytes, DiskKind, DiskLimits, DiskMeasurement, DiskUsage};
    let mut facts = observations();
    let without_disk = wt_sweep::review::judge(&facts).unwrap();
    facts.inventory.disk_usage = Some(DiskUsage {
        storage: None,
        requested_paths: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
        elapsed_ms: 10,
        limits: DiskLimits::default(),
        complete: false,
        temporal_consistency: "not_guaranteed".into(),
        unique_bytes: DiskBytes {
            logical: 1024,
            allocated: 4096,
        },
        measurements: vec![DiskMeasurement {
            requested: true,
            path: facts.items[0].path.clone(),
            kind: DiskKind::Worktree,
            identity: None,
            bytes: Some(DiskBytes {
                logical: 1024,
                allocated: 4096,
            }),
            complete: false,
            entries_checked: 2,
            excluded_registered_roots: 0,
            elapsed_ms: 10,
            gaps: vec![],
            gaps_omitted: 0,
        }],
    });
    let with_disk = wt_sweep::review::judge(&facts).unwrap();
    let value = serde_json::to_value(&with_disk).unwrap();
    assert_eq!(
        value["disk_usage"],
        serde_json::to_value(&facts.inventory.disk_usage).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&with_disk.items).unwrap(),
        serde_json::to_value(&without_disk.items).unwrap()
    );
    assert_ne!(with_disk.digest_sha256, without_disk.digest_sha256);
    let mut tampered = value.clone();
    tampered["disk_usage"]["unique_bytes"]["allocated"] = 8192.into();
    let tampered: Review = serde_json::from_value(tampered).unwrap();
    assert_ne!(
        wt_sweep::output::canonical_digest(&tampered).unwrap(),
        with_disk.digest_sha256
    );
    let repeated = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(value, serde_json::to_value(&repeated).unwrap());
    let detail = wt_sweep::presentation::detail(&with_disk);
    let summary = wt_sweep::presentation::summary(&with_disk);
    assert_eq!(detail, wt_sweep::presentation::detail(&repeated));
    assert_eq!(summary, wt_sweep::presentation::summary(&repeated));
    for text in [&detail, &summary] {
        assert!(text.contains("4.00 KiB"));
        assert!(text.contains("INCOMPLETE: totals cover observed entries only"));
        assert!(text.contains("Disk observation interval (Unix ms): 10..20"));
    }
    assert!(wt_sweep::presentation::detail_bounded(&with_disk, detail.len() - 1).is_err());
    let mut old = value;
    old.as_object_mut().unwrap().remove("disk_usage");
    assert!(
        serde_json::from_value::<Review>(old).is_ok(),
        "old reviews remain readable"
    );
}
