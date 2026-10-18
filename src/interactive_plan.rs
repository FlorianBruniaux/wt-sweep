//! Terminal-only target selection. Suggestions never establish integration.
use std::{
    collections::BTreeSet,
    io::{BufRead, IsTerminal, Read, Write},
    path::Path,
    sync::mpsc::{self, RecvTimeoutError},
    time::Duration,
};
use wt_sweep::{
    cancel,
    git::{ControlledGit, GitRead},
    model::{BytePath, Family, Inventory, TARGET_MAP_SCHEMA, TargetMap, TargetMapFamily},
    review::targets::{FamilyTargets, TargetInput},
};

const PAGE_SIZE: usize = 20;
const INPUT_LIMIT: u64 = 4096;

pub fn require_terminal() -> Result<(), String> {
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err("--interactive requires a terminal on stdin and stderr; use --target or --target-map for unattended analysis".into());
    }
    Ok(())
}

pub fn select(inventory: &Inventory, input: &Path) -> Result<TargetMap, String> {
    require_terminal()?;
    FamilyTargets::bind(inventory, TargetInput::Simple(&[])).map_err(|e| e.to_string())?;
    if inventory.families.is_empty() {
        return Err("inventory contains no repository families; create a new scan".into());
    }
    // One CLI input thread keeps SIGINT responsive despite the process-wide
    // SA_RESTART handler. It cannot write, and exits with this CLI process if
    // cancellation leaves it waiting on the terminal. The channel has no queue.
    let (send, receive) = mpsc::sync_channel(0);
    std::thread::Builder::new()
        .name("target-prompt-input".into())
        .spawn(move || {
            let mut stdin = std::io::stdin().lock();
            loop {
                let line = read_answer(&mut stdin);
                let failed = line.is_err();
                if send.send(line).is_err() || failed {
                    break;
                }
            }
        })
        .map_err(|e| format!("cannot read terminal: {e}"))?;
    let answer = || loop {
        if cancel::requested() {
            return Err("analysis cancelled; no report created".into());
        }
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => return line,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => return Err("terminal input ended".into()),
        }
    };
    let mut out = std::io::stderr().lock();
    writeln!(out, "\nGUIDED ANALYSIS\nInventory: {}\n{} repository families; {} recorded worktrees.\nSaved disk measurements will not be refreshed.\nBranches below are local observations, not recommended cleanup decisions.\nRemote-tracking refs may be stale; no fetch is performed.\nChoose numbers or full refs, separated by spaces. 'more' shows the next page;\n'skip' leaves a family without integration targets; 'quit' cancels.\nNothing is selected by default.\n",
        BytePath::from_path(input).display(), inventory.families.len(),
        inventory.families.iter().map(|f| f.worktrees.len()).sum::<usize>()).map_err(io_error)?;
    let mut map = TargetMap {
        schema: TARGET_MAP_SCHEMA.into(),
        families: Vec::new(),
    };
    for (index, family) in inventory.families.iter().enumerate() {
        writeln!(
            out,
            "Repository {}/{}: {}\n  {} recorded worktrees",
            index + 1,
            inventory.families.len(),
            family.common_dir.display(),
            family.worktrees.len()
        )
        .map_err(io_error)?;
        let refs = match suggestions(inventory, family) {
            Ok(refs) => refs,
            Err(error) => {
                writeln!(
                    out,
                    "  Cannot propose targets: {}\n  Use skip or quit; a fresh scan may be needed.",
                    BytePath::from_bytes(error.as_bytes()).display()
                )
                .map_err(io_error)?;
                Vec::new()
            }
        };
        let mut page = 0;
        show_page(&mut out, &refs, page)?;
        loop {
            prompt(&mut out, "Targets> ")?;
            let line = answer()?;
            match line.as_str() {
                "quit" => return Err("analysis cancelled; no report created".into()),
                "skip" => break,
                "more" if !refs.is_empty() => {
                    page = if (page + 1) * PAGE_SIZE < refs.len() {
                        page + 1
                    } else {
                        0
                    };
                    show_page(&mut out, &refs, page)?;
                }
                _ => match choices(&line, &refs) {
                    Ok(targets) => {
                        let entry = binding(family, targets);
                        validate_entry(inventory, &entry)?;
                        map.families.push(entry);
                        break;
                    }
                    Err(error) => writeln!(out, "  {error}").map_err(io_error)?,
                },
            }
        }
    }
    if map.families.is_empty() {
        return Err("no targets selected; analysis cancelled, no report created".into());
    }
    writeln!(out, "\nCONFIRM TARGETS").map_err(io_error)?;
    for family in &inventory.families {
        let selected = map
            .families
            .iter()
            .find(|f| f.common_dir == family.common_dir);
        writeln!(out, "  {}", family.common_dir.display()).map_err(io_error)?;
        match selected {
            Some(entry) => {
                for target in &entry.targets {
                    writeln!(out, "    {}", target.display()).map_err(io_error)?;
                }
            }
            None => writeln!(
                out,
                "    Skipped: no integration target; rows remain in the report."
            )
            .map_err(io_error)?,
        }
    }
    prompt(&mut out, "Start read-only analysis? Type yes to confirm> ")?;
    if answer()? != "yes" {
        return Err("analysis not confirmed; no report created".into());
    }
    // The user may have spent minutes choosing. Ref disappearance or directory
    // replacement must not silently change their selection. The existing review
    // engine then resolves current OIDs and performs its own final checks.
    FamilyTargets::bind(inventory, TargetInput::Map(&map)).map_err(|e| e.to_string())?;
    for entry in &map.families {
        let common = entry.common_dir.to_path().map_err(|e| e.to_string())?;
        let current = read_refs(&common)?;
        for target in &entry.targets {
            if !current.contains(target) {
                return Err(format!(
                    "selected target {} disappeared; rerun --interactive",
                    target.display()
                ));
            }
            ControlledGit::default()
                .read(
                    &common,
                    GitRead::ResolveCommit {
                        reference: target.clone(),
                    },
                )
                .map_err(|e| {
                    format!(
                        "selected target {} cannot resolve to a commit: {e}",
                        target.display()
                    )
                })?;
        }
    }
    Ok(map)
}

fn binding(family: &Family, targets: Vec<BytePath>) -> TargetMapFamily {
    TargetMapFamily {
        common_dir: family.common_dir.clone(),
        identity: family.identity.clone(),
        targets,
    }
}

fn validate_entry(inventory: &Inventory, entry: &TargetMapFamily) -> Result<(), String> {
    let map = TargetMap {
        schema: TARGET_MAP_SCHEMA.into(),
        families: vec![entry.clone()],
    };
    FamilyTargets::bind(inventory, TargetInput::Map(&map))
        .map(|_| ())
        .map_err(|e| e.to_string())
}

fn suggestions(inventory: &Inventory, family: &Family) -> Result<Vec<BytePath>, String> {
    let entry = binding(family, Vec::new());
    validate_entry(inventory, &entry)?;
    let refs = read_refs(&family.common_dir.to_path().map_err(|e| e.to_string())?)?;
    validate_entry(inventory, &entry)?;
    Ok(refs)
}

fn read_refs(common: &Path) -> Result<Vec<BytePath>, String> {
    let bytes = ControlledGit::default()
        .read(common, GitRead::LocalRefs)
        .map_err(|e| e.to_string())?;
    let mut refs = BTreeSet::new();
    for line in bytes.split_inclusive(|b| *b == b'\n') {
        let fields: Vec<_> = line
            .strip_suffix(b"\n")
            .ok_or("unterminated branch listing")?
            .split(|b| *b == 0)
            .collect();
        if fields.len() != 3
            || !fields[2].is_empty()
            || !(fields[0].starts_with(b"refs/heads/") || fields[0].starts_with(b"refs/remotes/"))
            || ![40, 64].contains(&fields[1].len())
            || !fields[1].iter().all(u8::is_ascii_hexdigit)
            || fields[1].iter().all(|b| *b == b'0')
        {
            return Err("invalid branch listing; no targets proposed".into());
        }
        // origin/HEAD is an alias, not an integration branch selection.
        if fields[0].starts_with(b"refs/remotes/") && fields[0].ends_with(b"/HEAD") {
            continue;
        }
        refs.insert(BytePath::from_bytes(fields[0]));
    }
    let mut refs: Vec<_> = refs.into_iter().collect();
    // Conventional names appear first, but require the same explicit choice.
    refs.sort_by_key(|r| (rank(r), r.clone()));
    Ok(refs)
}

fn rank(reference: &BytePath) -> u8 {
    let bytes = reference.bytes().unwrap_or_default();
    if [
        b"refs/heads/main".as_slice(),
        b"refs/heads/develop",
        b"refs/heads/master",
        b"refs/heads/trunk",
    ]
    .contains(&bytes.as_slice())
    {
        0
    } else if bytes.starts_with(b"refs/heads/") {
        1
    } else {
        2
    }
}

fn show_page(out: &mut impl Write, refs: &[BytePath], page: usize) -> Result<(), String> {
    if refs.is_empty() {
        writeln!(out, "  No branches available; use skip or quit.").map_err(io_error)?;
    }
    for (index, reference) in refs
        .iter()
        .enumerate()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
    {
        writeln!(out, "  {}. {}", index + 1, reference.display()).map_err(io_error)?;
    }
    if refs.len() > PAGE_SIZE {
        writeln!(
            out,
            "  {} branches total; use more to page through the list.",
            refs.len()
        )
        .map_err(io_error)?;
    }
    Ok(())
}

fn choices(line: &str, refs: &[BytePath]) -> Result<Vec<BytePath>, String> {
    let mut selected = BTreeSet::new();
    for token in line.split_whitespace() {
        let reference = if let Ok(number) = token.parse::<usize>() {
            number.checked_sub(1).and_then(|i| refs.get(i)).cloned()
        } else {
            let exact = BytePath::from_bytes(token.as_bytes());
            refs.iter().find(|r| **r == exact).cloned()
        }
        .ok_or("Choose a displayed number or an existing full ref; use more, skip or quit.")?;
        selected.insert(reference);
        if selected.len() > 128 {
            return Err("Choose at most 128 distinct targets.".into());
        }
    }
    if selected.is_empty() {
        return Err("No default target. Choose explicitly, or type skip or quit.".into());
    }
    Ok(selected.into_iter().collect())
}

fn read_answer(input: &mut impl BufRead) -> Result<String, String> {
    let mut bytes = Vec::new();
    input
        .take(INPUT_LIMIT + 1)
        .read_until(b'\n', &mut bytes)
        .map_err(io_error)?;
    if bytes.is_empty() {
        return Err("terminal input ended; analysis cancelled".into());
    }
    if bytes.len() as u64 > INPUT_LIMIT {
        return Err("terminal answer exceeds 4096 bytes; analysis cancelled".into());
    }
    if !bytes.ends_with(b"\n") {
        return Err("incomplete terminal answer; analysis cancelled".into());
    }
    String::from_utf8(bytes)
        .map(|s| s.trim().to_owned())
        .map_err(|_| "type a number to select a non-UTF-8 ref".into())
}

fn prompt(out: &mut impl Write, text: &str) -> Result<(), String> {
    write!(out, "{text}")
        .and_then(|_| out.flush())
        .map_err(io_error)
}

fn io_error(error: std::io::Error) -> String {
    format!("terminal I/O failed: {error}")
}
