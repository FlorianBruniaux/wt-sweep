#![allow(clippy::expect_used)]
use wt_sweep::model::BytePath;
use wt_sweep::output::{canonical_digest, sha256};

#[test]
fn canonical_digest_binds_content_and_ignores_its_own_field() {
    assert_eq!(
        sha256(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    let first: serde_json::Value =
        serde_json::from_str(r#"{"b":2,"a":{"y":2,"x":1},"digest_sha256":"old"}"#).expect("json");
    let same: serde_json::Value =
        serde_json::from_str(r#"{"digest_sha256":"new","a":{"x":1,"y":2},"b":2}"#).expect("json");
    let changed: serde_json::Value =
        serde_json::from_str(r#"{"a":{"x":1,"y":3},"b":2}"#).expect("json");
    assert_eq!(
        canonical_digest(&first).expect("digest"),
        canonical_digest(&same).expect("digest")
    );
    assert_ne!(
        canonical_digest(&first).expect("digest"),
        canonical_digest(&changed).expect("digest")
    );
}

#[test]
fn path_bytes_survive_json_and_do_not_inject_terminal_controls() {
    let bytes = b"/tmp/a b\n\xff\\c\x1b";
    let path = BytePath::from_bytes(bytes);
    let json = serde_json::to_string(&path).expect("serialize");
    assert_eq!(json, r#"{"hex":"2f746d702f6120620aff5c631b"}"#);
    let parsed: BytePath = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.bytes().expect("decode"), bytes);
    assert_eq!(parsed.display(), r"/tmp/a b\n\xff\\c\x1b");
}

#[test]
fn malformed_path_and_unknown_identity_fields_are_refused() {
    for hex in ["f", "zz", "00", "FF", "é"] {
        assert!(BytePath { hex: hex.into() }.to_path().is_err(), "{hex}");
    }
    assert!(serde_json::from_str::<BytePath>(r#"{"hex":"61","action":"delete"}"#).is_err());
}
