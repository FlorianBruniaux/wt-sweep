use wt_sweep::model::*;

fn report() -> Result<Review, SweepError> {
    let facts = Observations {
        context: None,
        inventory: Inventory::new(ScanScope::Repo {
            path: BytePath::from_bytes(b"/fixture"),
        }),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        items: vec![],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 30,
    };
    let mut report = wt_sweep::review::judge(&facts)?;
    for (path, class, reasons, missing) in [
        (
            b"/fixture/kept".as_slice(),
            "keep",
            vec!["local_ignored", "git_operation"],
            vec!["C3"],
        ),
        (
            b"/fixture/candidate".as_slice(),
            "cleanup_candidate",
            vec![],
            vec![],
        ),
        (
            b"/fixture/check\nnext".as_slice(),
            "verify",
            vec![],
            vec!["C1", "C4"],
        ),
    ] {
        report.items.push(ReviewItem {
            worktree_state: None,
            decision: Some(TriageDecision {
                keep_reasons: reasons.into_iter().map(str::to_owned).collect(),
                missing_conditions: missing.into_iter().map(str::to_owned).collect(),
            }),
            path: BytePath::from_bytes(path),
            private_history: None,
            history: vec![],
            history_omitted_targets: 0,
            local_summary: None,
            branch: None,
            head: Some("a".repeat(40)),
            subject: None,
            subject_truncated: false,
            classification: class.into(),
            observations: vec![],
            requests: vec![],
            risks: vec![],
            unknowns: vec!["forge not collected".into()],
        });
    }
    Ok(report)
}

#[test]
fn triage_summary_counts_decisions_without_calling_unknown_usage_inactive() -> Result<(), SweepError>
{
    let summary = wt_sweep::presentation::summary(&report()?);
    assert_eq!(summary.lines().count(), 5);
    assert!(
        summary.contains("Keep: 1; cleanup candidates: 1; verify: 1"),
        "{summary}"
    );
    assert!(summary.contains("Usage: UNKNOWN"), "{summary}");
    assert!(summary.contains("removal"), "{summary}");
    assert!(!summary.contains("Active:"), "{summary}");
    Ok(())
}

#[test]
fn detail_explains_each_reason_missing_condition_and_candidate_limits() -> Result<(), SweepError> {
    let detail = wt_sweep::presentation::detail(&report()?);
    for expected in [
        "Ignored content observed",
        "Git operation marker observed",
        "Next check C1:",
        "Next check C4:",
        "Usage UNKNOWN: confirm before removal",
        "authorizes no removal",
        "Decision evidence interval: 10..30",
        "ACLs, extended attributes",
        "/fixture/check\\nnext",
    ] {
        assert!(detail.contains(expected), "missing {expected}: {detail}");
    }
    assert!(!detail.contains("/fixture/check\nnext"));
    Ok(())
}

#[test]
fn detail_byte_limit_refuses_truncation_and_keeps_exact_boundary() -> Result<(), SweepError> {
    let report = report()?;
    let expected = wt_sweep::presentation::detail(&report);
    let before = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    assert_eq!(
        wt_sweep::presentation::detail_bounded(&report, expected.len())?,
        expected
    );
    assert!(wt_sweep::presentation::detail_bounded(&report, expected.len() - 1).is_err());
    assert!(wt_sweep::presentation::detail_bounded(&report, 0).is_err());
    let after = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    assert_eq!(before, after);
    Ok(())
}

#[test]
fn detail_and_summary_preserve_source_coverage_omissions() -> Result<(), SweepError> {
    let mut report = report()?;
    report.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::Interrupted,
        known_pending_paths: 7,
        cover: RemainingCover::RequestedScope,
    });
    report.coverage.publication = Some(PublicationOmissions {
        local_inventories: 4,
        markers: 2,
        families: 1,
        worktrees: 3,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    let detail = wt_sweep::presentation::detail(&report);
    assert!(
        detail.contains(
            "Discovery remaining: interrupted; known pending=7; conservative cover=requested scope"
        ),
        "{detail}"
    );
    assert!(
        detail
            .contains("Publication omitted: 1 families, 3 worktrees, 4 local manifests, 2 markers"),
        "{detail}"
    );
    assert!(
        detail.contains("including registered worktrees outside roots"),
        "{detail}"
    );
    let summary = wt_sweep::presentation::summary(&report);
    assert_eq!(summary.lines().count(), 5);
    assert!(
        summary.contains("discovery remaining: interrupted"),
        "{summary}"
    );
    assert!(summary.contains("unpublished worktrees: 3"), "{summary}");
    Ok(())
}
