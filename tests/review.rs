// Panics are assertion failures in these isolated tests, never product handling.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use serde_json::json;
use wt_sweep::model::{BytePath, Inventory, Observations, Review, ReviewItem, ScanScope};

fn observations() -> Observations {
    let path = BytePath::from_bytes(b"/private/tmp/synthetic-pure-judgement-input");
    let inventory = Inventory::new(ScanScope::Repo { path: path.clone() });
    Observations {
        inventory,
        items: vec![ReviewItem {
            path,
            branch: None,
            head: None,
            subject: None,
            subject_truncated: false,
            classification: "to_triage".into(),
            observations: Vec::new(),
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
