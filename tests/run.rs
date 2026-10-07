#![allow(clippy::expect_used)]
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use wt_sweep::{
    model::{BytePath, Inventory, ScanScope},
    run::{Artifact, PublicationIo, PublicationPhase, publish, publish_with_io},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-run-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("fixture");
        Self(path)
    }
    fn boundaries(&self) -> Inventory {
        Inventory::new(ScanScope::Roots {
            paths: vec![BytePath::from_path(&self.0.join("protected"))],
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn artifact() -> Artifact<'static> {
    Artifact {
        command: "scan",
        name: "inventory.json",
        json: b"{\"value\":true}\n",
        report: "observed report\n",
        html: None,
        partial: true,
    }
}
struct Failure {
    phase: PublicationPhase,
    prefix: bool,
    wrote: bool,
    errno: i32,
}
impl PublicationIo for Failure {
    fn write(
        &mut self,
        file: &mut fs::File,
        bytes: &[u8],
        phase: PublicationPhase,
    ) -> io::Result<usize> {
        if phase == self.phase {
            if self.prefix && !self.wrote {
                self.wrote = true;
                return file.write(&bytes[..bytes.len().min(3)]);
            }
            return Err(io::Error::from_raw_os_error(self.errno));
        }
        file.write(bytes)
    }
    fn sync(&mut self, file: &fs::File, phase: PublicationPhase) -> io::Result<()> {
        if phase == self.phase {
            Err(io::Error::from_raw_os_error(self.errno))
        } else {
            file.sync_all()
        }
    }
}
#[test]
fn every_publication_phase_failure_retains_real_bytes_and_prior_run() {
    use PublicationPhase::*;
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let prior = publish(&fixture.0, artifact(), &boundaries).expect("prior positive run");
    let prior_json = fs::read(prior.join("inventory.json")).expect("prior json");
    let prior_receipt = fs::read(prior.join("run.json")).expect("prior receipt");
    for phase in [
        JsonWrite,
        JsonSync,
        ReportWrite,
        ReportSync,
        ArtifactsDirectorySync,
        ReceiptWrite,
        ReceiptSync,
        FinalDirectorySync,
        ParentDirectorySync,
    ] {
        let error = publish_with_io(
            &fixture.0,
            artifact(),
            &boundaries,
            &mut Failure {
                phase,
                prefix: false,
                wrote: false,
                errno: 28,
            },
        )
        .expect_err("injected phase must fail");
        assert_eq!(error.operation, phase.as_str());
        assert!(error.message.contains("durability=unknown"));
        let path = error
            .path
            .as_ref()
            .expect("owned run path")
            .to_path()
            .expect("decode");
        assert!(path.is_dir());
        if phase != JsonWrite {
            assert_eq!(
                fs::read(path.join("inventory.json")).expect("json"),
                artifact().json
            );
        }
        if matches!(
            phase,
            ArtifactsDirectorySync
                | ReceiptWrite
                | ReceiptSync
                | FinalDirectorySync
                | ParentDirectorySync
        ) {
            assert_eq!(
                fs::read(path.join("report.txt")).expect("report"),
                artifact().report.as_bytes()
            );
        }
        if matches!(
            phase,
            ReceiptSync | FinalDirectorySync | ParentDirectorySync
        ) {
            let receipt: serde_json::Value =
                serde_json::from_slice(&fs::read(path.join("run.json")).expect("visible receipt"))
                    .expect("valid receipt");
            assert_eq!(receipt["status"], "partial");
            assert_eq!(
                receipt["publication"]["durability"],
                "unknown_from_receipt_alone"
            );
            assert!(error.message.contains("receipt_may_be_visible=true"));
        } else if phase != ReceiptWrite {
            assert!(!path.join("run.json").exists());
        }
        assert_eq!(
            fs::read(prior.join("inventory.json")).expect("prior preserved"),
            prior_json
        );
        assert_eq!(
            fs::read(prior.join("run.json")).expect("prior preserved receipt"),
            prior_receipt
        );
    }
}
#[test]
fn partial_write_then_space_or_interrupt_error_preserves_exact_prefix() {
    use PublicationPhase::*;
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    for (phase, name, bytes) in [
        (JsonWrite, "inventory.json", artifact().json),
        (ReportWrite, "report.txt", artifact().report.as_bytes()),
        (ReceiptWrite, "run.json", b"{\n " as &[u8]),
    ] {
        for errno in [28, 4] {
            let error = publish_with_io(
                &fixture.0,
                artifact(),
                &boundaries,
                &mut Failure {
                    phase,
                    prefix: true,
                    wrote: false,
                    errno,
                },
            )
            .expect_err("real partial write must fail");
            let path = error.path.expect("run path").to_path().expect("decode");
            assert_eq!(
                fs::read(path.join(name)).expect("actual partial artifact"),
                bytes[..3]
            );
            assert_eq!(error.operation, phase.as_str());
        }
    }
}
struct ShortWrites {
    phases: Vec<PublicationPhase>,
    parent_identity: (u64, u64),
}
impl PublicationIo for ShortWrites {
    fn write(
        &mut self,
        file: &mut fs::File,
        bytes: &[u8],
        _: PublicationPhase,
    ) -> io::Result<usize> {
        file.write(&bytes[..bytes.len().min(2)])
    }
    fn sync(&mut self, file: &fs::File, phase: PublicationPhase) -> io::Result<()> {
        self.phases.push(phase);
        if phase == PublicationPhase::ParentDirectorySync {
            use std::os::unix::fs::MetadataExt;
            let metadata = file.metadata()?;
            assert!(metadata.is_dir());
            assert_eq!(
                (metadata.dev(), metadata.ino()),
                self.parent_identity,
                "parent sync must use the retained parent descriptor"
            );
        }
        file.sync_all()
    }
}
#[test]
fn successful_short_writes_sync_parent_and_receipt_explains_its_own_limit() {
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    use std::os::unix::fs::MetadataExt;
    let parent_metadata = fs::metadata(&fixture.0).expect("parent identity");
    let mut io = ShortWrites {
        phases: Vec::new(),
        parent_identity: (parent_metadata.dev(), parent_metadata.ino()),
    };
    let path = publish_with_io(&fixture.0, artifact(), &boundaries, &mut io)
        .expect("short writes handled");
    assert_eq!(
        fs::read(path.join("inventory.json")).expect("full json"),
        artifact().json
    );
    assert_eq!(
        fs::read(path.join("report.txt")).expect("full report"),
        artifact().report.as_bytes()
    );
    assert_eq!(
        io.phases,
        vec![
            PublicationPhase::JsonSync,
            PublicationPhase::ReportSync,
            PublicationPhase::ArtifactsDirectorySync,
            PublicationPhase::ReceiptSync,
            PublicationPhase::FinalDirectorySync,
            PublicationPhase::ParentDirectorySync
        ]
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("run.json")).expect("receipt"))
            .expect("valid receipt");
    assert_eq!(receipt["schema"], "wt-sweep.run/1");
    assert_eq!(
        receipt["publication"]["meaning"],
        "artifacts_synced_before_receipt"
    );
    assert_eq!(
        receipt["publication"]["completion"],
        "requires_successful_publisher_return"
    );
    assert_eq!(
        receipt["artifacts"][0]["sha256"],
        wt_sweep::output::sha256(artifact().json)
    );
}

fn review_artifact() -> Artifact<'static> {
    Artifact {
        command: "review",
        name: "review.json",
        json: b"{\"schema\":\"synthetic-review\"}\n",
        report: "review report\n",
        html: None,
        partial: false,
    }
}
#[test]
fn optional_inventory_is_published_exactly_and_receipt_hashes_all_three_artifacts() {
    use std::os::unix::fs::MetadataExt;
    use wt_sweep::run::publish_with_inventory_io;
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let metadata = fs::metadata(&fixture.0).expect("parent");
    let mut io = ShortWrites {
        phases: Vec::new(),
        parent_identity: (metadata.dev(), metadata.ino()),
    };
    let inventory = b"{\"input\":\"full original\"}\n";
    let path = publish_with_inventory_io(
        &fixture.0,
        review_artifact(),
        &boundaries,
        inventory,
        &mut io,
    )
    .expect("combined publication");
    assert_eq!(
        fs::read(path.join("inventory.json")).expect("full input"),
        inventory
    );
    assert_eq!(
        fs::read(path.join("review.json")).expect("review"),
        review_artifact().json
    );
    assert_eq!(
        io.phases,
        vec![
            PublicationPhase::InventorySync,
            PublicationPhase::JsonSync,
            PublicationPhase::ReportSync,
            PublicationPhase::ArtifactsDirectorySync,
            PublicationPhase::ReceiptSync,
            PublicationPhase::FinalDirectorySync,
            PublicationPhase::ParentDirectorySync
        ]
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("run.json")).expect("receipt"))
            .expect("valid receipt");
    let artifacts = receipt["artifacts"].as_array().expect("artifacts");
    assert_eq!(artifacts.len(), 3);
    for (name, bytes) in [
        ("inventory.json", inventory.as_slice()),
        ("review.json", review_artifact().json),
        ("report.txt", review_artifact().report.as_bytes()),
    ] {
        let artifact = artifacts
            .iter()
            .find(|artifact| artifact["name"] == name)
            .expect("bound artifact");
        assert_eq!(artifact["sha256"], wt_sweep::output::sha256(bytes));
        assert_eq!(artifact["bytes"], bytes.len());
    }
}
#[test]
fn inventory_write_and_sync_failure_preserve_real_prefix_and_prior_run() {
    use wt_sweep::run::{publish_with_inventory, publish_with_inventory_io};
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let inventory = b"{\"input\":true}\n";
    let prior = publish_with_inventory(&fixture.0, review_artifact(), &boundaries, inventory)
        .expect("prior combined run");
    let prior_receipt = fs::read(prior.join("run.json")).expect("prior receipt");
    for phase in [
        PublicationPhase::InventoryWrite,
        PublicationPhase::InventorySync,
    ] {
        for errno in [28, 4] {
            let error = publish_with_inventory_io(
                &fixture.0,
                review_artifact(),
                &boundaries,
                inventory,
                &mut Failure {
                    phase,
                    prefix: true,
                    wrote: false,
                    errno,
                },
            )
            .expect_err("inventory phase must fail");
            let path = error
                .path
                .as_ref()
                .expect("run path")
                .to_path()
                .expect("decode");
            let expected: &[u8] = if phase == PublicationPhase::InventoryWrite {
                &inventory[..3]
            } else {
                inventory
            };
            assert_eq!(
                fs::read(path.join("inventory.json")).expect("actual input bytes"),
                expected
            );
            assert!(!path.join("review.json").exists());
            assert!(!path.join("run.json").exists());
            assert_eq!(error.operation, phase.as_str());
            assert!(error.message.contains("receipt_may_be_visible=false"));
            assert!(error.message.contains("durability=unknown"));
            assert_eq!(
                fs::read(prior.join("inventory.json")).expect("prior input"),
                inventory
            );
            assert_eq!(
                fs::read(prior.join("run.json")).expect("prior receipt conserved"),
                prior_receipt
            );
        }
    }
}
#[test]
fn combined_publication_rejects_collisions_and_oversize_before_creating_any_run() {
    use wt_sweep::run::publish_with_inventory;
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    assert!(
        publish_with_inventory(&fixture.0, artifact(), &boundaries, b"input").is_err(),
        "inventory.json cannot collide with primary artifact"
    );
    assert_eq!(fs::read_dir(&fixture.0).expect("directory").count(), 0);
    let over = vec![b'x'; 32 * 1024 * 1024 + 1];
    assert!(publish_with_inventory(&fixture.0, review_artifact(), &boundaries, &over).is_err());
    assert!(
        publish_with_inventory(
            &fixture.0,
            Artifact {
                json: &over,
                ..review_artifact()
            },
            &boundaries,
            b"input"
        )
        .is_err()
    );
    let report = String::from_utf8(over).expect("synthetic report");
    assert!(
        publish_with_inventory(
            &fixture.0,
            Artifact {
                report: &report,
                ..review_artifact()
            },
            &boundaries,
            b"input"
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&fixture.0).expect("directory").count(), 0);
}

#[test]
fn html_publication_receipt_binds_bytes_after_html_sync() {
    use std::os::unix::fs::MetadataExt;
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let metadata = fs::metadata(&fixture.0).expect("parent");
    let mut io = ShortWrites {
        phases: Vec::new(),
        parent_identity: (metadata.dev(), metadata.ino()),
    };
    let html = "<!doctype html><title>Observed worktrees</title>\n";
    let path = publish_with_io(
        &fixture.0,
        Artifact {
            html: Some(html),
            ..artifact()
        },
        &boundaries,
        &mut io,
    )
    .expect("HTML run");
    assert_eq!(
        fs::read(path.join("report.html")).expect("HTML bytes"),
        html.as_bytes()
    );
    let receipt: serde_json::Value =
        serde_json::from_slice(&fs::read(path.join("run.json")).expect("receipt")).expect("JSON");
    let entry = receipt["artifacts"]
        .as_array()
        .expect("artifacts")
        .iter()
        .find(|entry| entry["name"] == "report.html")
        .expect("HTML receipt binding");
    assert_eq!(entry["sha256"], wt_sweep::output::sha256(html.as_bytes()));
    assert_eq!(entry["bytes"], html.len());
    let html_sync = io
        .phases
        .iter()
        .position(|phase| *phase == PublicationPhase::HtmlSync)
        .expect("HTML synced");
    let receipt_sync = io
        .phases
        .iter()
        .position(|phase| *phase == PublicationPhase::ReceiptSync)
        .expect("receipt synced");
    assert!(html_sync < receipt_sync);
}

#[test]
fn html_write_or_sync_failure_never_publishes_a_receipt() {
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let html = "<!doctype html>synthetic partial HTML";
    for phase in [PublicationPhase::HtmlWrite, PublicationPhase::HtmlSync] {
        for errno in [28, 4] {
            let error = publish_with_io(
                &fixture.0,
                Artifact {
                    html: Some(html),
                    ..artifact()
                },
                &boundaries,
                &mut Failure {
                    phase,
                    prefix: true,
                    wrote: false,
                    errno,
                },
            )
            .expect_err("HTML publication failure");
            let path = error.path.expect("owned run path").to_path().expect("path");
            assert_eq!(error.operation, phase.as_str());
            assert!(!path.join("run.json").exists());
            let expected = if phase == PublicationPhase::HtmlWrite {
                &html.as_bytes()[..3]
            } else {
                html.as_bytes()
            };
            assert_eq!(
                fs::read(path.join("report.html")).expect("retained prefix"),
                expected
            );
        }
    }
}

#[test]
fn oversized_html_refuses_before_creating_a_run() {
    let fixture = Fixture::new();
    let boundaries = fixture.boundaries();
    let html = "x".repeat(32 * 1024 * 1024 + 1);
    assert!(
        publish(
            &fixture.0,
            Artifact {
                html: Some(&html),
                ..artifact()
            },
            &boundaries
        )
        .is_err()
    );
    assert_eq!(fs::read_dir(&fixture.0).expect("directory").count(), 0);
}
