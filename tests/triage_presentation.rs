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
    assert!(summary.lines().count() <= 60);
    assert!(
        summary.contains("Keep: 1")
            && summary.contains("Cleanup candidates: 1")
            && summary.contains("Needs verification: 1"),
        "{summary}"
    );
    assert!(
        summary.contains("Usage by agents or terminals: not checked"),
        "{summary}"
    );
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
    assert!(summary.lines().count() <= 60);
    assert!(
        summary.contains("Discovery remaining: interrupted"),
        "{summary}"
    );
    assert!(summary.contains("Unpublished worktrees: 3"), "{summary}");
    Ok(())
}

#[test]
fn complete_discovery_does_not_hide_partial_or_uncollected_local_checks() -> Result<(), SweepError>
{
    let mut report = report()?;
    report.coverage.traversal_complete = true;
    report.items[0].local_summary = Some(LocalSummary {
        worktree_identity: Some(Identity {
            device: 1,
            inode: 2,
        }),
        staged: LocalCount {
            observed: 2,
            ..Default::default()
        },
        ..Default::default()
    });
    report.items[1].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[1].path.clone(),
            reason: "whole-review local summary budget exhausted".into(),
        }],
        ..Default::default()
    });
    let summary = wt_sweep::presentation::summary(&report);
    for expected in [
        "WORKTREE SWEEP",
        "RESULTS",
        "Discovery: complete",
        "Decision checks: incomplete for 2/3 worktrees",
        "Local checks complete: 0",
        "Local checks partial: 1",
        "Local checks not collected: 2",
        "Local collection budget reached",
    ] {
        assert!(summary.contains(expected), "missing {expected}: {summary}");
    }
    Ok(())
}

#[test]
fn complete_local_counts_are_not_a_claim_of_clean_files_or_inactive_usage()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = report()?;
    let count = LocalCount {
        complete: true,
        ..Default::default()
    };
    report.items[0].local_summary = Some(LocalSummary {
        head_complete: true,
        index_complete: true,
        disk_complete: true,
        staged: LocalCount {
            observed: 2,
            ..count.clone()
        },
        unstaged: count.clone(),
        conflicts: count.clone(),
        untracked: count.clone(),
        ignored: count,
        ..Default::default()
    });
    let summary = wt_sweep::presentation::summary(&report);
    assert!(summary.contains("Local checks complete: 1"), "{summary}");
    assert!(summary.contains("Keep: 1"), "{summary}");
    assert!(
        summary.contains("Usage by agents or terminals: not checked"),
        "{summary}"
    );
    report.items[0]
        .local_summary
        .as_mut()
        .ok_or("missing fixture summary")?
        .temporal_consistency = "change_observed".into();
    assert!(wt_sweep::presentation::summary(&report).contains("Local checks complete: 0"));
    Ok(())
}

#[test]
fn repeated_limits_are_grouped_and_console_does_not_mutate_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let mut report = report()?;
    report.history_metrics.stop_reason =
        Some("per-target history traversal step budget reached".into());
    for item in &mut report.items {
        item.unknowns
            .push("required final validation quarantined after family Git timeout".into());
    }
    let before = serde_json::to_vec(&report)?;
    let summary = wt_sweep::presentation::summary(&report);
    assert_eq!(
        summary.matches("History traversal limit reached").count(),
        1,
        "{summary}"
    );
    assert_eq!(summary.matches("Git timeout").count(), 1, "{summary}");
    assert_eq!(serde_json::to_vec(&report)?, before);
    Ok(())
}

#[test]
fn local_object_and_time_caps_have_distinct_retry_guidance() -> Result<(), SweepError> {
    let mut report = report()?;
    report.items[0].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[0].path.clone(),
            reason: "HEAD blob unavailable: summary object limit reached".into(),
        }],
        ..Default::default()
    });
    report.items[1].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[1].path.clone(),
            reason: "summary time budget exhausted during Git read".into(),
        }],
        ..Default::default()
    });
    let before = serde_json::to_vec(&report).map_err(|e| {
        SweepError::new("test", "serialize", std::path::Path::new(""), e.to_string())
    })?;
    let summary = wt_sweep::presentation::summary(&report);
    assert!(summary.contains("Local object limit reached"), "{summary}");
    assert!(summary.contains("Local time budget reached"), "{summary}");
    assert!(summary.contains("--worktree ABSOLUTE_PATH"), "{summary}");
    assert!(
        summary.contains("Object and byte caps still apply"),
        "{summary}"
    );
    assert!(!summary.contains("Git timeout"), "{summary}");
    assert_eq!(
        before,
        serde_json::to_vec(&report).map_err(|e| SweepError::new(
            "test",
            "serialize",
            std::path::Path::new(""),
            e.to_string()
        ))?
    );
    Ok(())
}

#[test]
fn focused_run_names_private_budget_without_suggesting_identical_retry() -> Result<(), SweepError> {
    let mut report = report()?;
    report.items[0]
        .unknowns
        .push("private history Git process or time budget reached".into());
    report.items[0].local_summary = Some(LocalSummary {
        gaps: vec![Gap {
            path: report.items[0].path.clone(),
            reason: "HEAD blob unavailable: summary object limit reached".into(),
        }],
        ..Default::default()
    });
    for item in &mut report.items[1..] {
        item.unknowns
            .push("Not selected for evidence collection (--worktree).".into());
    }
    let summary = wt_sweep::presentation::summary(&report);
    assert!(
        summary.contains("Private history collection budget reached"),
        "{summary}"
    );
    assert!(
        summary.contains("Inspect the selected worktree's gaps"),
        "{summary}"
    );
    assert!(!summary.contains("Retry one worktree"), "{summary}");
    Ok(())
}
