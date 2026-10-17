use super::*;
use std::{
    cell::Cell,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::fs::canonicalize(std::env::temp_dir())?.join(format!(
            "wt-sweep-open-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root)?;
        Ok(Self(root))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn open_implies_html_and_happens_only_after_successful_publication()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let args = vec![
        OsString::from("--open"),
        OsString::from("--run-dir"),
        fixture.0.clone().into_os_string(),
    ];
    let opts = options(&args, PLAN_OPTIONS, &[])?;
    assert!(opts.contains_key("--html"));
    let calls = Cell::new(0);
    let opener = |path: &Path| {
        let directory = path.parent().ok_or("missing report directory")?;
        let receipt: serde_json::Value = serde_json::from_slice(
            &std::fs::read(directory.join("run.json")).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        assert!(
            receipt["artifacts"]
                .as_array()
                .is_some_and(|a| a.iter().any(|a| a["name"] == "report.html"))
        );
        assert_eq!(
            std::fs::read_to_string(path).map_err(|e| e.to_string())?,
            "<html>fixture</html>"
        );
        calls.set(calls.get() + 1);
        // A desktop-app failure must not invalidate the saved evidence.
        Err("fixture opener unavailable".into())
    };
    let inventory = Inventory::new(ScanScope::Roots { paths: vec![] });
    let artifact = || RunInfo {
        command: "scan",
        name: "inventory.json",
        partial: false,
        detail: None,
        inventory_json: None,
        html: Some("<html>fixture</html>"),
        comparison: None,
    };
    deliver_with_opener(
        &opts,
        b"{}\n".to_vec(),
        "Fixture\n".into(),
        &inventory,
        artifact(),
        &opener,
    )?;
    assert_eq!(calls.get(), 1);
    let protected = Inventory::new(ScanScope::Roots {
        paths: vec![BytePath::from_path(&fixture.0)],
    });
    assert!(
        deliver_with_opener(
            &opts,
            b"{}\n".to_vec(),
            "Fixture\n".into(),
            &protected,
            artifact(),
            &opener
        )
        .is_err()
    );
    assert_eq!(
        calls.get(),
        1,
        "failed publication must never open a report"
    );
    Ok(())
}

#[test]
fn html_without_open_never_launches_an_application() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = Fixture::new()?;
    let opts = options(
        &[
            "--html".into(),
            "--run-dir".into(),
            fixture.0.clone().into_os_string(),
        ],
        PLAN_OPTIONS,
        &[],
    )?;
    let calls = Cell::new(0);
    let opener = |_: &Path| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    deliver_with_opener(
        &opts,
        b"{}\n".to_vec(),
        "Fixture\n".into(),
        &Inventory::new(ScanScope::Roots { paths: vec![] }),
        RunInfo {
            command: "scan",
            name: "inventory.json",
            partial: false,
            detail: None,
            inventory_json: None,
            html: Some("<html>fixture</html>"),
            comparison: None,
        },
        &opener,
    )?;
    assert_eq!(calls.get(), 0);
    Ok(())
}
