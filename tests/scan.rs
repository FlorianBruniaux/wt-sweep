#![cfg(unix)]
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use wt_sweep::git::{ControlledGit, GitRead, scan_family};
use wt_sweep::model::BytePath;
use wt_sweep::model::DirState;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/private/tmp/wt-sweep-rust-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap_or_else(|e| panic!("fixture: {e}"));
        Self(path)
    }
    fn git(&self, repo: &Path, args: &[&std::ffi::OsStr]) -> Vec<u8> {
        let output = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("git: {e}"));
        assert!(output.status.success(), "git: {:?}", output.stderr);
        output.stdout
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn scan_has_exact_registered_set_with_byte_paths_and_missing_entry() {
    let fixture = Fixture::new();
    let main = fixture.0.join("main");
    fs::create_dir(&main).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(&main, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
    fixture.git(
        &main,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let linked = fixture.0.join("linked \npath");
    let detached = fixture.0.join("detached");
    let missing = fixture.0.join("missing");
    fixture.git(
        &main,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    fixture.git(
        &main,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "--detach".as_ref(),
            detached.as_os_str(),
        ],
    );
    fixture.git(
        &main,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "missing".as_ref(),
            missing.as_os_str(),
        ],
    );
    fs::remove_dir_all(&missing).unwrap_or_else(|e| panic!("remove owned fixture: {e}"));
    let inventory = scan_family(&linked).unwrap_or_else(|e| panic!("scan failed: {e}"));
    assert_eq!(inventory.families.len(), 1);
    let family = &inventory.families[0];
    assert_eq!(
        family
            .common_dir
            .to_path()
            .unwrap_or_else(|e| panic!("path: {e}")),
        main.join(".git")
    );
    let actual: std::collections::BTreeSet<_> = family
        .worktrees
        .iter()
        .map(|w| w.path.to_path().unwrap_or_else(|e| panic!("path: {e}")))
        .collect();
    assert_eq!(
        actual,
        [
            main.clone(),
            linked.clone(),
            detached.clone(),
            missing.clone()
        ]
        .into_iter()
        .collect()
    );
    for tree in &family.worktrees {
        assert_eq!(tree.head.as_ref().map(String::len), Some(40));
        let path = tree.path.to_path().unwrap_or_else(|e| panic!("path: {e}"));
        assert_eq!(tree.detached, path == detached);
        assert_eq!(matches!(tree.path_state, DirState::Absent), path == missing);
        assert_eq!(tree.identity.is_some(), path != missing);
    }
    let repeated = scan_family(&main).unwrap_or_else(|e| panic!("scan: {e}"));
    assert_eq!(repeated.families[0].identity, family.identity);
}

#[test]
fn non_repository_is_an_error_not_an_empty_inventory() {
    let fixture = Fixture::new();
    assert!(scan_family(&fixture.0).is_err());
}

#[test]
fn separate_gitdir_primary_uses_proven_worktree_path_and_preserves_linked_paths() {
    let fixture = Fixture::new();
    let main = fixture.0.join("primary \nworktree");
    let admin = fixture.0.join("external \nadmin");
    fs::create_dir(&main).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(
        &main,
        &[
            "init".as_ref(),
            "--separate-git-dir".as_ref(),
            admin.as_os_str(),
            "-b".as_ref(),
            "main".as_ref(),
        ],
    );
    fixture.git(
        &main,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let linked = fixture.0.join("linked");
    fixture.git(
        &main,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    let inventory = scan_family(&main).unwrap_or_else(|e| panic!("scan: {e}"));
    let family = &inventory.families[0];
    assert_eq!(family.common_dir, BytePath::from_path(&admin));
    assert_eq!(
        family
            .worktrees
            .iter()
            .map(|tree| tree.path.clone())
            .collect::<std::collections::BTreeSet<_>>(),
        [BytePath::from_path(&main), BytePath::from_path(&linked)]
            .into_iter()
            .collect()
    );
    assert!(
        family
            .worktrees
            .iter()
            .all(|tree| tree.identity.is_some() && matches!(tree.path_state, DirState::Present))
    );
    let repeated = scan_family(&main).unwrap_or_else(|e| panic!("rescan: {e}"));
    assert_eq!(
        repeated.families[0].worktrees[0].path,
        BytePath::from_path(&main)
    );
    let error = scan_family(&linked)
        .err()
        .unwrap_or_else(|| panic!("linked tree was invented as primary"));
    assert_eq!(error.code, "primary_worktree_unknown");
}

#[test]
fn separate_admin_without_primary_backlink_never_invents_worktree_path() {
    let fixture = Fixture::new();
    let main = fixture.0.join("main");
    let admin = fixture.0.join("admin");
    fs::create_dir(&main).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(
        &main,
        &[
            "init".as_ref(),
            "--separate-git-dir".as_ref(),
            admin.as_os_str(),
            "-b".as_ref(),
            "main".as_ref(),
        ],
    );
    fixture.git(
        &main,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let error = scan_family(&admin)
        .err()
        .unwrap_or_else(|| panic!("administration invented a worktree"));
    assert_eq!(error.code, "primary_worktree_unknown");
}

#[test]
fn normal_administration_and_bare_repository_keep_their_primary_paths() {
    let fixture = Fixture::new();
    let main = fixture.0.join("main");
    fs::create_dir(&main).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(&main, &["init".as_ref(), "-b".as_ref(), "main".as_ref()]);
    fixture.git(
        &main,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let inventory = scan_family(&main.join(".git")).unwrap_or_else(|e| panic!("admin scan: {e}"));
    assert_eq!(
        inventory.families[0].worktrees[0].path,
        BytePath::from_path(&main)
    );
    let bare = fixture.0.join("arbitrary bare name");
    fs::create_dir(&bare).unwrap_or_else(|e| panic!("mkdir: {e}"));
    fixture.git(&bare, &["init".as_ref(), "--bare".as_ref()]);
    let inventory = scan_family(&bare).unwrap_or_else(|e| panic!("bare scan: {e}"));
    assert_eq!(
        inventory.families[0].worktrees[0].path,
        BytePath::from_path(&bare)
    );
    assert!(inventory.families[0].worktrees[0].bare);
}

#[test]
fn broken_registration_is_an_error_not_a_complete_partial_family() {
    let fixture = Fixture::new();
    fixture.git(
        &fixture.0,
        &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
    );
    fixture.git(
        &fixture.0,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let linked = fixture.0.join("linked");
    fixture.git(
        &fixture.0,
        &[
            "worktree".as_ref(),
            "add".as_ref(),
            "-b".as_ref(),
            "linked".as_ref(),
            linked.as_os_str(),
        ],
    );
    fs::write(fixture.0.join(".git/worktrees/linked/gitdir"), b"")
        .unwrap_or_else(|e| panic!("break own registration: {e}"));
    assert!(scan_family(&fixture.0).is_err());
}

#[test]
fn unborn_head_remains_unknown_and_does_not_claim_complete_collection() {
    let fixture = Fixture::new();
    fixture.git(
        &fixture.0,
        &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
    );
    let inventory = scan_family(&fixture.0).unwrap_or_else(|e| panic!("scan: {e}"));
    assert!(inventory.families[0].worktrees[0].head.is_none());
    assert!(!inventory.coverage.traversal_complete);
    assert!(!inventory.coverage.gaps.is_empty());
}

#[test]
fn controlled_reads_resolve_and_read_real_commit_and_local_refs() {
    let fixture = Fixture::new();
    fixture.git(
        &fixture.0,
        &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
    );
    fixture.git(
        &fixture.0,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let git = ControlledGit::default();
    let resolved = git
        .read(
            &fixture.0,
            GitRead::ResolveCommit {
                reference: BytePath::from_bytes(b"refs/heads/main"),
            },
        )
        .unwrap_or_else(|e| panic!("resolve: {e}"));
    let oid = String::from_utf8(resolved).unwrap_or_else(|e| panic!("oid: {e}"));
    let commit = git
        .read(
            &fixture.0,
            GitRead::Commit {
                oid: oid.trim_end_matches('\n').into(),
            },
        )
        .unwrap_or_else(|e| panic!("commit: {e}"));
    assert!(commit.starts_with(b"tree "));
    assert!(commit.ends_with(b"\ninitial\n"));
    let refs = git
        .read(&fixture.0, GitRead::LocalRefs)
        .unwrap_or_else(|e| panic!("refs: {e}"));
    assert!(refs.starts_with(b"refs/heads/main\0"));
    assert!(
        git.read(
            &fixture.0,
            GitRead::Commit {
                oid: "--help".into()
            }
        )
        .is_err()
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::ResolveCommit {
                reference: BytePath::from_bytes(b"--help")
            }
        )
        .is_err()
    );
}

#[test]
fn controlled_tree_and_remote_url_reads_keep_status_and_raw_fields() {
    let fixture = Fixture::new();
    fixture.git(
        &fixture.0,
        &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
    );
    fixture.git(
        &fixture.0,
        &[
            "commit".as_ref(),
            "--allow-empty".as_ref(),
            "-m".as_ref(),
            "initial".as_ref(),
        ],
    );
    let git = ControlledGit::default();
    assert!(
        git.read(&fixture.0, GitRead::RemoteUrls)
            .unwrap_or_else(|e| panic!("empty remote: {e}"))
            .is_empty()
    );
    fixture.git(
        &fixture.0,
        &[
            "remote".as_ref(),
            "add".as_ref(),
            "origin".as_ref(),
            "https://example.invalid/group/repo.git".as_ref(),
        ],
    );
    assert_eq!(
        git.read(&fixture.0, GitRead::RemoteUrls)
            .unwrap_or_else(|e| panic!("remote: {e}")),
        b"remote.origin.url\nhttps://example.invalid/group/repo.git\0"
    );
    let tree = fixture.git(&fixture.0, &["rev-parse".as_ref(), "HEAD^{tree}".as_ref()]);
    let oid = String::from_utf8(tree).unwrap_or_else(|e| panic!("tree oid: {e}"));
    assert!(
        git.read(
            &fixture.0,
            GitRead::Tree {
                oid: oid.trim_end_matches('\n').into()
            }
        )
        .unwrap_or_else(|e| panic!("tree: {e}"))
        .is_empty()
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::Tree {
                oid: "--help".into()
            }
        )
        .is_err()
    );
}

#[test]
fn controlled_blob_read_returns_exact_bytes_and_rejects_invalid_oid() {
    let fixture = Fixture::new();
    fixture.git(
        &fixture.0,
        &["init".as_ref(), "-b".as_ref(), "main".as_ref()],
    );
    fs::write(fixture.0.join("data"), b"synthetic\0binary\xff\n")
        .unwrap_or_else(|e| panic!("data: {e}"));
    fixture.git(&fixture.0, &["add".as_ref(), "data".as_ref()]);
    fixture.git(
        &fixture.0,
        &["commit".as_ref(), "-m".as_ref(), "initial".as_ref()],
    );
    let raw = fixture.git(&fixture.0, &["rev-parse".as_ref(), "HEAD:data".as_ref()]);
    let oid = String::from_utf8(raw).unwrap_or_else(|e| panic!("oid: {e}"));
    let git = ControlledGit::default();
    assert_eq!(
        git.read(
            &fixture.0,
            GitRead::Blob {
                oid: oid.trim_end_matches('\n').into()
            }
        )
        .unwrap_or_else(|e| panic!("blob: {e}")),
        b"synthetic\0binary\xff\n"
    );
    assert!(
        git.read(
            &fixture.0,
            GitRead::Blob {
                oid: "--help".into()
            }
        )
        .is_err()
    );
}
