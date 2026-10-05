#![allow(clippy::expect_used)]
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};
use wt_sweep::{
    model::{BytePath, Family, Identity, Inventory, ScanScope},
    output::write_new,
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let p = PathBuf::from(format!(
            "/private/tmp/wt-sweep-output-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir(&p).expect("owned fixture");
        Self(p)
    }
    fn inventory(&self) -> Inventory {
        let repo = self.0.join("repo");
        fs::create_dir(&repo).expect("repo");
        let common = repo.join(".git");
        fs::create_dir(&common).expect("admin");
        let mut i = Inventory::new(ScanScope::Repo {
            path: BytePath::from_path(&repo),
        });
        i.families.push(Family {
            common_dir: BytePath::from_path(&common),
            identity: Identity {
                device: 0,
                inode: 0,
            },
            worktrees: Vec::new(),
        });
        i
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn output_creates_exact_bytes_and_never_replaces_existing_file() {
    let f = Fixture::new();
    let i = f.inventory();
    let out = f.0.join("inventory.json");
    write_new(&out, b"{\"observed\":true}", &i).expect("outside output");
    assert_eq!(fs::read(&out).expect("output"), b"{\"observed\":true}");
    assert!(write_new(&out, b"changed", &i).is_err());
    assert_eq!(fs::read(&out).expect("preserved"), b"{\"observed\":true}");
}

#[test]
fn output_refuses_target_administration_and_symlink_ancestor() {
    let f = Fixture::new();
    let i = f.inventory();
    for relative in ["repo/report.json", "repo/.git/report.json"] {
        let out = f.0.join(relative);
        assert!(write_new(&out, b"bad", &i).is_err());
        assert!(!out.exists());
    }
    std::os::unix::fs::symlink(f.0.join("repo"), f.0.join("alias")).expect("symlink");
    assert!(write_new(&f.0.join("alias/report.json"), b"bad", &i).is_err());
    assert!(!f.0.join("repo/report.json").exists());
}

#[test]
fn global_partial_inventory_does_not_authorize_output_inside_an_unobserved_marker() {
    let fixture = Fixture::new();
    let mut inventory = fixture.inventory();
    inventory.scope = ScanScope::AllMounted;
    inventory.families.clear();
    inventory.coverage.traversal_complete = false;
    let forbidden = fixture.0.join("repo/report.json");
    assert!(write_new(&forbidden, b"must not appear", &inventory).is_err());
    assert!(!forbidden.exists());
    let bare = fixture.0.join("unknown-administration");
    fs::create_dir(&bare).expect("admin");
    fs::write(bare.join("HEAD"), b"ref: refs/heads/main\n").expect("head");
    fs::create_dir(bare.join("objects")).expect("objects");
    assert!(write_new(&bare.join("report.json"), b"forbidden", &inventory).is_err());
    let allowed = fixture.0.join("outside-report.json");
    write_new(&allowed, b"explicit report", &inventory).expect("outside marker");
    assert_eq!(fs::read(allowed).expect("report"), b"explicit report");
}
