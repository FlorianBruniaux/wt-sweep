#![allow(clippy::expect_used)]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::forge::ingest;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-forge-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("owned fixture");
        Self(path)
    }
    fn capture(&self) -> (PathBuf, PathBuf) {
        let raw = self.0.join("raw.json");
        let receipt = self.0.join("receipt.json");
        let status = Command::new("python3")
            .args([
                "-B",
                "experiments/forge-capture/capture_fixture.py",
                "tests/data/forge/gitlab-pages.json",
            ])
            .arg(&raw)
            .arg(&receipt)
            .status()
            .expect("fixture executor");
        assert!(status.success());
        (raw, receipt)
    }
    fn write(&self, name: &str, value: &serde_json::Value) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, serde_json::to_vec(value).expect("json")).expect("write fixture");
        path
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn report(raw: &Path, receipt: Option<&Path>) -> serde_json::Value {
    serde_json::to_value(ingest(raw, receipt).expect("ingest")).expect("serialize")
}

#[test]
fn executor_receipt_imports_positive_facts_without_proving_absence() {
    let fixture = Fixture::new();
    let (raw, receipt) = fixture.capture();
    let value = report(&raw, Some(&receipt));
    assert_eq!(value["schema"], "wt-sweep.forge/1");
    assert_eq!(value["requests"].as_array().expect("requests").len(), 2);
    assert_eq!(value["coverage"]["pagination_complete"], true);
    assert_eq!(value["coverage"]["absence_proven"], false);
    assert_eq!(value["coverage"]["temporal_consistency"], "not_guaranteed");
}

#[test]
fn replaced_raw_bytes_are_refused() {
    let fixture = Fixture::new();
    let (raw, receipt) = fixture.capture();
    fs::write(&raw, b"[]").expect("replace owned export");
    assert!(ingest(&raw, Some(&receipt)).is_err());
}

#[test]
fn receipt_status_truncation_pages_and_cursors_cannot_claim_completion() {
    for mutation in [
        "missing_status",
        "failure",
        "truncated",
        "missing_page",
        "repeat_cursor",
        "wrong_scope",
        "bad_interval",
    ] {
        let fixture = Fixture::new();
        let (raw, receipt) = fixture.capture();
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&receipt).expect("receipt")).expect("json");
        match mutation {
            "missing_status" => {
                value.as_object_mut().expect("object").remove("exit_code");
            }
            "failure" => value["exit_code"] = 1.into(),
            "truncated" => value["truncated"] = true.into(),
            "missing_page" => {
                value["pages"].as_array_mut().expect("pages").pop();
            }
            "repeat_cursor" => {
                value["pages"][0]["cursor"] = "same".into();
                value["pages"][1]["cursor"] = "same".into();
            }
            "wrong_scope" => value["host"] = "other.example.invalid".into(),
            "bad_interval" => value["started_unix_ms"] = u64::MAX.into(),
            _ => unreachable!(),
        }
        fixture.write("receipt.json", &value);
        let report = report(&raw, Some(&receipt));
        assert_eq!(
            report["requests"].as_array().expect("requests").len(),
            2,
            "{mutation}"
        );
        assert_eq!(
            report["coverage"]["pagination_complete"], false,
            "{mutation}"
        );
        assert_eq!(report["coverage"]["absence_proven"], false, "{mutation}");
        assert!(
            !report["coverage"]["gaps"]
                .as_array()
                .expect("gaps")
                .is_empty(),
            "{mutation}"
        );
    }
}

#[test]
fn unknown_receipt_fields_and_nonregular_files_are_refused() {
    let fixture = Fixture::new();
    let (raw, receipt) = fixture.capture();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&receipt).expect("receipt")).expect("json");
    value["approve_deletion"] = true.into();
    fixture.write("receipt.json", &value);
    assert!(ingest(&raw, Some(&receipt)).is_err());
    let link = fixture.0.join("alias.json");
    std::os::unix::fs::symlink(&raw, &link).expect("link");
    assert!(ingest(&link, None).is_err());
    let fifo = fixture.0.join("fifo");
    assert!(
        Command::new("python3")
            .args(["-B", "-c", "import os,sys; os.mkfifo(sys.argv[1])"])
            .arg(&fifo)
            .status()
            .expect("fifo")
            .success()
    );
    assert!(ingest(&fifo, None).is_err());
}

#[test]
fn missing_receipt_preserves_forks_and_colliding_project_iids() {
    let fixture = Fixture::new();
    let raw = fixture.write("raw.json", &serde_json::json!([
        {"iid": 9,"project_id":17,"source_project_id":19,"target_project_id":17,"source_branch":"topic","state":"opened","web_url":"https://forge.example.invalid/a/-/merge_requests/9"},
        {"iid": 9,"project_id":18,"source_project_id":18,"target_project_id":18,"source_branch":"topic","state":"closed","web_url":"https://forge.example.invalid/b/-/merge_requests/9"}
    ]));
    let report = report(&raw, None);
    let requests = report["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), 2);
    assert_ne!(requests[0]["project"], requests[1]["project"]);
    assert_eq!(requests[0]["source_project"], "19");
    assert_eq!(report["sources"][0]["receipt_verified"], false);
    assert_eq!(report["coverage"]["absence_proven"], false);
}

#[test]
fn duplicate_and_contradictory_versions_are_retained_as_gaps() {
    let fixture = Fixture::new();
    let raw = fixture.write(
        "raw.json",
        &serde_json::json!([
            {"iid":1,"project_id":17,"state":"opened"},
            {"iid":1,"project_id":17,"state":"closed"}
        ]),
    );
    let report = report(&raw, None);
    assert_eq!(report["requests"].as_array().expect("requests").len(), 2);
    assert!(
        report["coverage"]["gaps"]
            .to_string()
            .contains("contradict")
    );
}

#[test]
fn github_rest_graphql_and_cli_formats_preserve_positive_facts() {
    let fixture = Fixture::new();
    let node = serde_json::json!({"number":42,"state":"OPEN","url":"https://github.example.invalid/target/repo/pull/42", "headRefName":"topic","baseRefName":"main", "headRefOid":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "headRepository":{"nameWithOwner":"fork/repo"}, "baseRepository":{"nameWithOwner":"target/repo"}});
    for value in [
        serde_json::json!([node.clone()]),
        serde_json::json!({"data":{"repository":{"nameWithOwner":"target/repo", "pullRequests":{"nodes":[node],"pageInfo":{"hasNextPage":false,"endCursor":"last"}}}}}),
        serde_json::json!([{"number":42,"state":"open","html_url":"https://github.example.invalid/target/repo/pull/42", "head":{"ref":"topic","sha":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","repo":{"full_name":"fork/repo"}}, "base":{"ref":"main","repo":{"full_name":"target/repo"}}}]),
    ] {
        let raw = fixture.write("raw.json", &value);
        let report = report(&raw, None);
        assert_eq!(report["requests"][0]["iid"], 42);
        assert_eq!(report["requests"][0]["source_project"], "fork/repo");
        assert_eq!(report["requests"][0]["target_project"], "target/repo");
        assert_eq!(report["requests"][0]["state"], "opened");
        assert_eq!(report["coverage"]["absence_proven"], false);
    }
}

#[test]
fn saturated_pages_and_valid_json_truncation_are_not_complete() {
    let fixture = Fixture::new();
    for count in [100, 2000] {
        let rows: Vec<_> = (1..=count)
            .map(|id| serde_json::json!({"iid":id,"project_id":17,"state":"opened"}))
            .collect();
        let raw = fixture.write("raw.json", &serde_json::json!(rows));
        let report = report(&raw, None);
        assert_eq!(
            report["requests"].as_array().expect("requests").len(),
            count
        );
        assert!(report["coverage"]["gaps"].to_string().contains("saturat"));
        assert_eq!(report["coverage"]["pagination_complete"], false);
    }
}

#[test]
fn page_traversal_is_not_a_temporally_consistent_snapshot() {
    let fixture = Fixture::new();
    let input = fixture.0.join("moving.json");
    fs::write(&input, concat!(
        "[{\"iid\":1,\"project_id\":17,\"state\":\"opened\",\"web_url\":\"https://forge.example.invalid/team/project/-/merge_requests/1\"}]\n",
        "[{\"iid\":1,\"project_id\":17,\"state\":\"closed\",\"web_url\":\"https://forge.example.invalid/team/project/-/merge_requests/1\"}]\n"
    )).expect("moving fixture");
    let raw = fixture.0.join("raw.json");
    let receipt = fixture.0.join("receipt.json");
    assert!(
        Command::new("python3")
            .args(["-B", "experiments/forge-capture/capture_fixture.py"])
            .arg(&input)
            .arg(&raw)
            .arg(&receipt)
            .status()
            .expect("executor")
            .success()
    );
    let report = report(&raw, Some(&receipt));
    assert_eq!(report["requests"].as_array().expect("requests").len(), 2);
    assert_eq!(report["coverage"]["pagination_complete"], true);
    assert_eq!(report["coverage"]["temporal_consistency"], "not_guaranteed");
    assert_eq!(report["coverage"]["absence_proven"], false);
    assert!(
        report["coverage"]["gaps"]
            .to_string()
            .contains("contradict")
    );
}

#[test]
#[ignore = "requires explicit authorized historical raw path; no private data in repository"]
fn historical_lead_app_positive_import() {
    let path = std::env::var_os("WT_SWEEP_HISTORICAL_RAW").expect("explicit historical raw path");
    let report = report(Path::new(&path), None);
    assert_eq!(report["requests"].as_array().expect("requests").len(), 430);
    assert_eq!(report["coverage"]["absence_proven"], false);
    assert_eq!(report["sources"][0]["receipt_verified"], false);
}

#[test]
fn bounded_reads_reject_oversized_raw_and_receipt_without_modifying_sources() {
    let fixture = Fixture::new();
    let (raw, receipt) = fixture.capture();
    let before_raw = fs::read(&raw).expect("raw");
    let before_receipt = fs::read(&receipt).expect("receipt");
    assert!(ingest(&raw, Some(&receipt)).is_ok());
    assert_eq!(fs::read(&raw).expect("raw"), before_raw);
    assert_eq!(fs::read(&receipt).expect("receipt"), before_receipt);
    let large = fixture.0.join("large.json");
    fs::File::create(&large)
        .expect("owned sparse file")
        .set_len(16 * 1024 * 1024 + 1)
        .expect("size");
    assert!(ingest(&large, None).is_err());
    fs::File::create(&large)
        .expect("owned sparse receipt")
        .set_len(1024 * 1024 + 1)
        .expect("size");
    assert!(ingest(&raw, Some(&large)).is_err());
}

#[test]
fn envelopes_preserve_partial_graphql_data_and_reject_short_oids() {
    let fixture = Fixture::new();
    let raw = fixture.write(
        "raw.json",
        &serde_json::json!({"pages":[
            {"items":[{"iid":1,"state":"opened","project_id":17,"sha":"abcdef"}]},
            [{"iid":2,"state":"closed","project_id":17}]
        ]}),
    );
    let value = report(&raw, None);
    assert_eq!(value["requests"].as_array().expect("requests").len(), 2);
    assert!(value["requests"][0]["head_oid"].is_null());
    assert!(value["coverage"]["gaps"].to_string().contains("shortened"));
    let raw = fixture.write("raw.json", &serde_json::json!({"errors":[{"message":"page partial"}], "data":{"search":{"edges":[{"node":{"number":5,"state":"OPEN","url":"https://github.example.invalid/a/b/pull/5"}}],"pageInfo":{"hasNextPage":true,"endCursor":"next"}}}}));
    let value = report(&raw, None);
    assert_eq!(value["requests"][0]["iid"], 5);
    assert!(
        value["coverage"]["gaps"]
            .to_string()
            .contains("forge returned errors")
    );
    assert_eq!(value["coverage"]["absence_proven"], false);
}

#[test]
fn replay_paths_are_canonical_and_independent_of_ingestion_cwd() {
    let path = Path::new("tests/data/forge/gitlab-pages.json");
    let export = ingest(path, None).expect("relative input");
    assert_eq!(
        export.sources[0].raw_path.to_path().expect("path"),
        fs::canonicalize(path).expect("canonical input")
    );
}

#[test]
fn contradictory_alternative_fields_are_unknown_and_cannot_match_a_worktree() {
    let fixture = Fixture::new();
    let a = "a".repeat(40);
    let b = "b".repeat(40);
    let cases = [
        (
            serde_json::json!({"iid":1,"project_id":18,"target_project_id":17,"source_project_id":17,"source_branch":"topic","state":"opened","sha":a,"diff_refs":{"head_sha":b},"web_url":"https://forge.example.invalid/team/project/-/merge_requests/1"}),
            vec!["project", "target_project", "head_oid"],
        ),
        (
            serde_json::json!({"number":1,"state":"OPEN","head":{"sha":a,"ref":"topic","repo":{"full_name":"fork/one"}},"headRefOid":b,"headRefName":"other","headRepository":{"nameWithOwner":"fork/two"},"url":"https://forge.example.invalid/team/project/pull/1"}),
            vec!["source_project", "source_branch", "head_oid"],
        ),
        (
            serde_json::json!({"number":1,"state":"OPEN","base":{"ref":"main","repo":{"full_name":"team/one"}},"baseRefName":"other","baseRepository":{"nameWithOwner":"team/two"},"url":"https://forge.example.invalid/team/project/pull/1","headRefOid":a}),
            vec!["project", "target_project", "target_branch"],
        ),
        (
            serde_json::json!({"number":1,"state":"CLOSED","headRefOid":a,"merge_commit_sha":a,"mergeCommit":{"oid":b},"updated_at":"2026-01-01T00:00:00Z","updatedAt":"2026-01-02T00:00:00Z","url":"https://forge.example.invalid/team/project/pull/1"}),
            vec!["merge_oid", "updated_at"],
        ),
        (
            serde_json::json!({"number":1,"state":"OPEN","headRefOid":a,"html_url":"https://forge.example.invalid/team/one/pull/1","url":"https://forge.example.invalid/team/two/pull/1"}),
            vec!["url"],
        ),
        (
            serde_json::json!({"number":1,"state":"OPEN","headRefOid":a,"html_url":"https://github.com/team/one/pull/1","url":"https://api.github.com/repos/team/two/pulls/1"}),
            vec!["url"],
        ),
    ];
    for (row, unknown_fields) in cases {
        let raw = fixture.write("raw.json", &serde_json::json!([row]));
        let value = report(&raw, None);
        assert_eq!(value["requests"].as_array().expect("requests").len(), 1);
        for field in unknown_fields {
            assert!(
                value["requests"][0][field].is_null(),
                "contradictory {field}: {value}"
            );
        }
        assert!(
            value["requests"][0]["host"].is_null(),
            "contradictory request still associable"
        );
        assert!(
            value["coverage"]["gaps"]
                .to_string()
                .contains("contradictory alternatives")
        );
    }
}

#[test]
fn missing_graphql_edge_node_preserves_raw_cardinality_and_blocks_completion() {
    let fixture = Fixture::new();
    let raw = fixture.write("raw.json", &serde_json::json!({"data":{"repository":{"pullRequests":{
        "edges":[{"node":{"number":1,"state":"OPEN","url":"https://forge.example.invalid/team/project/pull/1"}},{}],
        "pageInfo":{"hasNextPage":false,"endCursor":"last"}
    }}}}));
    // A maliciously reduced receipt is intentionally constructed as an attack,
    // not represented as an executor-generated proof.
    for count in [1, 2] {
        let bytes = fs::read(&raw).expect("raw");
        let receipt = fixture.write(
            "receipt.json",
            &serde_json::json!({
                "schema":"wt-sweep.forge-receipt/1","argv":["synthetic-adversarial-receipt"],
                "provider":"github","host":"forge.example.invalid","project":"team/project",
                "started_unix_ms":1,"finished_unix_ms":2,"exit_code":0,
                "raw_sha256":wt_sweep::output::sha256(&bytes),"raw_bytes":bytes.len(),
                "truncated":false,"pagination_complete":true,
                "pages":[{"number":1,"items":count,"cursor":null,"next_cursor":null}]
            }),
        );
        let value = report(&raw, Some(&receipt));
        assert_eq!(value["requests"].as_array().expect("requests").len(), 1);
        assert_eq!(
            value["sources"][0]["receipt_verified"], false,
            "count={count}"
        );
        assert_eq!(
            value["coverage"]["pagination_complete"], false,
            "count={count}"
        );
        assert!(value["coverage"]["gaps"].to_string().contains("GraphQL"));
        if count == 1 {
            assert!(
                value["coverage"]["gaps"]
                    .to_string()
                    .contains("count mismatch")
            );
        }
    }
}

#[test]
fn agreeing_alternatives_and_rest_api_url_keep_positive_identity() {
    let fixture = Fixture::new();
    let oid = "a".repeat(40);
    let raw = fixture.write("raw.json", &serde_json::json!([
        {"iid":1,"project_id":17,"target_project_id":17,"source_project_id":17,
         "state":"opened","sha":oid,"diff_refs":{"head_sha":oid.to_uppercase()},
         "web_url":"https://forge.example.invalid/team/project/-/merge_requests/1"},
        {"number":1,"state":"OPEN","head":{"sha":oid,"ref":"topic","repo":{"full_name":"fork/project"}},
         "headRefOid":oid,"headRefName":"topic","headRepository":{"nameWithOwner":"fork/project"},
         "base":{"ref":"main","repo":{"full_name":"team/project"}},"baseRefName":"main","baseRepository":{"nameWithOwner":"team/project"},
         "html_url":"https://github.com/team/project/pull/1","url":"https://api.github.com/repos/team/project/pulls/1"}
    ]));
    let value = report(&raw, None);
    for request in value["requests"].as_array().expect("requests") {
        assert!(request["host"].is_string());
        assert_eq!(request["head_oid"], oid);
        assert!(request["target_project"].is_string());
    }
    assert!(
        !value["coverage"]["gaps"]
            .to_string()
            .contains("contradictory alternatives")
    );
}
