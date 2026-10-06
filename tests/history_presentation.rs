use wt_sweep::model::*;

#[test]
fn graph_evidence_is_labeled_separately_and_does_not_promote_classification()
-> Result<(), SweepError> {
    let path = BytePath::from_bytes(b"/synthetic/worktree");
    let mut facts = Observations {
        collection_metrics: None,
        context: None,
        shared_stashes: Vec::new(),
        inventory: Inventory::new(ScanScope::Repo { path: path.clone() }),
        history_metrics: HistoryMetrics {
            items_total: 1,
            first_pass_items: 1,
            completed_targets: 1,
            ..Default::default()
        },
        items: vec![ReviewItem {
            worktree_state: None,
            decision: None,
            private_history: None,
            path,
            history: vec![HistoryObservation {
                source_oid: Some("a".repeat(40)),
                target_ref: BytePath::from_bytes(b"refs/heads/main"),
                target_oid: Some("b".repeat(40)),
                result: "ancestor".into(),
                observed_result: None,
                started_unix_ms: 10,
                finished_unix_ms: 20,
                stop_reason: None,
            }],
            history_omitted_targets: 0,
            local_summary: None,
            branch: None,
            head: Some("a".repeat(40)),
            subject: None,
            subject_truncated: false,
            classification: "to_triage".into(),
            observations: Vec::new(),
            requests: Vec::new(),
            risks: Vec::new(),
            unknowns: Vec::new(),
        }],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 30,
    };
    let report = wt_sweep::review::judge(&facts)?;
    assert_eq!(report.items[0].classification, "verify");
    assert!(report.items[0].observations.is_empty());
    let rendered = wt_sweep::presentation::detail(&report);
    assert!(
        rendered.contains("History ancestor"),
        "graph result absent: {rendered}"
    );
    assert!(
        rendered.contains("commit graph only"),
        "proof level absent: {rendered}"
    );
    assert!(
        rendered.contains("1/1 worktrees"),
        "progress counts absent: {rendered}"
    );
    facts.items[0].history[0].result = "unknown".into();
    facts.items[0].history[0].observed_result = Some("ancestor".into());
    facts.items[0].history[0].stop_reason = Some("target changed\ninjected line".into());
    facts.items[0].history_omitted_targets = 2;
    let rendered = wt_sweep::presentation::detail(&wt_sweep::review::judge(&facts)?);
    assert!(rendered.contains("Earlier graph result: ancestor"));
    assert!(rendered.contains("2 history targets omitted"));
    assert!(rendered.contains("changed\\ninjected line"));
    assert!(!rendered.contains("changed\ninjected line"));
    Ok(())
}
