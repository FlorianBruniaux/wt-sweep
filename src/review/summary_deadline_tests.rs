//! Independent same-family aggregation oracle. Limits alone are injected.
use super::*;
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs, io,
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
type TestResult<T = ()> = Result<T, Box<dyn Error>>;

// Setup emits only short builtin Git results. No hooks, external transports or
// implicit downloads can run; every command has its own bounded process group.
fn bounded(mut command: Command) -> io::Result<Vec<u8>> {
    let mut child = command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait()?.is_none() {
        if Instant::now() >= deadline {
            let mut signal = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{}", child.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()?;
            let signal_deadline = Instant::now() + Duration::from_secs(2);
            while signal.try_wait()?.is_none() {
                if Instant::now() >= signal_deadline {
                    signal.kill()?;
                    signal.wait()?;
                    break;
                }
                thread::sleep(Duration::from_millis(5));
            }
            let _ = child.kill();
            child.wait()?;
            return Err(io::Error::other(
                "owned fixture command deadline; stopped and reaped",
            ));
        }
        thread::sleep(Duration::from_millis(5));
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    Ok(output.stdout)
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> TestResult<Self> {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-summary-aggregation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root)?;
        Ok(Self(root))
    }
    fn git(&self, path: &Path, args: &[&str]) -> TestResult<String> {
        let mut command = Command::new("/usr/bin/git");
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("no-config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-C",
            ])
            .arg(path)
            .args(args);
        Ok(String::from_utf8(bounded(command)?)?
            .trim_end_matches('\n')
            .into())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Compare all owned files, hidden administration, FIFO identity and metadata.
// Atime is excluded because reading fixture bytes can update it.
fn manifest(root: &Path) -> io::Result<BTreeMap<PathBuf, String>> {
    fn visit(path: &Path, output: &mut BTreeMap<PathBuf, String>) -> io::Result<()> {
        let m = fs::symlink_metadata(path)?;
        let content = if m.file_type().is_file() {
            format!("{:x}", Sha256::digest(fs::read(path)?))
        } else if m.file_type().is_symlink() {
            format!("{:?}", fs::read_link(path)?)
        } else {
            String::new()
        };
        output.insert(
            path.into(),
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                m.dev(),
                m.ino(),
                m.mode(),
                m.uid(),
                m.gid(),
                m.nlink(),
                m.len(),
                m.mtime(),
                m.mtime_nsec(),
                m.ctime(),
                m.ctime_nsec(),
                m.rdev(),
                content
            ),
        );
        if m.is_dir() {
            for entry in fs::read_dir(path)? {
                visit(&entry?.path(), output)?;
            }
        }
        Ok(())
    }
    let mut output = BTreeMap::new();
    visit(root, &mut output)?;
    Ok(output)
}

#[test]
fn shortened_local_deadline_does_not_quarantine_healthy_worktree_in_same_family() -> TestResult {
    let f = Fixture::new()?;
    let healthy_path = f.0.join("principal");
    let blocked_path = f.0.join("first-linked");
    fs::create_dir(&healthy_path)?;
    f.git(&healthy_path, &["init", "--template=", "-b", "main"])?;
    fs::write(healthy_path.join("tracked"), b"base\n")?;
    f.git(&healthy_path, &["add", "tracked"])?;
    f.git(&healthy_path, &["commit", "-m", "fixture"])?;
    f.git(
        &healthy_path,
        &[
            "worktree",
            "add",
            "-b",
            "topic",
            blocked_path.to_str().ok_or("fixture path UTF-8")?,
        ],
    )?;
    f.git(
        &healthy_path,
        &["config", "extensions.worktreeConfig", "true"],
    )?;
    let head = f.git(&healthy_path, &["rev-parse", "HEAD"])?;
    assert_eq!(head, f.git(&blocked_path, &["rev-parse", "HEAD"])?);
    assert_eq!(f.git(&blocked_path, &["status", "--porcelain=v1"])?, "");
    fs::write(
        healthy_path.join("valuable-local-notes"),
        b"retain this current work\n",
    )?;
    assert_eq!(
        f.git(&healthy_path, &["status", "--porcelain=v1"])?,
        "?? valuable-local-notes"
    );

    let admin = PathBuf::from(f.git(&blocked_path, &["rev-parse", "--absolute-git-dir"])?);
    assert!(admin.starts_with(healthy_path.join(".git/worktrees")));
    let mut inventory = crate::git::scan_family(&healthy_path)?;
    assert_eq!(inventory.families.len(), 1);
    let family = &mut inventory.families[0];
    assert_eq!(family.worktrees.len(), 2);
    family
        .worktrees
        .sort_by_key(|w| w.path != BytePath::from_path(&blocked_path));
    assert_eq!(family.worktrees[0].path, BytePath::from_path(&blocked_path));
    assert_eq!(family.worktrees[1].path, BytePath::from_path(&healthy_path));
    let expected_identity = fs::metadata(healthy_path.join(".git"))?;
    assert_eq!(family.identity.device, expected_identity.dev());
    assert_eq!(family.identity.inode, expected_identity.ino());

    let fifo = f.0.join("owned-blocked-include");
    let mut command = Command::new("/usr/bin/mkfifo");
    command.env_clear().arg(&fifo);
    bounded(command)?;
    fs::write(
        admin.join("config.worktree"),
        format!("[include]\n path = {}\n", fifo.display()),
    )?;
    let before = manifest(&f.0)?;
    crate::git::take_child_read_journal();
    let mut calls = Vec::new();
    let mut first_reads = Vec::new();
    let mut second_reads = Vec::new();
    let collected =
        local_summaries_collect(&inventory, 1024 * 1024, &BTreeMap::new(), |w, limits| {
            calls.push(w.path.clone());
            let mut limits = limits.clone();
            if w.path == BytePath::from_path(&blocked_path) {
                limits.timeout = Duration::from_millis(100);
            }
            let outcome = crate::local::collect_summary_for_review(w, &limits);
            let reads = crate::git::take_child_read_journal();
            if w.path == BytePath::from_path(&blocked_path) {
                first_reads = reads;
            } else {
                second_reads = reads;
            }
            outcome
        })?;
    assert_eq!(
        before,
        manifest(&f.0)?,
        "collection changed owned work/admin bytes or metadata"
    );
    assert_eq!(
        first_reads.len(),
        1,
        "fixture must reach one actual shortened Git child, without dependent retries"
    );
    assert_eq!(first_reads[0].0, BytePath::from_path(&blocked_path));
    assert!(matches!(first_reads[0].1, GitRead::GitDir));
    assert_eq!(
        calls,
        [
            BytePath::from_path(&blocked_path),
            BytePath::from_path(&healthy_path)
        ],
        "healthy same-family collector was skipped"
    );
    assert!(
        collected.stopped_families.is_empty(),
        "facet budget became family backend quarantine"
    );
    assert!(
        !second_reads.is_empty(),
        "healthy same-family collection launched no real reads"
    );
    assert!(
        second_reads
            .iter()
            .all(|(path, _)| path == &BytePath::from_path(&healthy_path))
    );
    let second = collected.summaries[&BytePath::from_path(&healthy_path)]
        .as_ref()
        .ok_or("healthy summary omitted")?;
    assert_eq!(second.head.as_ref(), Some(&head));
    assert!(
        second.head_complete && second.index_complete && second.disk_complete,
        "{second:?}"
    );
    assert!(
        !second.sources.is_empty(),
        "healthy summary replaced by unavailable placeholder"
    );
    assert!(second.gaps.is_empty(), "{second:?}");
    assert_eq!(second.untracked.observed, 1);
    assert!(second.untracked.complete);
    for count in [
        &second.staged,
        &second.unstaged,
        &second.conflicts,
        &second.ignored,
    ] {
        assert_eq!(count.observed, 0);
        assert!(count.complete);
    }
    assert!(
        !second
            .gaps
            .iter()
            .any(|g| g.reason.contains("family collection stopped"))
    );

    let first = collected.summaries[&BytePath::from_path(&blocked_path)]
        .as_ref()
        .ok_or("first summary omitted")?;
    assert!(!first.head_complete && !first.index_complete && !first.disk_complete);
    assert!(
        first
            .gaps
            .iter()
            .any(|g| g.reason.contains("summary time budget")),
        "{first:?}"
    );

    // Reverse control: the same actual blocked child now has a default 30s
    // facet allowance, so the native 10s backend cap must quarantine this family.
    // Never fabricate a timeout result or substitute a shorter backend cap.
    calls.clear();
    first_reads.clear();
    second_reads.clear();
    crate::git::take_child_read_journal();
    let quarantined =
        local_summaries_collect(&inventory, 1024 * 1024, &BTreeMap::new(), |w, limits| {
            calls.push(w.path.clone());
            assert_eq!(
                limits.timeout,
                crate::local::SummaryLimits::default().timeout
            );
            let outcome = crate::local::collect_summary_for_review(w, limits);
            let reads = crate::git::take_child_read_journal();
            if w.path == BytePath::from_path(&blocked_path) {
                first_reads = reads;
            } else {
                second_reads = reads;
            }
            outcome
        })?;
    assert_eq!(
        before,
        manifest(&f.0)?,
        "backend timeout changed owned fixture"
    );
    assert_eq!(
        calls,
        [BytePath::from_path(&blocked_path)],
        "backend-failed family retried healthy row"
    );
    assert_eq!(
        first_reads.len(),
        1,
        "backend timeout launched dependent facet children"
    );
    assert!(matches!(first_reads[0].1, GitRead::GitDir));
    assert!(
        second_reads.is_empty(),
        "quarantined family launched a second worktree child"
    );
    let family = &inventory.families[0];
    let family_id = (family.identity.device, family.identity.inode);
    assert!(matches!(
        quarantined.stopped_families.get(&family_id),
        Some(GitRead::GitDir)
    ));
    let first = quarantined.summaries[&BytePath::from_path(&blocked_path)]
        .as_ref()
        .ok_or("backend failed summary omitted")?;
    assert!(
        first.gaps.iter().any(|g| g.reason.contains("git_timeout")),
        "{first:?}"
    );
    let second = quarantined.summaries[&BytePath::from_path(&healthy_path)]
        .as_ref()
        .ok_or("quarantine placeholder omitted")?;
    assert!(second.sources.is_empty());
    assert!(!second.head_complete && !second.index_complete && !second.disk_complete);
    assert!(
        !second.untracked.complete,
        "quarantine fabricated complete absence of known local work"
    );
    assert!(
        second.gaps.iter().any(|g| g
            .reason
            .contains("family collection stopped after Git timeout in GitDir")),
        "{second:?}"
    );
    Ok(())
}

#[test]
fn local_allowance_reserves_time_for_later_worktrees() -> TestResult {
    let f = Fixture::new()?;
    f.git(&f.0, &["init", "-b", "main"])?;
    fs::write(f.0.join("tracked"), b"base")?;
    f.git(&f.0, &["add", "."])?;
    f.git(&f.0, &["commit", "-m", "fixture"])?;
    let mut inventory = crate::git::scan_family(&f.0)?;
    let original = inventory.families[0].worktrees[0].clone();
    inventory.families[0].worktrees = (0..50)
        .map(|n| {
            let mut w = original.clone();
            w.path = BytePath::from_path(&f.0.join(format!("owned-{n}")));
            w
        })
        .collect();
    let mut allowances = Vec::new();
    local_summaries_collect(
        &inventory,
        24 * 1024 * 1024,
        &BTreeMap::new(),
        |w, limits| {
            allowances.push(limits.timeout);
            Ok(crate::local::SummaryOutcome {
                metrics: Default::default(),
                summary: unavailable_local_summary(w, "injected no-I/O collector".into())?,
                first_git_timeout: None,
            })
        },
    )?;
    assert_eq!(allowances.len(), 50);
    assert!(
        allowances[0] <= Duration::from_millis(2400),
        "first worktree consumed allowance reserved for others: {:?}",
        allowances[0]
    );
    assert!(allowances[49] <= Duration::from_secs(30));
    Ok(())
}
