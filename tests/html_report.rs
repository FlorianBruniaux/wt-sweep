#![allow(clippy::unwrap_used)]
use wt_sweep::{
    disk::storage::{DeviceStorage, StorageUsage, VolumeSpace},
    disk::{DiskBytes, DiskKind, DiskLimits, DiskMeasurement, DiskUsage},
    model::*,
    output::canonical_digest,
    presentation::html,
    view,
};

fn bp(bytes: &[u8]) -> BytePath {
    BytePath::from_bytes(bytes)
}

fn dashboard_storage() -> StorageUsage {
    let inputs = [
        (
            7u64,
            b"/synthetic/volume-a".as_slice(),
            100_000_000_000,
            1234,
            800_000_000_000,
        ),
        (
            8u64,
            b"/synthetic/volume-b".as_slice(),
            50_000_000_000,
            0,
            1_200_000_000_000,
        ),
    ];
    StorageUsage {
        schema: "wt-sweep.storage/1".into(),
        started_unix_ms: 101,
        finished_unix_ms: 150,
        temporal_consistency: "not_guaranteed".into(),
        devices: inputs
            .into_iter()
            .map(
                |(device, path, allocated, logical, available)| DeviceStorage {
                    device,
                    bytes: DiskBytes { logical, allocated },
                    unavailable_reason: None,
                    volume: Some(VolumeSpace {
                        anchor: Identity {
                            device,
                            inode: device + 10,
                        },
                        anchor_path: bp(path),
                        mount_path: Some(bp(path)),
                        source: None,
                        filesystem: Some(bp(b"apfs")),
                        identity_verified: true,
                        capacity_bytes: Some(2_000_000_000_000),
                        free_bytes: Some(available),
                        available_bytes: Some(available),
                        method: "fstatfs".into(),
                        started_unix_ms: 101,
                        finished_unix_ms: 102,
                        gaps: vec![],
                    }),
                },
            )
            .collect(),
    }
}

fn dashboard_fixture() -> Inventory {
    let (mut source, _) = fixture();
    let usage = source.disk_usage.as_mut().unwrap();
    usage.storage = Some(dashboard_storage());
    usage.unique_bytes = DiskBytes {
        logical: 1234,
        allocated: 150_000_000_000,
    };
    source
}

#[test]
fn dashboard_keeps_volumes_distinct_and_source_intact() {
    let source = dashboard_fixture();
    let before = serde_json::to_vec(&source).unwrap();
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("data-device=\"7\""));
    assert!(report.contains("data-device=\"8\""));
    assert!(report.contains("5.0%"));
    assert!(report.contains("2.5%"));
    assert!(!report.contains("data-capacity-bytes=\"4000000000000\""));
    assert_eq!(serde_json::to_vec(&source).unwrap(), before);
    assert!(report.contains("data-disk-dashboard"));
    assert!(report.contains("aria-label=\"Measured allocation as a share of volume capacity\""));
}

#[test]
fn dashboard_legacy_inventory_explains_missing_capacity() {
    let (source, _) = fixture();
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("Capacity was not recorded in this inventory"));
    assert!(report.contains("8192 octets"));
    assert!(!report.contains("<meter"));
}

#[test]
fn dashboard_over_100_keeps_number_and_caps_meter() {
    let mut source = dashboard_fixture();
    let storage = source
        .disk_usage
        .as_mut()
        .unwrap()
        .storage
        .as_mut()
        .unwrap();
    storage.devices[0].volume.as_mut().unwrap().capacity_bytes = Some(40_000_000_000);
    storage.devices[0].volume.as_mut().unwrap().free_bytes = Some(0);
    storage.devices[0].volume.as_mut().unwrap().available_bytes = Some(0);
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("250.0%"));
    assert!(report.contains("value=\"100.0\""));
    assert!(report.contains("exceeds reported capacity"));
    let usage = source.disk_usage.as_mut().unwrap();
    usage.storage.as_mut().unwrap().devices.truncate(1);
    usage.storage.as_mut().unwrap().devices[0].bytes.allocated = u64::MAX;
    let volume = usage.storage.as_mut().unwrap().devices[0]
        .volume
        .as_mut()
        .unwrap();
    volume.capacity_bytes = Some(u64::MAX);
    volume.available_bytes = Some(0);
    volume.free_bytes = Some(0);
    usage.unique_bytes.allocated = u64::MAX;
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("data-capacity-bytes=\"18446744073709551615\""));
    assert!(report.contains("100.0%"));
}

#[test]
fn dashboard_unknown_schema_or_broken_totals_refuses_percentages() {
    for case in 0..4 {
        let mut source = dashboard_fixture();
        let storage = source
            .disk_usage
            .as_mut()
            .unwrap()
            .storage
            .as_mut()
            .unwrap();
        match case {
            0 => storage.schema = "wt-sweep.storage/99".into(),
            1 => storage.devices[0].bytes.allocated += 1,
            2 => storage.devices[1].device = 7,
            _ => storage.devices[0].bytes.logical += 1,
        }
        let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
        assert!(report.contains("Captured volume data is unsupported or inconsistent"));
        assert!(!report.contains("<meter"));
        assert_eq!(report.matches("data-worktree=\"").count(), 2);
    }
}

#[test]
fn dashboard_escape_names_and_unknown_capacity_without_hiding_measured_bytes() {
    let mut source = dashboard_fixture();
    let volume = source
        .disk_usage
        .as_mut()
        .unwrap()
        .storage
        .as_mut()
        .unwrap()
        .devices[0]
        .volume
        .as_mut()
        .unwrap();
    volume.mount_path = Some(bp(b"/bad<&\"\xff"));
    volume.capacity_bytes = Some(0);
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("/bad&lt;&amp;&quot;\\xff"));
    assert!(!report.contains("data-capacity-bytes=\"0\""));
    assert!(report.contains("Partial measurement"));
    assert!(report.contains("Capacity unavailable for this volume"));
    assert!(report.contains("100000000000 octets"));
}

#[test]
fn dashboard_review_uses_full_source_counts_and_keeps_four_coverage_states() {
    let inventory = dashboard_fixture();
    let (_, mut review) = fixture();
    review.disk_usage = inventory.disk_usage.clone();
    let usage = review.disk_usage.as_mut().unwrap();
    let mut complete = usage.measurements[0].clone();
    complete.complete = true;
    let mut missing = complete.clone();
    missing.bytes = None;
    missing.complete = false;
    let mut omitted = missing.clone();
    omitted.requested = false;
    usage.measurements.extend([complete, missing, omitted]);
    review.inventory_sha256 = canonical_digest(&inventory).unwrap();
    review.digest_sha256 = canonical_digest(&review).unwrap();
    let selected = view::build(
        &inventory,
        &review,
        ReviewFilters {
            verdict: Some("keep".into()),
            ..Default::default()
        },
        "repo",
    )
    .unwrap();
    assert_eq!(selected.selected_paths.len(), 1);
    let report = html::review(&selected, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("data-cleanup-count=\"keep\">1"));
    assert!(report.contains("data-cleanup-count=\"cleanup_candidate\">1"));
    assert!(report.contains("data-cleanup-count=\"verify\">0"));
    assert!(
        report.contains(
            "Working directories: 1 complete; 1 partial; 1 not measured; 1 not requested"
        )
    );
    assert!(report.contains("data-device=\"8\""));
}

#[test]
fn dashboard_fixture_export() {
    let source = dashboard_fixture();
    let before = serde_json::to_vec(&source).unwrap();
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("<meter"));
    assert!(report.contains("Captured volume statistics"));
    assert_eq!(serde_json::to_vec(&source).unwrap(), before);
    if let Some(path) = std::env::var_os("WT_SWEEP_DASHBOARD_FIXTURE_DIR") {
        use std::io::Write;
        let path = std::path::PathBuf::from(path).join("report.html");
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap();
        file.write_all(report.as_bytes()).unwrap();
    }
}

#[test]
fn dashboard_no_measurements_distinguishes_unavailable_from_invalid_storage() {
    let (mut source, _) = fixture();
    source.disk_usage = None;
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("Sizes and capacity are unavailable in this inventory"));
    assert!(!report.contains("The measured size remains available"));
    let (mut source, _) = fixture();
    let usage = source.disk_usage.as_mut().unwrap();
    usage.measurements.clear();
    usage.unique_bytes = DiskBytes::default();
    let mut storage = dashboard_storage();
    storage.devices.clear();
    usage.storage = Some(storage);
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("No measured device has captured capacity"));
    assert!(!report.contains("unsupported or inconsistent"));
    assert!(!report.contains("<meter"));
}
fn fixture() -> (Inventory, Review) {
    let mut inventory = Inventory::new(ScanScope::Roots {
        paths: vec![bp(b"/synthetic")],
    });
    inventory.started_unix_ms = 100;
    inventory.finished_unix_ms = 200;
    inventory.coverage.gaps = vec![Gap {
        path: bp(b"/synthetic/refused"),
        reason: "permission <denied>".into(),
    }];
    inventory.coverage.publication = Some(PublicationOmissions {
        local_inventories: 2,
        markers: 3,
        families: 4,
        worktrees: 5,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    for (n, path) in [
        b"/synthetic/a/project".as_slice(),
        b"/synthetic/b/project\xff<&\"'\n",
    ]
    .into_iter()
    .enumerate()
    {
        inventory.families.push(Family {
            common_dir: bp(
                format!("/synthetic/{}/project/.git", if n == 0 { "a" } else { "b" }).as_bytes(),
            ),
            identity: Identity {
                device: 7,
                inode: n as u64 + 10,
            },
            worktrees: vec![Worktree {
                path: bp(path),
                identity: None,
                head: Some(if n == 0 {
                    "a".repeat(40)
                } else {
                    "b".repeat(64)
                }),
                branch: if n == 0 {
                    Some(bp(b"refs/heads/topic<&\""))
                } else {
                    None
                },
                detached: n == 1,
                bare: false,
                locked: None,
                prunable: None,
                path_state: DirState::Present,
            }],
        });
    }
    inventory.disk_usage = Some(DiskUsage {
        storage: None,
        requested_paths: None,
        started_unix_ms: 101,
        finished_unix_ms: 150,
        elapsed_ms: 49,
        limits: DiskLimits::default(),
        complete: false,
        temporal_consistency: "not_guaranteed".into(),
        unique_bytes: DiskBytes {
            logical: 1234,
            allocated: 8192,
        },
        measurements: vec![DiskMeasurement {
            requested: true,
            path: inventory.families[0].worktrees[0].path.clone(),
            kind: DiskKind::Worktree,
            identity: None,
            bytes: Some(DiskBytes {
                logical: 1234,
                allocated: 4096,
            }),
            complete: false,
            entries_checked: 3,
            excluded_registered_roots: 0,
            elapsed_ms: 49,
            gaps: vec![Gap {
                path: bp(b"/synthetic/a/ignored"),
                reason: "disk refused".into(),
            }],
            gaps_omitted: 6,
        }],
    });
    let observations = Observations {
        context: None,
        inventory: inventory.clone(),
        shared_stashes: vec![],
        history_metrics: Default::default(),
        collection_metrics: None,
        items: vec![],
        forge: None,
        started_unix_ms: 201,
        finished_unix_ms: 300,
    };
    let mut review = wt_sweep::review::judge(&observations).unwrap();
    review.items = inventory
        .families
        .iter()
        .enumerate()
        .map(|(n, family)| {
            let tree = &family.worktrees[0];
            ReviewItem {
                path: tree.path.clone(),
                branch: tree.branch.clone(),
                head: tree.head.clone(),
                subject: Some(bp(b"subject </script><script>alert(1)</script>")),
                subject_truncated: n == 1,
                classification: if n == 0 { "cleanup_candidate" } else { "keep" }.into(),
                worktree_state: Some(WorktreeState {
                    observed: Some(tree.clone()),
                    common_dir: family.common_dir.clone(),
                    family_identity: Some(family.identity.clone()),
                    git_dir: None,
                    git_dir_identity: None,
                    role: "linked".into(),
                    protections: vec![],
                    protection_complete: false,
                    operations: vec![],
                    operations_complete: false,
                    checks: "partial".into(),
                    started_unix_ms: 201,
                    finished_unix_ms: 300,
                    sources: vec![],
                    gaps: vec!["controls absent".into()],
                }),
                decision: Some(TriageDecision {
                    keep_reasons: if n == 1 {
                        vec!["local_staged".into()]
                    } else {
                        vec![]
                    },
                    missing_conditions: if n == 1 {
                        vec!["C2".into(), "C5".into()]
                    } else {
                        vec![]
                    },
                }),
                private_history: None,
                history: vec![],
                history_omitted_targets: 0,
                local_summary: None,
                observations: vec![],
                requests: vec![],
                risks: vec!["risk </script>\u{202e}hello".into()],
                unknowns: vec!["unknown & unfinished".into()],
            }
        })
        .collect();
    review.digest_sha256 = canonical_digest(&review).unwrap();
    (inventory, review)
}

#[test]
fn inventory_puts_counts_and_one_visible_flat_list_before_closed_technical_details() {
    let (mut inventory, _) = fixture();
    let original = inventory.families[0].worktrees[0].clone();
    for (path, bare, state) in [
        (b"/synthetic/0-bare".as_slice(), true, DirState::Present),
        (b"/synthetic/0-missing".as_slice(), false, DirState::Absent),
        (
            b"/synthetic/0-unknown".as_slice(),
            false,
            DirState::Unknown {
                reason: "access <refused>".into(),
            },
        ),
    ] {
        let mut row = original.clone();
        row.path = bp(path);
        row.bare = bare;
        row.path_state = state;
        inventory.families[0].worktrees.push(row);
    }
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    let overview = report.find("id=\"overview\"").unwrap();
    let list = report.find("id=\"worktree-families\"").unwrap();
    let disk = report.find("id=\"disk-priority\"").unwrap();
    assert!(
        overview < list && list < disk,
        "counts and list must lead the report"
    );
    let table = &report[list..disk];
    assert!(table.split_once('>').unwrap().0.contains("open"));
    assert!(table.contains("data-flat-list=\"\""));
    assert_eq!(table.matches("data-row-group").count(), 1);
    assert_eq!(table.matches("data-worktree=\"").count(), 5);
    assert!(
        !table.contains("class=\"family\""),
        "no repeated family headers before rows"
    );
    for column in [
        "Repository",
        "Branch",
        "Path",
        "Measured size",
        "Directory state",
    ] {
        assert!(table.contains(&format!("scope=\"col\">{column}</th>")));
    }
    for state in ["present", "bare", "absent", "unknown"] {
        assert!(table.contains(&format!("data-state=\"{state}\"")));
    }
    assert!(
        table.find("data-state=\"present\"").unwrap() < table.find("data-state=\"bare\"").unwrap()
    );
    assert!(table.contains("access &lt;refused&gt;"));
    assert!(table.contains("Recorded evidence"));
    assert!(report[..list].contains("Cleanup analysis not performed"));
    assert!(!report[..list].contains("Not established"));
    for id in ["disk-priority", "disk", "coverage"] {
        let opening = report
            .split_once(&format!("id=\"{id}\""))
            .unwrap()
            .1
            .split_once('>')
            .unwrap()
            .0;
        assert!(
            !opening.contains("open"),
            "technical section {id} starts closed"
        );
    }
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
}

#[test]
fn inventory_summary_separates_present_directories_bare_and_unavailable_entries() {
    let (mut inventory, _) = fixture();
    let original = inventory.families[0].worktrees[0].clone();
    for (path, bare, state) in [
        (b"/synthetic/bare".as_slice(), true, DirState::Present),
        (b"/synthetic/missing".as_slice(), false, DirState::Absent),
        (
            b"/synthetic/unknown".as_slice(),
            false,
            DirState::Unknown {
                reason: "not checked".into(),
            },
        ),
        (
            b"/synthetic/inaccessible".as_slice(),
            false,
            DirState::Inaccessible {
                reason: "denied".into(),
            },
        ),
        (
            b"/synthetic/read-error".as_slice(),
            false,
            DirState::ReadError {
                reason: "read failed".into(),
            },
        ),
        (
            b"/synthetic/volume-unavailable".as_slice(),
            false,
            DirState::VolumeUnavailable {
                reason: "not mounted".into(),
            },
        ),
    ] {
        let mut tree = original.clone();
        tree.path = bp(path);
        tree.bare = bare;
        tree.path_state = state;
        inventory.families[0].worktrees.push(tree);
    }
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    let overview = report
        .split_once("id=\"overview\"")
        .unwrap()
        .1
        .split_once("Requested scan scope")
        .unwrap()
        .0;
    for expected in [
        "8 Git registrations across 2 repository families",
        "data-inventory-count=\"present\">2</strong>",
        "data-inventory-count=\"bare\">1</strong>",
        "data-inventory-count=\"unavailable\">5</strong>",
        "1 absent; 4 not verified",
    ] {
        assert!(overview.contains(expected), "missing {expected}");
    }
    assert_eq!(report.matches("data-worktree=\"").count(), 8);

    inventory.families.clear();
    let empty = html::inventory(&inventory, 1_000_000).unwrap();
    for category in ["present", "bare", "unavailable"] {
        assert!(empty.contains(&format!("data-inventory-count=\"{category}\">0</strong>")));
    }
}

#[test]
fn inventory_retains_distinct_families_bytes_sizes_and_untriaged_status() {
    let (inventory, _) = fixture();
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for expected in [
        "/synthetic/a/project/.git",
        "/synthetic/b/project/.git",
        "project\\xff&lt;&amp;&quot;&#39;\\n",
        "refs/heads/topic&lt;&amp;&quot;",
        "detached",
        "Cleanup analysis not performed",
        "1,23 Ko",
        "8,19 Ko",
        "4,10 Ko",
        "partial",
        "disk refused",
        "Additional disk gaps: 6",
        "permission &lt;denied&gt;",
        "5 worktrees",
        "Unix ms",
        "not_guaranteed",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert_eq!(report.matches("data-worktree=\"").count(), 2);
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
}

#[test]
fn review_preserves_source_and_view_totals_and_exact_decisions_without_script_injection() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let before = serde_json::to_vec(&selected).unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    for expected in [
        "cleanup_candidate",
        "keep",
        "local_staged",
        "C2",
        "C5",
        "controls absent",
        "unknown &amp; unfinished",
        "&lt;/script&gt;&lt;script&gt;alert(1)&lt;/script&gt;",
        "\\u{202e}",
        "No removal authorized",
        "8,19 Ko",
        &"b".repeat(64),
        &review.digest_sha256,
        &selected.digest_sha256,
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(!report.contains("<script>alert(1)"));
    let scripts: Vec<&str> = report
        .split("<script>")
        .skip(1)
        .map(|part| part.split_once("</script>").unwrap().0)
        .collect();
    assert_eq!(
        scripts,
        vec![
            include_str!("../src/presentation/html/theme.js"),
            include_str!("../src/presentation/html/report.js"),
        ]
    );
    assert_eq!(report.matches("</script>").count(), 2);
    assert!(report.find("<script>").unwrap() < report.find("</head>").unwrap());
    assert_eq!(report.matches("data-worktree=\"").count(), 2);
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

#[test]
fn filtered_and_empty_views_keep_original_counts_coverage_and_deduplicated_total() {
    let (inventory, review) = fixture();
    for (verdict, rows) in [("keep", 1), ("verify", 0)] {
        let selected = view::build(
            &inventory,
            &review,
            ReviewFilters {
                verdict: Some(verdict.into()),
                ..Default::default()
            },
            "repo",
        )
        .unwrap();
        let report = html::review(&selected, 1_000_000).unwrap();
        assert_eq!(report.matches("data-worktree=\"").count(), rows);
        for expected in [
            "Source totals",
            "Selected totals",
            "8,19 Ko",
            "permission &lt;denied&gt;",
            "5 worktrees",
            "1 cleanup_candidate",
            "1 keep",
        ] {
            assert!(report.contains(expected), "missing {expected}: {report}");
        }
        if rows == 0 {
            assert!(report.contains("No worktrees selected"));
        }
    }
}

#[test]
fn output_limit_is_exact_and_refuses_a_partial_document() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let full = html::review(&selected, 1_000_000).unwrap();
    assert!(!full.is_empty(), "renderer must produce a document");
    assert_eq!(html::review(&selected, full.len()).unwrap(), full);
    assert_eq!(
        html::review(&selected, full.len() - 1).unwrap_err().code,
        "publication_limit"
    );
    assert!(html::inventory(&inventory, 0).is_err());
    let mut large = inventory;
    large.coverage.gaps[0].reason = "<&".repeat(1_000_000);
    assert_eq!(
        html::inventory(&large, 4096).unwrap_err().code,
        "publication_limit"
    );
}

#[test]
fn missing_measurements_and_current_branch_uncertainty_stay_visible() {
    let (mut inventory, _) = fixture();
    inventory.disk_usage = None;
    inventory.families[0].worktrees[0].branch = None;
    inventory.families[0].worktrees[0].detached = false;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    assert!(report.contains("Not measured"));
    assert!(report.contains("Branch UNKNOWN"));
    assert!(!report.contains("0 o</span>"));
}

#[test]
fn family_index_links_to_each_distinct_family_without_using_hostile_names_as_ids() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    for report in [
        html::inventory(&inventory, 1_000_000).unwrap(),
        html::review(&selected, 1_000_000).unwrap(),
    ] {
        assert!(report.contains("href=\"#family-0\""));
        assert!(report.contains("href=\"#family-1\""));
        assert!(report.contains("id=\"family-0\""));
        assert!(report.contains("id=\"family-1\""));
    }
}

#[test]
fn invalid_hex_paths_and_ambiguous_or_missing_selected_rows_are_refused() {
    let (mut inventory, report) = fixture();
    let mut selected = view::build(&inventory, &report, Default::default(), "repo").unwrap();
    selected
        .selected_paths
        .push(selected.selected_paths[0].clone());
    assert_eq!(
        html::review(&selected, 1_000_000).unwrap_err().code,
        "invalid_review_view"
    );
    selected.selected_paths.pop();
    selected.selected_paths[0] = bp(b"/missing");
    assert_eq!(
        html::review(&selected, 1_000_000).unwrap_err().code,
        "invalid_review_view"
    );
    for bad in ["00", "abcdF", "' onclick='alert(1)"] {
        inventory.families[0].worktrees[0].path.hex = bad.into();
        assert_eq!(
            html::inventory(&inventory, 1_000_000).unwrap_err().code,
            "invalid_path"
        );
    }
}

#[test]
fn observations_display_the_exact_utc_interval_without_replacing_original_timestamps() {
    let (mut inventory, _) = fixture();
    inventory.started_unix_ms = 1_709_251_199_123;
    inventory.finished_unix_ms = 1_709_251_200_456;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    assert!(report.contains("2024-02-29 23:59:59.123 UTC"));
    assert!(report.contains("2024-03-01 00:00:00.456 UTC"));
    assert!(report.contains("1709251199123..1709251200456"));
}

#[test]
fn recorded_evidence_remains_valid_copyable_json_with_hostile_unicode_and_newlines() {
    let (inventory, review) = fixture();
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    let mut records = 0;
    for block in report.split("<pre>").skip(1) {
        let encoded = block.split("</pre>").next().unwrap();
        let json = encoded
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'")
            .replace("&amp;", "&");
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        if let Some(risks) = parsed.get("risks") {
            assert_eq!(risks, &serde_json::json!(["risk </script>\u{202e}hello"]));
            records += 1;
        }
    }
    assert_eq!(
        records, 2,
        "all selected worktree evidence must remain valid JSON"
    );
}

#[test]
fn local_manifest_summaries_retain_per_worktree_gaps_omissions_and_completeness() {
    let (mut inventory, _) = fixture();
    inventory.coverage.gaps.push(Gap {
        path: inventory.families[0].worktrees[0].path.clone(),
        reason: "local inventory is incomplete; see local_inventories gaps".into(),
    });
    inventory.local_inventories = vec![
        LocalInventory {
            worktree: bp(b"/synthetic/manifest-\xff<&"),
            started_unix_ms: 401,
            finished_unix_ms: 499,
            entries: vec![],
            index_entries: vec![],
            administration_entries: vec![],
            traversal_complete: false,
            temporal_consistency: "manifest_interval_not_atomic".into(),
            gaps: vec![Gap {
                path: bp(b"/synthetic/manifest-\xff/ignored<&"),
                reason: "unique manifest gap </summary><script>alert(7)</script>".into(),
            }],
            not_collected: vec!["unique local omission ACLs & attributes".into()],
            content_digest_sha256: "c".repeat(64),
        },
        LocalInventory {
            worktree: bp(b"/synthetic/complete-manifest"),
            started_unix_ms: 501,
            finished_unix_ms: 599,
            entries: vec![],
            index_entries: vec![],
            administration_entries: vec![],
            traversal_complete: true,
            temporal_consistency: "not_guaranteed".into(),
            gaps: vec![],
            not_collected: vec![],
            content_digest_sha256: "d".repeat(64),
        },
    ];
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for expected in [
        "Local manifest: /synthetic/manifest-\\xff&lt;&amp;",
        "Local manifest: /synthetic/complete-manifest",
        "Traversal: incomplete",
        "Traversal: complete",
        "401..499",
        "501..599",
        "manifest_interval_not_atomic",
        "/synthetic/manifest-\\xff/ignored&lt;&amp;",
        "unique manifest gap &lt;/summary&gt;&lt;script&gt;alert(7)&lt;/script&gt;",
        "unique local omission ACLs &amp; attributes",
        &"c".repeat(64),
        &"d".repeat(64),
        "not a backup",
        "Local bytes have not been revalidated",
    ] {
        assert!(report.contains(expected), "missing {expected}: {report}");
    }
    assert!(!report.contains("<script>alert(7)"));
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
    assert_eq!(
        html::inventory(&inventory, report.len() - 1)
            .unwrap_err()
            .code,
        "publication_limit"
    );
}

#[test]
fn disk_sizes_use_decimal_units_and_preserve_exact_counts() {
    for (bytes, label) in [
        (0, "0 o"),
        (999, "999 o"),
        (1000, "1,00 Ko"),
        (1234, "1,23 Ko"),
        (999_994, "999,99 Ko"),
        (999_995, "1,00 Mo"),
        (1_000_000, "1,00 Mo"),
        (38_037_770_240, "38,04 Go"),
        (33_181_323_981, "33,18 Go"),
        (1_000_000_000_000, "1,00 To"),
        (u64::MAX, "18,45 Eo"),
    ] {
        let (mut inventory, _) = fixture();
        let common = inventory.families[0].common_dir.clone();
        let usage = inventory.disk_usage.as_mut().unwrap();
        usage.unique_bytes = DiskBytes {
            logical: bytes,
            allocated: bytes,
        };
        usage.measurements[0].bytes = Some(usage.unique_bytes.clone());
        let mut shared = usage.measurements[0].clone();
        shared.path = common;
        shared.kind = DiskKind::GitCommon;
        usage.measurements.push(shared);
        let before = serde_json::to_vec(&inventory).unwrap();
        let report = html::inventory(&inventory, 1_000_000).unwrap();
        let value = format!("<span class=\"byte-size\" title=\"{bytes} octets\">{label}</span>");
        assert_eq!(
            report.matches(&value).count(),
            12,
            "overview, shortlist, totals, worktree and shared Git measurements: {label}"
        );
        assert!(report.contains(&format!("data-allocated=\"{bytes}\"")));
        assert!(report.contains("Exact totals (octets)"));
        assert!(report.contains("1 Ko = 1000 octets"));
        assert!(report.contains("partial; totals cover observed entries only"));
        assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
    }
}

#[test]
fn copy_controls_preserve_path_bytes_and_exclude_branches_and_subjects() {
    let (mut inventory, review) = fixture();
    let unicode = bp("/synthetic/été & \"notes\"/résumé".as_bytes());
    let newline = bp(b"/synthetic/line\nbreak");
    let selected = view::build(&inventory, &review, Default::default(), "repo").unwrap();
    inventory.scope = ScanScope::Roots {
        paths: vec![unicode.clone(), newline.clone()],
    };
    let before = serde_json::to_vec(&selected).unwrap();
    for report in [
        html::inventory(&inventory, 1_000_000).unwrap(),
        html::review(&selected, 1_000_000).unwrap(),
    ] {
        for path in inventory
            .families
            .iter()
            .flat_map(|f| std::iter::once(&f.common_dir).chain(f.worktrees.iter().map(|w| &w.path)))
        {
            assert!(
                report.contains(&format!("data-copy-path=\"{}\"", path.hex)),
                "missing copy control for {}",
                path.hex
            );
        }
        for non_path in [
            inventory.families[0].worktrees[0].branch.as_ref().unwrap(),
            review.items[0].subject.as_ref().unwrap(),
        ] {
            assert!(!report.contains(&format!("data-copy-path=\"{}\"", non_path.hex)));
        }
        assert!(report.contains("id=\"copy-status\""));
        assert!(report.contains("<dialog id=\"copy-dialog\""));
        assert!(report.contains("readonly"));
    }
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    for path in [
        &unicode,
        &newline,
        &inventory.coverage.gaps[0].path,
        &inventory.disk_usage.as_ref().unwrap().measurements[0].gaps[0].path,
    ] {
        assert!(report.contains(&format!("data-copy-path=\"{}\"", path.hex)));
    }
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

fn disk_priority_section(report: &str) -> &str {
    report
        .split_once("id=\"disk-priority\"")
        .unwrap()
        .1
        .split_once("<details class=\"report-section ")
        .unwrap()
        .0
}

#[test]
fn disk_priority_retains_source_scope_without_claiming_reclaimable_bytes() {
    let (mut inventory, mut review) = fixture();
    let usage = inventory.disk_usage.as_mut().unwrap();
    let original = usage.measurements[0].clone();
    for (path, bytes, complete, kind) in [
        (
            b"/synthetic/large-a".as_slice(),
            Some(6000),
            false,
            DiskKind::Worktree,
        ),
        (
            b"/synthetic/large-b".as_slice(),
            Some(6000),
            true,
            DiskKind::Worktree,
        ),
        (
            b"/synthetic/shared".as_slice(),
            Some(999999),
            true,
            DiskKind::GitCommon,
        ),
        (
            b"/synthetic/unknown".as_slice(),
            None,
            false,
            DiskKind::Worktree,
        ),
    ] {
        let mut measurement = original.clone();
        measurement.path = bp(path);
        measurement.bytes = bytes.map(|allocated| DiskBytes {
            allocated,
            logical: allocated,
        });
        measurement.complete = complete;
        measurement.kind = kind;
        usage.measurements.push(measurement);
    }
    review.disk_usage = inventory.disk_usage.clone();
    review.inventory_sha256 = canonical_digest(&inventory).unwrap();
    review.digest_sha256.clear();
    review.digest_sha256 = canonical_digest(&review).unwrap();
    let selected = view::build(
        &inventory,
        &review,
        ReviewFilters {
            path_prefix: Some(bp(b"/no-match")),
            ..Default::default()
        },
        "repo",
    )
    .unwrap();
    assert!(selected.selected_paths.is_empty());
    let before = serde_json::to_vec(&selected).unwrap();
    let report = html::review(&selected, 1_000_000).unwrap();
    let section = disk_priority_section(&report);
    assert!(
        section.contains("8,19 Ko"),
        "retain deduplicated source total"
    );
    assert!(
        !section.contains("12,00 Ko"),
        "never sum overlapping row allocations"
    );
    assert!(section.contains("Space freed by cleanup"));
    assert!(section.contains("Unknown: APFS"));
    assert!(
        section.contains("1 candidate"),
        "source count survives empty view"
    );
    assert!(section.contains("2/5 roots fully measured"));
    assert!(section.contains("2 partial; 1 not measured"));
    assert!(section.contains("Partial measured allocation"));
    assert!(section.contains("Inventory total unknown"));
    assert!(
        section.contains(
            "Working directories: 1 complete; 2 partial; 1 not measured; 0 not requested"
        )
    );
    assert!(section.contains(
        "Shared Git directories: 1 complete; 0 partial; 0 not measured; 0 not requested"
    ));
    assert!(section.contains("APFS"));
    assert!(section.contains("unaffected by filters"));
    assert!(
        !section.contains("/synthetic/shared"),
        "shared Git excluded from worktree shortlist"
    );
    assert!(
        !section.contains("/synthetic/unknown"),
        "unknown size is not ranked as zero"
    );
    assert!(
        section.find("/synthetic/large-a").unwrap() < section.find("/synthetic/large-b").unwrap()
    );
    assert!(
        section.find("/synthetic/large-b").unwrap() < section.find("/synthetic/a/project").unwrap()
    );
    assert!(section.contains("Partial observation"));
    assert!(
        report.find("id=\"disk-priority\"").unwrap()
            < report.find("No removal authorized.</strong>").unwrap()
    );
    assert_eq!(serde_json::to_vec(&selected).unwrap(), before);
}

#[test]
fn disk_priority_distinguishes_absent_measurements_from_measured_zero() {
    let (mut inventory, _) = fixture();
    for empty in [false, true] {
        let mut source = inventory.clone();
        if empty {
            source.disk_usage.as_mut().unwrap().measurements.clear();
        } else {
            source.disk_usage = None;
        }
        let report = html::inventory(&source, 1_000_000).unwrap();
        let section = disk_priority_section(&report);
        assert!(section.contains("Not measured"));
        assert!(section.contains("Cleanup analysis not performed"));
        assert!(!section.contains("0 o</span>"));
    }
    let usage = inventory.disk_usage.as_mut().unwrap();
    usage.unique_bytes = DiskBytes::default();
    usage.measurements[0].bytes = Some(DiskBytes::default());
    usage.measurements[0].complete = true;
    usage.complete = true;
    let report = html::inventory(&inventory, 1_000_000).unwrap();
    let section = disk_priority_section(&report);
    assert!(section.contains("0 o</span>"));
    assert!(section.contains("1/1 roots fully measured"));
    assert!(!section.contains("Partial measured allocation"));
    assert!(!section.contains("Inventory total unknown"));
    assert!(section.contains("Space freed by cleanup"));
    assert!(section.contains("Unknown: APFS"));
}

#[test]
fn enriched_output_limit_is_exact() {
    let inventory = dashboard_fixture();
    let full = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    assert!(full.contains("<meter"));
    assert_eq!(html::inventory(&inventory, full.len()).unwrap(), full);
    assert_eq!(
        html::inventory(&inventory, full.len() - 1)
            .unwrap_err()
            .code,
        "publication_limit"
    );
}

#[test]
fn discovery_status_is_explicit_next_to_inventory_counts() {
    let mut inventory = dashboard_fixture();
    inventory.coverage.publication = None;
    inventory.coverage.traversal_complete = false;
    inventory.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::DirectoryBudget,
        known_pending_paths: 2,
        cover: RemainingCover::RequestedScope,
    });
    let report = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    let overview = report.split("id=\"worktree-families\"").next().unwrap();
    assert!(
        overview.contains("Discovery interrupted: additional worktrees may remain undiscovered")
    );
    inventory.coverage.traversal_complete = true;
    inventory.coverage.remaining = None;
    let report = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    let overview = report.split("id=\"worktree-families\"").next().unwrap();
    assert!(overview.contains("Recorded discovery traversal completed within the requested scope"));
    assert!(!overview.contains("Discovery interrupted"));
    inventory.coverage.publication = Some(PublicationOmissions {
        local_inventories: 0,
        markers: 0,
        families: 2,
        worktrees: 3,
        cover: PublicationCover::RequestedScopeAndRegisteredWorktrees,
    });
    let report = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    let overview = report.split("id=\"worktree-families\"").next().unwrap();
    assert!(
        overview.contains("Publication omitted 3 worktree observations and 2 repository families")
    );
    inventory.coverage.publication = None;
    inventory.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::DirectoryBudget,
        known_pending_paths: 2,
        cover: RemainingCover::RequestedScope,
    });
    let report = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    assert!(
        report
            .split("id=\"worktree-families\"")
            .next()
            .unwrap()
            .contains("Discovery interrupted")
    );
}

#[test]
fn scan_limits_group_incidents_without_changing_source() {
    let mut inventory = dashboard_fixture();
    inventory.coverage.gaps = [
        (
            b"/synthetic/link-a".as_slice(),
            "symlink reaches an already visited directory; cycle or repeated alias retained",
        ),
        (
            b"/synthetic/link-b".as_slice(),
            "broken or cyclic symlink: missing target",
        ),
        (b"/synthetic/private".as_slice(), "permission denied"),
        (b"/synthetic/other<&".as_slice(), "other <unknown> reason"),
    ]
    .into_iter()
    .map(|(path, reason)| Gap {
        path: bp(path),
        reason: reason.into(),
    })
    .collect();
    inventory.coverage.remaining = Some(DiscoveryRemainder {
        reason: DiscoveryStop::DirectoryBudget,
        known_pending_paths: 123,
        cover: RemainingCover::RequestedScope,
    });
    let before = serde_json::to_vec(&inventory).unwrap();
    let report = html::inventory(&inventory, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("data-scan-limits"));
    assert!(report.contains("data-gap-group=\"links\" data-gap-count=\"2\""));
    assert!(report.contains("data-gap-group=\"access\" data-gap-count=\"1\""));
    assert!(report.contains("data-gap-group=\"other\" data-gap-count=\"1\""));
    assert!(report.contains("123 filesystem paths still pending"));
    assert!(report.contains("not a count of undiscovered worktrees"));
    assert!(report.contains("<details class=\"scan-technical\"><summary>Technical scan details"));
    for gap in &inventory.coverage.gaps {
        assert!(report.contains(&format!("data-copy-path=\"{}\"", gap.path.hex)));
    }
    assert!(report.contains("other &lt;unknown&gt; reason"));
    assert_eq!(serde_json::to_vec(&inventory).unwrap(), before);
}

#[test]
fn reading_guide_explains_inventory_and_embeds_an_offline_diagram() {
    let source = dashboard_fixture();
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("id=\"reading-guide\""));
    assert!(report.contains("This report is an inventory"));
    assert!(report.contains("class=\"guide-diagram\""));
    assert!(report.contains("aria-labelledby=\"guide-flow-title\""));
    assert!(report.contains("<dialog id=\"command-dialog\""));
    assert!(report.contains("aria-controls=\"command-results\""));
    assert!(report.contains("class=\"ui-icon\" aria-hidden=\"true\""));
    let (source, review) = fixture();
    let selected = view::build(&source, &review, Default::default(), "repo").unwrap();
    let report = html::review(&selected, 4 * 1024 * 1024).unwrap();
    assert!(report.contains("This report includes a Git review"));
}

#[test]
fn scan_limits_do_not_mislabel_local_collection_gaps_as_a_stopped_filesystem_walk() {
    let (mut source, _) = fixture();
    source.coverage.remaining = None;
    source.coverage.traversal_complete = false;
    source.coverage.gaps = vec![
        Gap {
            path: bp(b"/synthetic/project"),
            reason: "local inventory is incomplete; see local_inventories gaps".into(),
        },
        Gap {
            path: bp(b"/synthetic/project"),
            reason: "Git reports an unborn or unknown HEAD".into(),
        },
    ];
    let report = html::inventory(&source, 4 * 1024 * 1024).unwrap();
    let limits = report.split("data-scan-limits").nth(1).unwrap();
    assert!(
        report.contains("Recorded scan is incomplete: discovery or local evidence may be missing")
    );
    assert!(!report.contains("Discovery interrupted:"));
    assert!(limits.contains("Discovery or local collection is incomplete"));
    assert!(!limits.contains("filesystem scan stopped"));
    assert!(limits.contains("Git reports an unborn or unknown HEAD"));
    assert!(limits.contains("local inventory is incomplete"));
}
