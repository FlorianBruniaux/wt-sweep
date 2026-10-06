// Independent real-fixture T03 work-unit oracles. No product traversal computes expectations.
#![cfg(unix)]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};
use wt_sweep::{
    git::scan_family,
    model::{BytePath, Observations},
    review::{ReviewLimits, enrich_with_limits, enrich_with_limits_and_progress},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
// Captures are drained concurrently, capped, and every owned process is reaped.
fn bounded(mut command: Command) -> Vec<u8> {
    command
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("owned fixture child spawn");
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    fn capture(mut stream: impl Read + Send + 'static) -> thread::JoinHandle<(Vec<u8>, bool)> {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut overflow = false;
            let mut chunk = [0; 8192];
            loop {
                let read = stream.read(&mut chunk).expect("capture fixture output");
                if read == 0 {
                    break;
                }
                let retain = read.min((1024 * 1024_usize).saturating_sub(bytes.len()));
                bytes.extend_from_slice(&chunk[..retain]);
                overflow |= retain != read;
            }
            (bytes, overflow)
        })
    }
    let stdout = capture(stdout);
    let stderr = capture(stderr);
    let deadline = Instant::now() + Duration::from_secs(180);
    let mut timed_out = false;
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait fixture child") {
            break status;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            let mut killer = Command::new("/bin/kill")
                .args(["-KILL", &format!("-{}", child.id())])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .expect("stop owned fixture process group");
            let kill_deadline = Instant::now() + Duration::from_secs(2);
            while killer
                .try_wait()
                .expect("wait group signal utility")
                .is_none()
            {
                if Instant::now() >= kill_deadline {
                    killer.kill().expect("stop group signal utility");
                    killer.wait().expect("reap group signal utility");
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let _ = child.kill();
            break child.wait().expect("reap timed-out child");
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (out, out_overflow) = stdout.join().expect("join stdout reader");
    let (err, err_overflow) = stderr.join().expect("join stderr reader");
    assert!(
        !timed_out,
        "owned fixture process deadline, stopped and reaped"
    );
    assert!(
        !out_overflow && !err_overflow,
        "fixture capture exceeded 1 MiB bound"
    );
    assert!(
        status.success(),
        "fixture command failed: {}",
        String::from_utf8_lossy(&err)
    );
    out
}

struct Data {
    repo: PathBuf,
    paths: Vec<PathBuf>,
    heads: BTreeMap<PathBuf, String>,
    costly: String,
    target: String,
    target_blob: String,
}
struct Fixture {
    root: PathBuf,
    data: Data,
}
impl Fixture {
    fn git(&self, args: &[&str]) -> Vec<u8> {
        let mut command = Command::new("/usr/bin/git");
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("no-config"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_ATTR_NOSYSTEM", "1")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_ALLOW_PROTOCOL", "")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .env("LC_ALL", "C")
            .args([
                "-c",
                "core.hooksPath=/dev/null",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "gc.auto=0",
                "-c",
                "maintenance.auto=false",
                "-c",
                "protocol.allow=never",
                "-C",
            ])
            .arg(&self.data.repo)
            .args(args);
        bounded(command)
    }
    fn oid(&self, args: &[&str]) -> String {
        String::from_utf8(self.git(args)).unwrap().trim().to_owned()
    }
    fn new(count: usize, depth: usize) -> Self {
        assert!(count >= 2 && depth >= count);
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-t03-rust-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("exclusive fixture root");
        let repo = root.join("00-costly-principal");
        fs::create_dir(&repo).unwrap();
        let mut fixture = Self {
            root,
            data: Data {
                repo: repo.clone(),
                paths: vec![repo.clone()],
                heads: BTreeMap::new(),
                costly: String::new(),
                target: String::new(),
                target_blob: String::new(),
            },
        };
        fixture.git(&["init", "-q", "-b", "main"]);
        fs::write(repo.join("tracked.txt"), b"divergent source root\n").unwrap();
        fixture.git(&["add", "tracked.txt"]);
        fixture.git(&["commit", "-q", "-m", "Divergent source"]);
        let source = fixture.oid(&["rev-parse", "HEAD"]);
        fixture.data.costly = source.clone();
        fixture.data.heads.insert(repo.clone(), source.clone());
        fs::write(repo.join("tracked.txt"), b"small target content\n").unwrap();
        fixture.git(&["add", "tracked.txt"]);
        let tree = fixture.oid(&["write-tree"]);
        fixture.data.target_blob = fixture.oid(&["rev-parse", ":tracked.txt"]);
        let mut chain: Vec<String> = Vec::new();
        for number in 0..depth {
            let message = format!("Synthetic chain {number}");
            let next = if let Some(parent) = chain.last() {
                fixture.oid(&["commit-tree", &tree, "-p", parent, "-m", &message])
            } else {
                fixture.oid(&["commit-tree", &tree, "-m", &message])
            };
            chain.push(next);
        }
        let target = chain.last().unwrap().clone();
        fixture.data.target = target.clone();
        fixture.git(&["update-ref", "refs/heads/integration", &target]);
        fixture.git(&["reset", "--hard", "--quiet", &source]);
        for number in 1..count {
            let path = fixture.root.join(format!("simple-{number:03}"));
            let head = chain[chain.len() - number].clone();
            fixture.git(&[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                path.to_str().unwrap(),
                &head,
            ]);
            fixture.data.paths.push(path.clone());
            fixture.data.heads.insert(path, head);
        }
        fs::write(
            fixture.root.join("outside-sentinel"),
            b"preserve external owned sentinel\n",
        )
        .unwrap();
        fixture
    }
    fn manifest(&self) -> BTreeMap<PathBuf, Snapshot> {
        let mut result = BTreeMap::new();
        fn visit(path: &Path, result: &mut BTreeMap<PathBuf, Snapshot>) {
            use std::os::unix::ffi::OsStrExt;
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::symlink_metadata(path).unwrap();
            let content = if metadata.is_file() {
                fs::read(path).unwrap()
            } else if metadata.file_type().is_symlink() {
                fs::read_link(path).unwrap().as_os_str().as_bytes().to_vec()
            } else {
                Vec::new()
            };
            result.insert(
                path.to_owned(),
                Snapshot {
                    mode: metadata.mode(),
                    uid: metadata.uid(),
                    gid: metadata.gid(),
                    size: metadata.size(),
                    device: metadata.dev(),
                    inode: metadata.ino(),
                    links: metadata.nlink(),
                    mtime: (metadata.mtime(), metadata.mtime_nsec()),
                    ctime: (metadata.ctime(), metadata.ctime_nsec()),
                    content,
                },
            );
            if metadata.is_dir() {
                for entry in fs::read_dir(path).unwrap() {
                    visit(&entry.unwrap().path(), result);
                }
            }
        }
        visit(&self.root, &mut result);
        result
    }
    fn mutate_target(&self) {
        self.git(&["update-ref", "refs/heads/integration", &self.data.costly]);
    }
}
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    mode: u32,
    uid: u32,
    gid: u32,
    size: u64,
    device: u64,
    inode: u64,
    links: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
    content: Vec<u8>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove only independently owned disposable fixture");
    }
}
fn target() -> Vec<BytePath> {
    vec![BytePath::from_bytes(b"refs/heads/integration")]
}
fn by_path(facts: &Observations) -> BTreeMap<PathBuf, (String, Option<String>, Option<String>)> {
    facts
        .items
        .iter()
        .map(|item| {
            assert_eq!(item.history.len(), 1);
            assert_eq!(item.history_omitted_targets, 0);
            let history = &item.history[0];
            (
                item.path.to_path().unwrap(),
                (
                    history.result.clone(),
                    history.target_oid.clone(),
                    history.observed_result.clone(),
                ),
            )
        })
        .collect()
}

#[test]
fn independent_small_heads_receive_fair_history_with_fixed_work_units() {
    for count in [4, 8] {
        let fixture = Fixture::new(count, 32);
        let mut inventory = scan_family(&fixture.data.repo).expect("real fixture inventory");
        let before = fixture.manifest();
        let limits = ReviewLimits {
            object_read_attempts: 48,
            traversal_steps: 512,
            per_target_steps: 64,
            deep_object_read_attempts: 1,
            ..ReviewLimits::default()
        };
        let first =
            enrich_with_limits(&inventory, &target(), None, &limits).expect("bounded history");
        let outcomes = by_path(&first);
        assert_eq!(outcomes.len(), count, "omitted worktree");
        assert_eq!(
            outcomes[&fixture.data.repo].0, "not_ancestor",
            "complete divergent DAG is negative"
        );
        for path in &fixture.data.paths[1..] {
            let expected = if fixture.data.heads[path] == fixture.data.target {
                "identical_commit"
            } else {
                "ancestor"
            };
            assert_eq!(
                outcomes[path].0, expected,
                "simple HEAD starved behind costly first row"
            );
            assert_eq!(outcomes[path].1.as_ref(), Some(&fixture.data.target));
        }
        assert_eq!(first.history_metrics.items_total, count as u64);
        assert_eq!(first.history_metrics.first_pass_items, count as u64);
        assert!(first.history_metrics.object_read_attempts <= limits.object_read_attempts as u64);
        assert!(first.history_metrics.object_bytes <= limits.object_bytes as u64);
        assert!(first.history_metrics.traversal_steps <= limits.traversal_steps as u64);
        assert!(
            first.items.iter().all(|item| item.observations.is_empty()),
            "one-object deep budget cannot produce a verified tree/content proof"
        );
        inventory.families[0].worktrees.reverse();
        let second =
            enrich_with_limits(&inventory, &target(), None, &limits).expect("permuted history");
        assert_eq!(
            outcomes,
            by_path(&second),
            "equivalent coverage changed history results by order"
        );
        assert_eq!(
            before,
            fixture.manifest(),
            "source/admin/index/sentinel changed"
        );
    }
}

#[test]
fn exhaustion_caps_reads_and_job_allocation_without_pointless_target_resolutions() {
    let fixture = Fixture::new(8, 32);
    let inventory = scan_family(&fixture.data.repo).unwrap();
    // Distinct unresolved refs are intentional missing evidence. Target ordinal
    // allocation must give each row its useful first target before target two.
    let mut targets = target();
    targets.extend(
        (1..128).map(|number| {
            BytePath::from_bytes(format!("refs/heads/missing-{number:03}").as_bytes())
        }),
    );
    let before = fixture.manifest();
    let limits = ReviewLimits {
        object_read_attempts: 5,
        object_bytes: 4096,
        target_jobs: 8,
        deep_object_read_attempts: 1,
        ..ReviewLimits::default()
    };
    let facts = enrich_with_limits(&inventory, &targets, None, &limits).unwrap();
    assert_eq!(
        facts.history_metrics.first_pass_items, 8,
        "first target jobs were row-major starved"
    );
    assert!(
        facts.history_metrics.object_read_attempts <= 5,
        "object cap exceeded"
    );
    assert!(
        facts.history_metrics.object_bytes <= 4096,
        "byte cap exceeded"
    );
    assert!(
        facts.history_metrics.target_resolution_attempts <= 2,
        "uncached omitted targets resolved after useful history jobs were exhausted"
    );
    assert!(
        facts.history_metrics.stop_reason.is_some(),
        "exhaustion cause absent"
    );
    for item in &facts.items {
        assert_eq!(item.history.len(), 1);
        assert_eq!(
            item.history_omitted_targets, 127,
            "omitted jobs silently vanished"
        );
        assert_eq!(item.history[0].target_ref, targets[0]);
        assert!(!item.unknowns.is_empty());
    }
    assert_eq!(before, fixture.manifest());
}

#[test]
fn missing_blob_cannot_turn_graph_ancestry_into_deep_content_evidence() {
    let fixture = Fixture::new(3, 3);
    let inventory = scan_family(&fixture.data.repo).unwrap();
    let oid = &fixture.data.target_blob;
    let path = fixture
        .data
        .repo
        .join(".git/objects")
        .join(&oid[..2])
        .join(&oid[2..]);
    assert!(
        path.is_file(),
        "fault injection must address a real owned blob"
    );
    fs::remove_file(path).unwrap();
    let before = fixture.manifest();
    let facts = enrich_with_limits(&inventory, &target(), None, &ReviewLimits::default()).unwrap();
    let item = facts
        .items
        .iter()
        .find(|item| item.path.to_path().unwrap() == fixture.data.paths[2])
        .unwrap();
    assert_eq!(item.history[0].result, "ancestor");
    assert!(
        item.observations
            .iter()
            .all(|observation| observation.target_role != "integration"),
        "available raw commits promoted over missing blob into a deep integration proof"
    );
    assert_eq!(before, fixture.manifest());
}

#[test]
fn target_mutation_at_final_revalidation_preserves_observed_result_as_unknown() {
    let fixture = Fixture::new(3, 3);
    let inventory = scan_family(&fixture.data.repo).unwrap();
    let mut mutated = false;
    let mut after_mutation = None;
    let facts = enrich_with_limits_and_progress(
        &inventory,
        &target(),
        None,
        &ReviewLimits::default(),
        &mut |event| {
            if event.phase == "history_revalidate" && !mutated {
                fixture.mutate_target();
                mutated = true;
                after_mutation = Some(fixture.manifest());
            }
        },
    )
    .unwrap();
    assert!(
        mutated,
        "deterministic final revalidation boundary was not reached"
    );
    let item = facts
        .items
        .iter()
        .find(|item| item.path.to_path().unwrap() == fixture.data.paths[2])
        .unwrap();
    assert_eq!(
        item.history[0].result, "unknown",
        "stale target stayed current positive"
    );
    assert_eq!(
        item.history[0].observed_result.as_deref(),
        Some("ancestor"),
        "prior verified graph result was erased"
    );
    assert!(item.history[0].stop_reason.is_some());
    assert!(
        item.observations
            .iter()
            .all(|observation| observation.target_role != "integration"),
        "deep proof retained after its target changed"
    );
    assert_eq!(
        after_mutation.unwrap(),
        fixture.manifest(),
        "product changed sources after test-owned mutation"
    );
}

#[test]
fn invalid_budget_and_more_than_128_explicit_targets_refuse_before_fixture_io() {
    let fixture = Fixture::new(2, 3);
    let inventory = scan_family(&fixture.data.repo).unwrap();
    let before = fixture.manifest();
    let limits = ReviewLimits {
        slice_object_read_attempts: 0,
        ..ReviewLimits::default()
    };
    assert!(
        enrich_with_limits(&inventory, &target(), None, &limits).is_err(),
        "zero slice silently accepted"
    );
    let targets: Vec<_> = (0..129)
        .map(|number| BytePath::from_bytes(format!("refs/heads/target-{number}").as_bytes()))
        .collect();
    assert!(
        enrich_with_limits(&inventory, &targets, None, &ReviewLimits::default()).is_err(),
        "oversized explicit target list silently truncated"
    );
    assert_eq!(before, fixture.manifest());
}
