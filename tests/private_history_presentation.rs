use wt_sweep::model::*;

fn facts() -> Observations {
    let path = BytePath::from_bytes(b"/synthetic/worktree-one");
    let common = BytePath::from_bytes(b"/synthetic/repo/.git");
    let root = PrivateRoot {
        oid: "a".repeat(40),
        provenance: vec![RootProvenance {
            source: BytePath::from_bytes(b"/synthetic/admin/logs/HEAD"),
            kind: "head_reflog".into(),
        }],
        result: "not_preserved_by_retained_refs".into(),
        anchor: None,
        observed_result: None,
        reason: Some("exclusive old commit\ninspect before cleanup".into()),
    };
    let item = ReviewItem {
        worktree_state: None,
        decision: None,
        path: path.clone(),
        private_history: Some(PrivateHistory {
            git_dir: Some(BytePath::from_bytes(b"/synthetic/admin")),
            git_dir_identity: Some(Identity {
                device: 1,
                inode: 3,
            }),
            started_unix_ms: 10,
            finished_unix_ms: 20,
            enumeration_complete: false,
            anchors_complete: true,
            temporal_consistency: "not_guaranteed".into(),
            sources: Vec::new(),
            roots: vec![root],
            gaps: vec!["another source unreadable".into()],
        }),
        history: Vec::new(),
        history_omitted_targets: 0,
        local_summary: None,
        branch: None,
        head: Some("b".repeat(40)),
        subject: None,
        subject_truncated: false,
        classification: "to_triage".into(),
        observations: Vec::new(),
        requests: Vec::new(),
        risks: Vec::new(),
        unknowns: Vec::new(),
    };
    let mut second = item.clone();
    second.path = BytePath::from_bytes(b"/synthetic/worktree-two");
    Observations {
        context: None,
        inventory: Inventory::new(ScanScope::Repo { path }),
        history_metrics: HistoryMetrics::default(),
        shared_stashes: vec![FamilyStashes {
            common_dir: common,
            identity: Identity {
                device: 1,
                inode: 2,
            },
            started_unix_ms: 10,
            finished_unix_ms: 20,
            enumeration_complete: true,
            temporal_consistency: "not_guaranteed".into(),
            sources: Vec::new(),
            entries: vec![StashEntry {
                oid: "c".repeat(40),
                base_oid: Some("b".repeat(40)),
                timestamp_unix: Some(1_700_000_000),
                message: Some(BytePath::from_bytes(b"WIP on topic\nforged line")),
                message_truncated: true,
                files: vec![BytePath::from_bytes(b"notes\nprivate.txt")],
                metadata_complete: true,
                files_complete: false,
                gaps: vec!["remaining file detail omitted".into()],
            }],
            gaps: Vec::new(),
        }],
        items: vec![item, second],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
    }
}

#[test]
fn shared_stashes_are_printed_once_and_partial_private_facts_remain_visible()
-> Result<(), SweepError> {
    let report = wt_sweep::review::judge(&facts())?;
    // These synthetic display facts have no family/control binding. Keep the
    // uncovered root visible without promoting an unbound observation.
    assert!(report.items.iter().all(|i| i.classification == "verify"));
    let rendered = wt_sweep::presentation::detail(&report);
    assert_eq!(rendered.matches("Shared stashes for").count(), 1);
    assert_eq!(
        rendered
            .matches(&format!("Stash {}", "c".repeat(40)))
            .count(),
        1
    );
    assert!(rendered.contains("list=complete"));
    assert!(rendered.contains("files=partial"));
    assert!(rendered.contains("1 observed roots; enumeration=partial"));
    assert!(rendered.contains("not preserved by retained refs"));
    assert!(rendered.contains("exclusive old commit\\ninspect before cleanup"));
    assert!(!rendered.contains("exclusive old commit\ninspect before cleanup"));
    assert!(rendered.contains("WIP on topic\\nforged line"));
    assert!(!rendered.contains("WIP on topic\nforged line"));
    assert!(rendered.contains("notes\\nprivate.txt"));
    assert!(rendered.contains("[truncated]"));
    Ok(())
}

#[test]
fn invalidated_anchor_is_labeled_as_an_earlier_observation() -> Result<(), SweepError> {
    let mut input = facts();
    let private = input.items[0].private_history.as_mut().ok_or_else(|| {
        SweepError::new(
            "fixture",
            "private_history",
            std::path::Path::new("/synthetic"),
            "missing fixture",
        )
    })?;
    private.roots[0].result = "unknown".into();
    private.roots[0].observed_result = Some("covered".into());
    private.roots[0].anchor = Some(RetainedAnchor {
        reference: BytePath::from_bytes(b"refs/heads/old-anchor"),
        oid: "d".repeat(40),
    });
    let rendered = wt_sweep::presentation::detail(&wt_sweep::review::judge(&input)?);
    assert!(rendered.contains("Earlier private result: covered"));
    assert!(rendered.contains("Earlier anchor: refs/heads/old-anchor"));
    assert!(!rendered.contains("Covered via: refs/heads/old-anchor"));
    Ok(())
}
