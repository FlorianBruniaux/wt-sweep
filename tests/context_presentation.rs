#![allow(clippy::unwrap_used)]
use wt_sweep::{model::*, output::canonical_digest};
fn bp(value: &[u8]) -> BytePath {
    BytePath::from_bytes(value)
}
fn report() -> Review {
    let path = bp(b"/fixture/linked");
    let mut report = wt_sweep::review::judge(&Observations {
        context: None,
        inventory: Inventory::new(ScanScope::Repo { path: path.clone() }),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        items: vec![],
        forge: None,
        started_unix_ms: 10,
        finished_unix_ms: 20,
    })
    .unwrap();
    report.items.push(ReviewItem {
        worktree_state: None,
        decision: None,
        path: path.clone(),
        private_history: None,
        history: vec![],
        history_omitted_targets: 0,
        local_summary: None,
        branch: None,
        head: Some("a".repeat(40)),
        subject: None,
        subject_truncated: false,
        classification: "verify".into(),
        observations: vec![],
        requests: vec![ForgeRequest {
            provider: "github".into(),
            host: Some("forge.example".into()),
            project: Some("fixture/project".into()),
            iid: 7,
            source_project: Some("fixture/project".into()),
            target_project: Some("fixture/project".into()),
            source_branch: None,
            target_branch: None,
            state: "open".into(),
            title: Some("synthetic\nrequest".into()),
            head_oid: Some("a".repeat(40)),
            merge_oid: None,
            updated_at: Some("2026-01-02T03:04:05Z".into()),
            url: None,
            source_index: 0,
        }],
        risks: vec![],
        unknowns: vec![],
    });
    report.forge = Some(ForgeExport {
        schema: FORGE_SCHEMA.into(),
        requests: vec![],
        sources: vec![ForgeSource {
            raw_path: bp(b"/fixture/raw.json"),
            sha256: "c".repeat(64),
            receipt_path: None,
            receipt_sha256: None,
            receipt_verified: false,
            started_unix_ms: Some(1),
            finished_unix_ms: Some(2),
            execution: None,
            gaps: vec![],
        }],
        coverage: ForgeCoverage {
            pagination_complete: false,
            temporal_consistency: "unknown".into(),
            absence_proven: false,
            gaps: vec![],
        },
    });
    report.context = Some(ReviewContext {
        started_unix_ms: 30,
        finished_unix_ms: 40,
        items: vec![WorktreeContext {
            path,
            common_dir: bp(b"/fixture/main/.git"),
            family_identity: Identity {
                device: 1,
                inode: 2,
            },
            head_oid: Some("a".repeat(40)),
            uncovered: UncoveredCommits {
                targets: vec![ContextTarget {
                    reference: bp(b"refs/heads/main"),
                    oid: Some("b".repeat(40)),
                }],
                observed: 2,
                complete: false,
                samples: vec![CommitContext {
                    oid: "d".repeat(40),
                    subject: Some(bp(b"synthetic\nsubject\xff")),
                    subject_truncated: true,
                }],
                samples_omitted: 1,
                gaps: vec!["synthetic traversal limit".into()],
            },
            changelogs: vec![ChangelogContext {
                revision_oid: "a".repeat(40),
                path: bp(b"CHANGELOG.md"),
                tree_oid: Some("e".repeat(40)),
                blob_oid: Some("f".repeat(40)),
                status: "present".into(),
                blob_bytes: Some(25),
                excerpt: Some(bp(b"merged\n\x1b[2J\xff")),
                excerpt_truncated: false,
                gaps: vec![],
            }],
            gaps: vec![],
        }],
        omitted_items: 1,
        gaps: vec!["synthetic output limit".into()],
    });
    report.digest_sha256 = canonical_digest(&report).unwrap();
    report
}
#[test]
fn context_renderer_binds_recorded_oids_and_separate_intervals_with_escaped_text() {
    let report = report();
    let detail = wt_sweep::presentation::detail(&report);
    for expected in [
        "Decision evidence interval: 10..20",
        "Context collection interval: 30..40",
        "no continuity after decision checks",
        ">=2",
        "synthetic\\nsubject\\xff",
        "CHANGELOG.md",
        "merged\\n\\x1b[2J\\xff",
        "Context rows omitted: 1",
        "synthetic output limit",
        "squash, cherry-pick and revert",
    ] {
        assert!(detail.contains(expected), "missing {expected}: {detail}");
    }
    for oid in ['a', 'b', 'd', 'e', 'f'] {
        assert!(detail.contains(&oid.to_string().repeat(40)));
    }
    assert!(!detail.contains('\x1b'));
}
#[test]
fn forge_renderer_exposes_recorded_provenance_without_calling_open_request_process_activity() {
    let detail = wt_sweep::presentation::detail(&report());
    for expected in [
        "forge.example",
        "fixture/project",
        "2026-01-02T03:04:05Z",
        "/fixture/raw.json",
        "receipt verified=false",
        "1..2",
        "current server state is not established",
    ] {
        assert!(detail.contains(expected), "missing {expected}: {detail}");
    }
    assert!(detail.contains(&"c".repeat(64)));
    assert!(!detail.contains("process active"));
}
