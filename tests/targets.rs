#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};
use wt_sweep::{
    git::scan_family,
    model::{BytePath, Inventory},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from(format!(
            "/private/tmp/wt-sweep-t06-targets-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("exclusive synthetic fixture root");
        Self(root)
    }
    fn git(&self, repo: &Path, args: &[&OsStr]) -> Vec<u8> {
        let mut child = Command::new("/usr/bin/git")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &self.0)
            .env("XDG_CONFIG_HOME", self.0.join("no-config"))
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
            .arg(repo)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        loop {
            if child.try_wait().unwrap().is_some() {
                break;
            }
            if start.elapsed() > Duration::from_secs(15) {
                let _ = child.kill();
                let _ = child.wait();
                panic!("owned fixture Git child exceeded deadline");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        let out = child.wait_with_output().unwrap();
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.stdout.len() <= 1024 * 1024);
        out.stdout
    }
    fn git_text(&self, repo: &Path, args: &[&str]) -> String {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        String::from_utf8(self.git(repo, &args))
            .unwrap()
            .trim()
            .into()
    }
    fn repo(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir(&path).unwrap();
        self.git_text(&path, &["init", "-q", "-b", "main"]);
        fs::write(path.join("file"), name).unwrap();
        self.git_text(&path, &["add", "file"]);
        self.git_text(&path, &["commit", "-q", "-m", "synthetic main"]);
        path
    }
    fn inventory(&self, repos: &[&Path]) -> Inventory {
        let mut inventory = scan_family(repos[0]).unwrap();
        for repo in &repos[1..] {
            inventory
                .families
                .extend(scan_family(repo).unwrap().families);
        }
        inventory
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn simple_nonempty_targets_refuse_multiple_families_in_review() {
    // Broadcasting a target to every physical family must make this assertion fail.
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    let inventory = f.inventory(&[&a, &b]);
    let targets = [BytePath::from_bytes(b"refs/heads/main")];
    assert!(
        wt_sweep::review::enrich(&inventory, &targets, None).is_err(),
        "legacy nonempty target slice broadcast across physical families"
    );
    assert!(
        wt_sweep::review::enrich(&inventory, &[], None).is_ok(),
        "empty legacy selection must remain available across families"
    );
}
#[test]
fn simple_nonempty_targets_refuse_multiple_families_in_standalone_private_collection() {
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    let inventory = f.inventory(&[&a, &b]);
    let targets = [BytePath::from_bytes(b"refs/heads/main")];
    assert!(
        wt_sweep::review::private::collect_with_limits(&inventory, &targets, &Default::default())
            .is_err(),
        "standalone private collection broadcast legacy targets across families"
    );
    assert!(
        wt_sweep::review::private::collect_with_limits(&inventory, &[], &Default::default())
            .is_ok()
    );
}

use std::{
    collections::BTreeMap,
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, symlink},
    },
};
use wt_sweep::{
    model::{TARGET_MAP_SCHEMA, TargetMap, TargetMapFamily},
    review::{
        ReviewLimits, enrich_with_target_input_and_controls,
        targets::{FamilyTargets, TargetInput},
    },
};
fn entry(inventory: &Inventory, family: usize, targets: &[&[u8]]) -> TargetMapFamily {
    let family = &inventory.families[family];
    TargetMapFamily {
        common_dir: family.common_dir.clone(),
        identity: family.identity.clone(),
        targets: targets.iter().map(|t| BytePath::from_bytes(t)).collect(),
    }
}
fn mapped(
    inventory: &Inventory,
    map: &TargetMap,
    limits: &ReviewLimits,
) -> wt_sweep::model::Observations {
    enrich_with_target_input_and_controls(
        inventory,
        TargetInput::Map(map),
        None,
        limits,
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |_| {},
    )
    .unwrap()
}
fn map(families: Vec<TargetMapFamily>) -> TargetMap {
    TargetMap {
        schema: TARGET_MAP_SCHEMA.into(),
        families,
    }
}
#[derive(Debug, PartialEq, Eq)]
struct ResidentState {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    mtime: (i64, i64),
    ctime: (i64, i64),
    content: Vec<u8>,
}
fn manifest(root: &Path) -> BTreeMap<PathBuf, ResidentState> {
    fn visit(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, ResidentState>) {
        let m = fs::symlink_metadata(path).unwrap();
        let content = if m.is_file() {
            fs::read(path).unwrap()
        } else if m.file_type().is_symlink() {
            fs::read_link(path).unwrap().as_os_str().as_bytes().to_vec()
        } else {
            Vec::new()
        };
        result.insert(
            path.strip_prefix(root).unwrap().to_path_buf(),
            ResidentState {
                device: m.dev(),
                inode: m.ino(),
                mode: m.mode(),
                links: m.nlink(),
                mtime: (m.mtime(), m.mtime_nsec()),
                ctime: (m.ctime(), m.ctime_nsec()),
                content,
            },
        );
        if m.is_dir() {
            let mut children: Vec<_> = fs::read_dir(path)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            children.sort();
            for child in children {
                visit(root, &child, result);
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}
fn develop(f: &Fixture, repo: &Path) {
    f.git_text(repo, &["checkout", "-q", "-b", "develop"]);
    fs::write(repo.join("file"), b"distinct synthetic develop content").unwrap();
    f.git_text(repo, &["add", "file"]);
    f.git_text(repo, &["commit", "-q", "-m", "synthetic develop"]);
    f.git_text(repo, &["checkout", "-q", "main"]);
}
fn linked(f: &Fixture, repo: &Path, reference: &str) -> PathBuf {
    let path = repo.join("linked");
    f.git(
        repo,
        &[
            OsStr::new("worktree"),
            OsStr::new("add"),
            OsStr::new("--detach"),
            path.as_os_str(),
            OsStr::new(reference),
        ],
    );
    path
}
#[test]
fn family_maps_select_distinct_graph_and_deep_targets_without_mutating_sources() {
    // Replacing the family lookup with a broadcast must expose a wrong target on either row.
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    develop(&f, &a);
    develop(&f, &b);
    let a_linked = linked(&f, &a, "main");
    let b_linked = linked(&f, &b, "develop");
    let a_oid = f.git_text(&a, &["rev-parse", "refs/heads/main"]);
    let b_oid = f.git_text(&b, &["rev-parse", "refs/heads/develop"]);
    let inventory = f.inventory(&[&a, &b]);
    let selected = map(vec![
        entry(&inventory, 0, &[b"refs/heads/main"]),
        entry(&inventory, 1, &[b"refs/heads/develop"]),
    ]);
    let before = manifest(&f.0);
    let facts = mapped(&inventory, &selected, &Default::default());
    for (path, reference, oid) in [
        (&a_linked, b"refs/heads/main".as_slice(), &a_oid),
        (&b_linked, b"refs/heads/develop".as_slice(), &b_oid),
    ] {
        let row = facts
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(path))
            .unwrap();
        assert_eq!(row.history.len(), 1);
        assert_eq!(row.history[0].target_ref.bytes().unwrap(), reference);
        assert_eq!(row.history[0].target_oid.as_ref(), Some(oid));
        assert_eq!(row.history[0].result, "identical_commit");
        let integration: Vec<_> = row
            .observations
            .iter()
            .filter(|o| o.target_role == "integration")
            .collect();
        assert!(
            !integration.is_empty(),
            "family-selected target never reached deep proof"
        );
        assert!(
            integration
                .iter()
                .all(|o| o.target_ref.bytes().unwrap() == reference && o.target_oid == *oid)
        );
        assert!(
            row.private_history
                .as_ref()
                .unwrap()
                .roots
                .iter()
                .any(|r| r.result == "covered")
        );
    }
    let private = wt_sweep::review::private::collect_with_target_input(
        &inventory,
        TargetInput::Map(&selected),
        &Default::default(),
        &wt_sweep::local::NativeHooks,
    )
    .unwrap();
    assert!(
        private.histories[&BytePath::from_path(&a_linked)]
            .roots
            .iter()
            .any(|r| r.result == "covered")
    );
    assert!(
        private.histories[&BytePath::from_path(&b_linked)]
            .roots
            .iter()
            .any(|r| r.result == "covered")
    );
    assert_eq!(
        before,
        manifest(&f.0),
        "collection changed resident sources or metadata"
    );
}
#[test]
fn missing_family_mapping_never_guesses_or_broadcasts_a_target() {
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    let b_linked = linked(&f, &b, "main");
    let inventory = f.inventory(&[&a, &b]);
    let selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
    let facts = mapped(&inventory, &selected, &Default::default());
    for row in facts
        .items
        .iter()
        .filter(|i| i.path.to_path().unwrap().starts_with(&b))
    {
        assert!(
            row.history.is_empty(),
            "unmapped family received an integration target job"
        );
        assert!(
            row.observations
                .iter()
                .all(|o| o.target_role != "integration")
        );
    }
    let judged = wt_sweep::review::judge(&facts).unwrap();
    let row = judged
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&b_linked))
        .unwrap();
    assert_eq!(row.classification, "verify");
    assert!(
        row.decision
            .as_ref()
            .unwrap()
            .missing_conditions
            .iter()
            .any(|c| c == "C2")
    );
}
#[test]
fn identical_map_bindings_normalize_target_order_and_duplicates() {
    let f = Fixture::new();
    let a = f.repo("a");
    f.git_text(&a, &["branch", "a-first"]);
    f.git_text(&a, &["branch", "z-last"]);
    let inventory = f.inventory(&[&a]);
    let first = entry(
        &inventory,
        0,
        &[
            b"refs/heads/z-last",
            b"refs/heads/a-first",
            b"refs/heads/z-last",
        ],
    );
    let second = entry(
        &inventory,
        0,
        &[b"refs/heads/a-first", b"refs/heads/z-last"],
    );
    let selected = map(vec![first, second]);
    let facts = mapped(&inventory, &selected, &Default::default());
    let refs: Vec<_> = facts.items[0]
        .history
        .iter()
        .map(|h| h.target_ref.bytes().unwrap())
        .collect();
    assert_eq!(
        refs,
        [
            b"refs/heads/a-first".to_vec(),
            b"refs/heads/z-last".to_vec()
        ]
    );
    assert!(
        facts.items[0]
            .history
            .iter()
            .all(|h| h.result == "identical_commit")
    );
}
#[test]
fn contradictory_map_bindings_refuse_instead_of_selecting_the_first() {
    let f = Fixture::new();
    let a = f.repo("a");
    let inventory = f.inventory(&[&a]);
    let selected = map(vec![
        entry(&inventory, 0, &[b"refs/heads/main"]),
        entry(&inventory, 0, &[b"refs/heads/develop"]),
    ]);
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err());
}
#[test]
fn map_identity_and_common_directory_must_match_the_saved_family() {
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    let inventory = f.inventory(&[&a, &b]);
    let mut stale = entry(&inventory, 0, &[b"refs/heads/main"]);
    stale.identity.inode = stale.identity.inode.wrapping_add(1);
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&map(vec![stale]))).is_err());
    let mut wrong_path = entry(&inventory, 0, &[b"refs/heads/main"]);
    wrong_path.common_dir = inventory.families[1].common_dir.clone();
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&map(vec![wrong_path]))).is_err());
}
#[test]
fn replaced_or_symlinked_live_family_cannot_rebind_even_with_the_same_saved_path() {
    let f = Fixture::new();
    let a = f.repo("a");
    let inventory = f.inventory(&[&a]);
    let selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
    let common = a.join(".git");
    let saved = a.join("admin-before");
    fs::rename(&common, &saved).unwrap();
    fs::create_dir(&common).unwrap();
    assert!(
        FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err(),
        "new directory was rebound to old targets"
    );
    fs::remove_dir(&common).unwrap();
    symlink(&saved, &common).unwrap();
    assert!(
        FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err(),
        "NOFOLLOW refused no alias despite same inode"
    );
}
#[test]
fn ambiguous_source_paths_refuse_even_when_all_target_lists_are_empty() {
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    let mut inventory = f.inventory(&[&a, &b]);
    inventory.families[1].worktrees[0].path = inventory.families[0].worktrees[0].path.clone();
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&map(Vec::new()))).is_err());
    assert!(wt_sweep::review::enrich(&inventory, &[], None).is_err());
}
#[test]
fn family_local_target_ordinals_preserve_fairness_and_exact_omitted_counts() {
    let f = Fixture::new();
    let a = f.repo("a");
    let b = f.repo("b");
    f.git_text(&a, &["branch", "a-first"]);
    for name in ["z-second", "z-third"] {
        f.git_text(&b, &["branch", name]);
    }
    let inventory = f.inventory(&[&a, &b]);
    let selected = map(vec![
        entry(&inventory, 0, &[b"refs/heads/main", b"refs/heads/a-first"]),
        entry(
            &inventory,
            1,
            &[
                b"refs/heads/z-third",
                b"refs/heads/main",
                b"refs/heads/z-second",
            ],
        ),
    ]);
    let limits = ReviewLimits {
        target_jobs: 2,
        ..Default::default()
    };
    let facts = mapped(&inventory, &selected, &limits);
    assert_eq!(facts.history_metrics.first_pass_items, 2);
    for (path, first, omitted) in [
        (&a, b"refs/heads/a-first".as_slice(), 1),
        (&b, b"refs/heads/main".as_slice(), 2),
    ] {
        let row = facts
            .items
            .iter()
            .find(|i| i.path == BytePath::from_path(path))
            .unwrap();
        assert_eq!(row.history.len(), 1);
        assert_eq!(row.history[0].target_ref.bytes().unwrap(), first);
        assert_eq!(row.history[0].result, "identical_commit");
        assert_eq!(row.history_omitted_targets, omitted);
    }
}
#[test]
fn raw_non_utf8_ref_bytes_survive_binding_and_graph_observation() {
    let f = Fixture::new();
    let a = f.repo("a");
    let reference = b"refs/heads/raw-\xff";
    let oid = f.git_text(&a, &["rev-parse", "HEAD"]);
    // APFS refuses raw non-UTF8 loose filenames. Packed refs retain the raw name
    // in ordinary file bytes and still exercise the real Git reader.
    let mut packed = format!("# pack-refs with: peeled fully-peeled sorted\n{oid} ").into_bytes();
    packed.extend_from_slice(reference);
    packed.push(b'\n');
    fs::write(a.join(".git/packed-refs"), packed).unwrap();
    let inventory = f.inventory(&[&a]);
    let selected = map(vec![entry(&inventory, 0, &[reference])]);
    let facts = mapped(&inventory, &selected, &Default::default());
    assert_eq!(
        facts.items[0].history[0].target_ref.bytes().unwrap(),
        reference
    );
    assert_eq!(facts.items[0].history[0].target_oid.as_ref(), Some(&oid));
    assert_eq!(facts.items[0].history[0].result, "identical_commit");
}
#[test]
fn final_family_replacement_invalidates_previously_bound_targets() {
    let f = Fixture::new();
    let a = f.repo("a");
    let a_linked = linked(&f, &a, "main");
    let inventory = f.inventory(&[&a]);
    let selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
    let mut replaced = false;
    let facts = enrich_with_target_input_and_controls(
        &inventory,
        TargetInput::Map(&selected),
        None,
        &Default::default(),
        &Default::default(),
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            if event.phase == "history_revalidate" {
                fs::rename(a.join(".git"), f.0.join("retained-admin")).unwrap();
                fs::create_dir(a.join(".git")).unwrap();
                replaced = true;
            }
        },
    )
    .unwrap();
    assert!(replaced);
    let row = facts
        .items
        .iter()
        .find(|i| i.path == BytePath::from_path(&a_linked))
        .unwrap();
    assert!(row.observations.is_empty());
    assert_eq!(row.history[0].result, "unknown");
    assert_eq!(
        row.history[0].observed_result.as_deref(),
        Some("identical_commit")
    );
    let judged = wt_sweep::review::judge(&facts).unwrap();
    assert!(
        judged
            .items
            .iter()
            .all(|i| i.classification != "cleanup_candidate")
    );
}

#[test]
fn invalid_schema_paths_refs_and_excessive_family_targets_refuse() {
    let f = Fixture::new();
    let a = f.repo("a");
    let inventory = f.inventory(&[&a]);
    let mut selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
    selected.schema = "wt-sweep.targets/999".into();
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err());
    for raw in [
        b"main".as_slice(),
        b"HEAD",
        b"refs/heads/main~1",
        b"refs/tags/main",
    ] {
        let selected = map(vec![entry(&inventory, 0, &[raw])]);
        assert!(
            FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err(),
            "invalid integration ref was accepted: {raw:?}"
        );
    }
    for path in [
        BytePath::from_bytes(b"relative"),
        BytePath { hex: "FF".into() },
        BytePath { hex: "00".into() },
    ] {
        let mut selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
        selected.families[0].common_dir = path;
        assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err());
    }
    let mut selected = map(vec![entry(&inventory, 0, &[])]);
    selected.families[0].targets = (0..129)
        .map(|n| BytePath::from_bytes(format!("refs/heads/target-{n}").as_bytes()))
        .collect();
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&selected)).is_err());
}

struct BindingRefusal(BytePath);
impl wt_sweep::local::LocalHooks for BindingRefusal {
    fn before_entry(&self, path: &BytePath) -> std::io::Result<()> {
        if path == &self.0 {
            Err(std::io::Error::from_raw_os_error(libc::EACCES))
        } else {
            Ok(())
        }
    }
}
impl wt_sweep::review::control::ControlHooks for BindingRefusal {}
#[test]
fn initial_binding_access_refusal_cannot_become_a_live_selection() {
    let f = Fixture::new();
    let a = f.repo("a");
    let inventory = f.inventory(&[&a]);
    let selected = map(vec![entry(&inventory, 0, &[b"refs/heads/main"])]);
    let hooks = BindingRefusal(inventory.families[0].common_dir.clone());
    let result = enrich_with_target_input_and_controls(
        &inventory,
        TargetInput::Map(&selected),
        None,
        &Default::default(),
        &Default::default(),
        &hooks,
        &mut |_| {},
    );
    assert!(
        result.is_err(),
        "initial binding access failure was silently treated as a valid family"
    );
}

#[test]
fn simple_target_keeps_missing_family_for_unknown_review_but_explicit_map_refuses() {
    use wt_sweep::{
        model::{TARGET_MAP_SCHEMA, TargetMap, TargetMapFamily},
        review::targets::{FamilyTargets, TargetInput},
    };
    let fixture = Fixture::new();
    let repo = fixture.repo("principal");
    let inventory = fixture.inventory(&[&repo]);
    let family = &inventory.families[0];
    let targets = vec![BytePath::from_bytes(b"refs/heads/main")];
    let mapping = TargetMap {
        schema: TARGET_MAP_SCHEMA.into(),
        families: vec![TargetMapFamily {
            common_dir: family.common_dir.clone(),
            identity: family.identity.clone(),
            targets: targets.clone(),
        }],
    };
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&mapping)).is_ok());
    assert!(FamilyTargets::bind(&inventory, TargetInput::Simple(&targets)).is_ok());
    fs::rename(&repo, fixture.0.join("retained-principal")).unwrap();
    assert!(FamilyTargets::bind(&inventory, TargetInput::Map(&mapping)).is_err());
    let simple = FamilyTargets::bind(&inventory, TargetInput::Simple(&targets));
    assert!(
        simple.is_ok(),
        "simple target should defer unavailable-family facts to controls: {}",
        simple.err().map(|e| e.to_string()).unwrap_or_default()
    );
    let facts = wt_sweep::review::enrich(&inventory, &targets, None).unwrap();
    let report = wt_sweep::review::judge(&facts).unwrap();
    assert_eq!(
        report.items.iter().map(|row| &row.path).collect::<Vec<_>>(),
        family
            .worktrees
            .iter()
            .map(|tree| &tree.path)
            .collect::<Vec<_>>()
    );
    assert!(report.items.iter().all(|row| {
        row.classification != "cleanup_candidate"
            && !row.unknowns.is_empty()
            && row
                .decision
                .as_ref()
                .unwrap()
                .missing_conditions
                .contains(&"C1".into())
    }));
}
