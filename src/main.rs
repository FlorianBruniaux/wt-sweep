mod report_access;
mod terminal;

#[cfg(test)]
mod report_output_tests;

use std::{collections::BTreeMap, ffi::OsString, io::Write, path::PathBuf, process::ExitCode};
use wt_sweep::{
    discover::{DiscoveryOptions, LocalFilesystem, scan_scope_with_progress},
    model::*,
    output::{PublicationLimits, bounded_json, canonical_digest, read_artifact, write_new},
    review::{ReviewOptions, enrich_with_options, judge, targets::TargetInput},
};

const HELP: &str = "Worktree Sweep (wt-sweep)

Usage:
  wt-sweep review (--repo PATH | --root PATH... | --all) [--workers 1..8] [--disk-usage] [--disk-budget standard|thorough] [PLAN OPTIONS]
  wt-sweep scan (--repo PATH | --root PATH... | --all | --resume FILE) [--workers 1..8] [--local] [--disk-usage] [--disk-budget standard|thorough] [--format json|table] [--html] [--open] [--out FILE | --run-dir BASE]
  wt-sweep plan --inventory FILE [--target FULL_REF... | --target-map FILE] [--filter-repo PATH] [--path-prefix PATH] [--branch FULL_REF] [--local-state clean|dirty|unknown] [--verdict keep|cleanup_candidate|verify] [--group-by repo|directory] [--changelog-path RELATIVE...] [--no-context] [--forge FILE] [--summary] [--format json|table] [--html] [--open] [--out FILE | --run-dir BASE]
  wt-sweep forge commandes
  wt-sweep forge ingerer --raw FILE [--receipt FILE] [--out FILE | --run-dir BASE]
  wt-sweep --help | --version

Read-only inventory and evidence review. Scans require explicit scope.
JSON preserves path bytes as hexadecimal; tables escape control bytes.
--out creates a new JSON file outside inspected targets and prints a table.
Use --target for one repository family, or --target-map for explicit targets per family.
Filters select reviewed rows; JSON views retain the full source and its coverage.
Global discovery reports gaps and does not establish a machine-wide snapshot.
--checkpoint-out NEW_FILE saves explicit-root discovery; --resume FILE continues it.\n--workers bounds parallel metadata reads; aggregation order stays deterministic.
--disk-usage measures file sizes and allocated blocks without reading contents (scan and review).
--disk-root ABSOLUTE_PATH limits disk collection to an exact registered root (repeatable).
--local hashes local files and index within explicit budgets; gaps remain visible.
review and plan report readable progress on stderr and keep JSON on stdout.
--summary prints a compact recap; --verbose adds detailed progress and metrics.
--color auto|always|never controls terminal color and icons (default: auto).
Auto respects NO_COLOR and TERM=dumb; JSON and saved reports stay plain.
--worktree PATH focuses evidence collection; unselected rows remain explicitly incomplete.
--run-dir creates an isolated run below an existing directory outside inspected targets.
--html saves an offline report.html; default on macOS: ~/Desktop/Worktree Sweep/.
--run-dir BASE overrides that destination; BASE must exist outside inspected targets.
--open implies --html and asks macOS to open the saved report; no browser is opened otherwise.
If Desktop is missing or blocked, choose --run-dir BASE. Reports are never overwritten.
--compare-local compares local changes against explicit targets; requires --run-dir or --html.\n--compare-reference ABSOLUTE_PATH adds a same-family checkout; requires --compare-local.\napply is unavailable until conservation and approval gates are verified.
";

type Options = BTreeMap<String, Vec<OsString>>;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args
        .first()
        .is_some_and(|arg| arg == "scan" || arg == "plan" || arg == "review")
        && let Err(error) = wt_sweep::cancel::install()
    {
        eprintln!("wt-sweep: cannot install interruption handler: {error}");
        return ExitCode::from(2);
    }
    match run(args) {
        Ok(()) if wt_sweep::cancel::requested() => ExitCode::from(130),
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!(
                "wt-sweep: {}",
                BytePath::from_bytes(message.as_bytes()).display()
            );
            ExitCode::from(if wt_sweep::cancel::requested() {
                130
            } else {
                2
            })
        }
    }
}

fn options(args: &[OsString], allowed: &[&str], repeat: &[&str]) -> Result<Options, String> {
    let mut values = BTreeMap::new();
    let mut index = 0;
    while index < args.len() {
        let arg = args[index].to_str().ok_or("option must be UTF-8")?;
        if !allowed.contains(&arg) {
            return Err(format!("unsupported option {arg}"));
        }
        if values.contains_key(arg) && !repeat.contains(&arg) {
            return Err(format!("repeated option {arg}"));
        }
        index += 1;
        let value = if matches!(
            arg,
            "--all"
                | "--summary"
                | "--local"
                | "--no-context"
                | "--disk-usage"
                | "--verbose"
                | "--html"
                | "--open"
                | "--compare-local"
        ) {
            OsString::from("true")
        } else {
            let value = args
                .get(index)
                .ok_or_else(|| format!("missing value for {arg}"))?
                .clone();
            index += 1;
            value
        };
        if arg == "--format" && value != "json" && value != "table" {
            return Err("format must be json or table".into());
        }
        if arg == "--color" && value != "auto" && value != "always" && value != "never" {
            return Err("--color must be auto, always or never".into());
        }
        values
            .entry(arg.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }
    if values.contains_key("--out") && values.contains_key("--run-dir") {
        return Err("--out and --run-dir are mutually exclusive".into());
    }
    if values.contains_key("--open") {
        if !cfg!(target_os = "macos") {
            return Err("--open is supported on macOS only; use --html --run-dir BASE".into());
        }
        values.insert("--html".into(), vec![OsString::from("true")]);
    }
    if values.contains_key("--html") {
        if values.contains_key("--out") {
            return Err("--html and --open are incompatible with --out; use --run-dir BASE".into());
        }
        if !values.contains_key("--run-dir") {
            report_access::default_base()?;
        }
    }
    Ok(values)
}

fn one(options: &Options, name: &str) -> Option<PathBuf> {
    options.get(name).and_then(|v| v.first()).map(PathBuf::from)
}

fn run(args: Vec<OsString>) -> Result<(), String> {
    match args.as_slice() {
        [] => return print(HELP.as_bytes()),
        [arg] if arg == "--help" || arg == "-h" => return print(HELP.as_bytes()),
        [arg] if arg == "--version" || arg == "-V" => {
            return print(format!("wt-sweep {}\n", env!("CARGO_PKG_VERSION")).as_bytes());
        }
        [command, flag] if flag == "--help" || flag == "-h" => {
            return command_help(command.to_str().unwrap_or(""));
        }
        [command, sub, flag] if command == "forge" && (flag == "--help" || flag == "-h") => {
            return command_help(&format!("forge {}", sub.to_str().unwrap_or("")));
        }
        _ => {}
    }
    match args.first().and_then(|arg| arg.to_str()) {
        Some("apply") => {
            Err("apply is unavailable: conservation and approval gates are not verified".into())
        }
        Some("scan") => scan(&args[1..]),
        Some("plan") => plan(&args[1..]),
        Some("review") => review(&args[1..]),
        Some("forge") => forge(&args[1..]),
        _ => Err("unsupported arguments; use wt-sweep --help".into()),
    }
}

fn command_help(command: &str) -> Result<(), String> {
    let usage = match command {
        "scan" => {
            "wt-sweep scan (--repo PATH | --root PATH... | --all | --resume FILE) [--workers 1..8] [--local] [--disk-usage] [--disk-budget standard|thorough] [--format json|table] [--html] [--open] [--out FILE | --run-dir BASE]"
        }
        "plan" => "wt-sweep plan --inventory FILE [REVIEW OPTIONS]",
        "review" => {
            "wt-sweep review (--repo PATH | --root PATH... | --all) [--workers 1..8] [--disk-usage] [--disk-budget standard|thorough] [REVIEW OPTIONS]"
        }
        "forge" => {
            "wt-sweep forge commandes | ingerer --raw FILE [--receipt FILE] [--out FILE | --run-dir BASE]"
        }
        "forge commandes" => "wt-sweep forge commandes",
        "forge ingerer" => {
            "wt-sweep forge ingerer --raw FILE [--receipt FILE] [--out FILE | --run-dir BASE]"
        }
        _ => return Err("unsupported command; use wt-sweep --help".into()),
    };
    let notes = match command {
        "scan" => {
            "Discovery requires an explicit scope.\n--checkpoint-out NEW_FILE saves traversal state for --root scopes.\n--resume FILE --checkpoint-out NEW_FILE resumes the saved exact roots; no old Git HEAD is reused.\n--discovery-directories 1..100000, --discovery-entries 1..1000000, --discovery-seconds 1..600 bound each pass.\nDefaults: 100000 directories, 1000000 entries, 60 seconds. Intervals remain separate; this is not a snapshot.\n--disk-usage measures registered roots, including ignored files, without hashing.\nDisk budgets: 30s/500,000 entries per root; 600s/2,000,000 entries overall; depth 128.\n--disk-budget thorough: 120s/1,000,000 entries per root; 1800s/10,000,000 overall.\n--disk-root ABSOLUTE_PATH (repeatable) selects exact registered roots; requires --disk-usage.\nRemaining total entries and time are shared among roots still to visit.\nAllocated blocks are not a reclaim estimate; partial measurements remain explicit.\n--local additionally hashes local content within budgets.\n--html saves report.html; default on macOS: ~/Desktop/Worktree Sweep/.\n--run-dir BASE overrides it and must be an existing directory outside inspected targets.\n--open implies --html and asks macOS to open the report after successful publication.\nIf Desktop is unavailable, choose --run-dir BASE. Browser failures leave the report saved.\nProgress is compact on stderr. --verbose adds paths and discovery counters.\n--color auto|always|never controls terminal color and icons.\nReports retain discovery metrics and coverage separately. Gaps and unfinished scope remain in the inventory.\n"
        }
        "plan" | "review" => {
            "REVIEW OPTIONS:\n  --budget standard|thorough (default: standard; larger finite collection allowances)\n  --target FULL_REF... | --target-map FILE\n  --compare-local [--compare-reference ABSOLUTE_PATH] (requires --run-dir or --html)\n  --worktree ABSOLUTE_PATH (collect evidence for this exact recorded worktree)\n  --filter-repo PATH  --path-prefix PATH  --branch FULL_REF\n  --local-state clean|dirty|unknown  --verdict keep|cleanup_candidate|verify\n  --group-by repo|directory  --forge FILE\n  --changelog-path RELATIVE... | --no-context (skip commit, changelog and remote-link context)\n  --summary  --format json|table  --out FILE | --run-dir BASE\n  --html (offline report.html; macOS default: ~/Desktop/Worktree Sweep/)\n  --open (implies --html; ask macOS to open the saved report)\n\nTargets must be explicit; no target leaves integration unknown.\nFilters select rows after judgement and retain source coverage in JSON views.\n--worktree narrows history/local/private/context reads; full inventory and identity checks remain.\nOther rows explicitly retain missing evidence. Add --path-prefix for a focused display.\n--summary cannot be combined with --format json. Progress is compact on stderr.\n--verbose adds detailed progress and recap metrics.\n--color auto|always|never (default: auto; respects NO_COLOR and TERM=dumb).\nJSON and saved report.txt files are always plain text.\n--run-dir needs an existing directory outside inspected targets; it overrides the Desktop default.\nWithout --html/--open, output defaults are unchanged. If Desktop is unavailable, choose --run-dir.\nOpening failure leaves the saved report available and prints a reopen command.\nreview accepts --disk-usage [--disk-budget standard|thorough] for metadata-only sizes.\nDisk measurement covers all inventoried roots by default. --disk-root ABSOLUTE_PATH (repeatable) selects exact roots.\n--disk-root requires --disk-usage and is independent of --worktree and display filters.\nRemaining total entries and time are shared among roots still to visit; unused shares go forward.\nIts budget is separate from --budget; defaults: 30s/root, 600s overall; thorough: 120s/root, 1800s overall.\nGitHub/GitLab repository, branch and commit links use local remotes (no fetch or server check).\nPR/MR links and captured states require --forge FILE and an exact source association.\nSelf-hosted forge routes need an associated import to identify the provider.\nscan does not collect remote-link context; use review or plan.\nreview saves inventory.json, review.json, report.txt and run.json together.\nplan reuses saved disk observations with their original interval; it does not recount sizes.\nplan saves review.json, report.txt and run.json.\nA cleanup_candidate still requires confirmation of its use. apply is unavailable.\n"
        }
        _ => {
            "Offline evidence only. No network command is executed by these subcommands.\ncommandes prints capture guidance; ingerer reads captured raw data and its receipt.\n"
        }
    };
    print(format!("Usage:\n  {usage}\n\nRead-only.\n{notes}").as_bytes())
}

fn scan(args: &[OsString]) -> Result<(), String> {
    let opts = options(
        args,
        &[
            "--repo",
            "--root",
            "--all",
            "--out",
            "--format",
            "--workers",
            "--checkpoint-out",
            "--resume",
            "--discovery-directories",
            "--discovery-entries",
            "--discovery-seconds",
            "--local",
            "--html",
            "--open",
            "--disk-usage",
            "--disk-budget",
            "--disk-root",
            "--verbose",
            "--color",
            "--run-dir",
        ],
        &["--root", "--disk-root"],
    )?;
    let disk_limits = disk_settings(&opts)?;
    let mut progress = terminal::Progress::new(&opts);
    let (mut inventory, checkpoint) = collect_inventory_with_checkpoint(&opts, &mut progress)?;
    collect_disk(&mut inventory, disk_limits.as_ref(), &mut progress)?;
    let publication_limits = PublicationLimits::default();
    if opts.contains_key("--local") {
        collect_local(&mut inventory, &publication_limits);
    }
    let mut prepared = wt_sweep::output::prepare_inventory(&inventory, &publication_limits)
        .map_err(|e| e.to_string())?;
    let html = opts
        .contains_key("--html")
        .then(|| {
            wt_sweep::presentation::html::inventory(
                &prepared.inventory,
                publication_limits.report_bytes,
            )
        })
        .transpose()
        .map_err(|e| e.to_string())?;
    let boundaries = if opts.contains_key("--out")
        || opts.contains_key("--run-dir")
        || opts.contains_key("--html")
        || checkpoint.is_some()
    {
        wt_sweep::output::revalidate_boundaries(&inventory).map_err(|e| e.to_string())?
    } else {
        inventory
    };
    if let Some(bytes) = checkpoint {
        let path = std::path::absolute(
            one(&opts, "--checkpoint-out").ok_or("missing checkpoint destination")?,
        )
        .map_err(|e| e.to_string())?;
        write_new(&path, &bytes, &boundaries).map_err(|e| e.to_string())?;
        prepared.report.push_str(&format!(
            "\nDiscovery checkpoint: {}\n",
            BytePath::from_path(&path).display()
        ));
        if !boundaries.coverage.traversal_complete {
            if let Some(path_text) = path.to_str().filter(|p| !p.chars().any(char::is_control)) {
                let quote = |value: &str| format!("'{}'", value.replace('\'', "'\"'\"'"));
                prepared.report.push_str(&format!("Continue discovery (new observation interval):\n  wt-sweep scan --resume {} --checkpoint-out {}\n", quote(path_text), quote(&format!("{path_text}.next"))));
            } else {
                prepared.report.push_str("Continue with scan --resume CHECKPOINT --checkpoint-out NEW_FILE, preserving the checkpoint path bytes.\n");
            }
        }
    }
    progress.finish();
    deliver(
        &opts,
        prepared.json,
        prepared.report,
        &boundaries,
        RunInfo {
            command: "scan",
            name: "inventory.json",
            partial: prepared.partial,
            detail: None,
            inventory_json: None,
            html: html.as_deref(),
            comparison: None,
        },
    )
}

struct DiskSettings {
    limits: wt_sweep::disk::DiskLimits,
    paths: Vec<BytePath>,
}

fn disk_settings(opts: &Options) -> Result<Option<DiskSettings>, String> {
    let paths: Vec<_> = opts
        .get("--disk-root")
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .collect();
    if !paths.is_empty() && !opts.contains_key("--disk-usage") {
        return Err("--disk-root requires --disk-usage".into());
    }
    if paths.iter().any(|p| !p.is_absolute()) {
        return Err("--disk-root requires an absolute registered path".into());
    }
    let limits = match opts.get("--disk-budget").and_then(|values| values.first()) {
        Some(_) if !opts.contains_key("--disk-usage") => {
            return Err("--disk-budget requires --disk-usage".into());
        }
        Some(value) if value == "standard" => wt_sweep::disk::DiskLimits::default(),
        Some(value) if value == "thorough" => wt_sweep::disk::DiskLimits::thorough(),
        Some(_) => return Err("--disk-budget must be standard or thorough".into()),
        None => wt_sweep::disk::DiskLimits::default(),
    };
    Ok(opts.contains_key("--disk-usage").then_some(DiskSettings {
        limits,
        paths: paths.iter().map(|p| BytePath::from_path(p)).collect(),
    }))
}

fn collect_disk(
    inventory: &mut Inventory,
    settings: Option<&DiskSettings>,
    progress: &mut terminal::Progress,
) -> Result<(), String> {
    if let Some(settings) = settings {
        progress.update(
            "Measuring registered worktrees and shared Git directories...",
            true,
        );
        let mut incomplete = 0;
        wt_sweep::disk::collect_selected(inventory, &settings.limits, &settings.paths, &mut |event| {
            if let Some(row) = event.completed {
                incomplete += usize::from(!row.complete);
                if progress.verbose() {
                    let size = row.bytes.as_ref().map(|b| wt_sweep::disk::human_bytes(b.allocated)).unwrap_or_else(|| "unknown".into());
                    progress.update(&format!("  {}: {size} ({})", row.path.display(), if row.complete { "complete" } else { "incomplete" }), false);
                }
            }
            let visible = event.completed.is_none();
            progress.update(&format!("Disk usage: {}/{} roots visited; {} entries; {:.1}s; {incomplete} incomplete", event.roots_completed, event.roots_requested, event.entries_checked, event.elapsed_ms as f64 / 1000.0), visible);
        }).map_err(|e| e.to_string())?;
        if let Some(usage) = &inventory.disk_usage {
            progress.update(
                &format!(
                    "Disk measurement finished: {}; {incomplete} incomplete",
                    usage.scope_summary()
                ),
                true,
            );
        }
    }
    Ok(())
}

fn collect_inventory(
    opts: &Options,
    progress: &mut terminal::Progress,
) -> Result<Inventory, String> {
    collect_inventory_with_checkpoint(opts, progress).map(|(inventory, _)| inventory)
}

fn collect_inventory_with_checkpoint(
    opts: &Options,
    progress: &mut terminal::Progress,
) -> Result<(Inventory, Option<Vec<u8>>), String> {
    use wt_sweep::discover::resume::{
        DiscoveryCheckpoint, MAX_CHECKPOINT_BYTES, scan_scope_resumable_with_progress,
    };
    let resuming = opts.contains_key("--resume");
    let checkpointing = opts.contains_key("--checkpoint-out");
    if resuming && !checkpointing {
        return Err("--resume requires --checkpoint-out NEW_FILE".into());
    }
    if checkpointing && (opts.contains_key("--repo") || opts.contains_key("--all")) {
        return Err("discovery checkpoints require explicit --root scope".into());
    }
    let checkpoint: Option<DiscoveryCheckpoint> = one(opts, "--resume")
        .map(|path| wt_sweep::output::read_artifact_bounded(&path, MAX_CHECKPOINT_BYTES as u64))
        .transpose()
        .map_err(|e| e.to_string())?;
    if let Some(saved) = &checkpoint {
        saved.to_bytes().map_err(|e| e.to_string())?;
    }
    let modes = ["--repo", "--root", "--all"]
        .iter()
        .filter(|name| opts.contains_key(**name))
        .count();
    if modes != 1 && !(resuming && modes == 0) {
        return Err(
            "scan/review requires exactly one explicit scope: --repo, --root or --all".into(),
        );
    }
    let scope = if let Some(repo) = one(opts, "--repo") {
        ScanScope::Repo {
            path: BytePath::from_path(&std::path::absolute(&repo).map_err(|e| e.to_string())?),
        }
    } else if let Some(roots) = opts.get("--root") {
        ScanScope::Roots {
            paths: roots
                .iter()
                .map(|r| {
                    std::path::absolute(PathBuf::from(r)).map(|path| BytePath::from_path(&path))
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| e.to_string())?,
        }
    } else if let Some(saved) = &checkpoint {
        saved.scope().clone()
    } else {
        ScanScope::AllMounted
    };
    let mut limits = DiscoveryOptions::default();
    if let Some(value) = opts.get("--workers").and_then(|values| values.first()) {
        limits.workers = value
            .to_str()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|workers| (1..=8).contains(workers))
            .ok_or("workers must be an integer between 1 and 8")?;
    }
    for (name, maximum, slot) in [
        (
            "--discovery-directories",
            100_000,
            &mut limits.max_directories,
        ),
        ("--discovery-entries", 1_000_000, &mut limits.max_entries),
    ] {
        if let Some(value) = opts.get(name).and_then(|v| v.first()) {
            *slot = value
                .to_str()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| (1..=maximum).contains(v))
                .ok_or_else(|| format!("{name} must be between 1 and {maximum}"))?;
        }
    }
    if let Some(value) = opts.get("--discovery-seconds").and_then(|v| v.first()) {
        let seconds = value
            .to_str()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| (1..=600).contains(v))
            .ok_or("--discovery-seconds must be between 1 and 600")?;
        limits.timeout = std::time::Duration::from_secs(seconds);
    }
    let mut notify = |event: &wt_sweep::discover::DiscoveryProgress| {
        progress.update(
            &wt_sweep::presentation::discovery_progress(event),
            matches!(
                event.phase,
                wt_sweep::discover::DiscoveryPhase::Start
                    | wt_sweep::discover::DiscoveryPhase::Finished
            ),
        );
    };
    if checkpointing {
        let batch = scan_scope_resumable_with_progress(
            &scope,
            &limits,
            &LocalFilesystem,
            checkpoint.as_ref(),
            &mut notify,
        )
        .map_err(|e| e.to_string())?;
        let bytes = batch.checkpoint.to_bytes().map_err(|e| e.to_string())?;
        Ok((batch.inventory, Some(bytes)))
    } else {
        scan_scope_with_progress(&scope, &limits, &LocalFilesystem, &mut notify)
            .map(|inventory| (inventory, None))
            .map_err(|e| e.to_string())
    }
}

fn collect_local(inventory: &mut Inventory, publication_limits: &PublicationLimits) {
    use wt_sweep::local::{LocalLimits, collect};
    let started = std::time::Instant::now();
    let mut entries_left = 25_000;
    let mut bytes_left = 256 * 1024 * 1024;
    let base_bytes = wt_sweep::output::bounded_json_size(inventory, publication_limits.json_bytes)
        .unwrap_or(publication_limits.json_bytes);
    let mut manifest_left = (24 * 1024 * 1024).min(
        publication_limits
            .json_bytes
            .saturating_sub(base_bytes)
            .saturating_sub(publication_limits.metadata_reserve_bytes),
    );
    let total_worktrees = inventory
        .families
        .iter()
        .map(|family| family.worktrees.len())
        .sum::<usize>();
    for (ordinal, worktree) in inventory
        .families
        .iter()
        .flat_map(|family| &family.worktrees)
        .enumerate()
    {
        let remaining = std::time::Duration::from_secs(120).saturating_sub(started.elapsed());
        if entries_left == 0
            || bytes_left == 0
            || manifest_left < 4096
            || remaining.is_zero()
            || wt_sweep::cancel::requested()
        {
            let omitted = total_worktrees.saturating_sub(ordinal);
            let reason = if wt_sweep::cancel::requested() {
                "interrupted"
            } else {
                "whole-scan local inventory budget exhausted"
            };
            let message = format!(
                "{reason}; {omitted} worktrees not collected; scope includes remaining discovered worktrees and registered worktrees outside requested roots"
            );
            inventory.coverage.gaps.push(Gap {
                path: worktree.path.clone(),
                reason: message.clone(),
            });
            inventory.not_collected.push(message);
            inventory.coverage.traversal_complete = false;
            break;
        }
        let defaults = LocalLimits::default();
        let limits = LocalLimits {
            max_entries: defaults.max_entries.min(entries_left),
            max_total_bytes: defaults.max_total_bytes.min(bytes_left),
            max_manifest_bytes: defaults.max_manifest_bytes.min(manifest_left),
            timeout: defaults.timeout.min(remaining),
            ..defaults
        };
        match collect(worktree, &limits) {
            Ok(local) => {
                let size = serde_json::to_vec(&local)
                    .map(|bytes| bytes.len())
                    .unwrap_or(usize::MAX);
                if size > manifest_left {
                    manifest_left = 0;
                    inventory.coverage.gaps.push(Gap { path: worktree.path.clone(), reason: "local manifest serialization budget exceeded; observations not published".into() });
                    inventory.coverage.traversal_complete = false;
                    continue;
                }
                manifest_left -= size;
                entries_left = entries_left.saturating_sub(
                    local.entries.len()
                        + local.index_entries.len()
                        + local.administration_entries.len(),
                );
                for entry in local
                    .entries
                    .iter()
                    .chain(&local.administration_entries)
                    .filter(|entry| entry.content_sha256.is_some())
                {
                    bytes_left = bytes_left.saturating_sub(entry.size);
                }
                if !local.traversal_complete || !local.gaps.is_empty() {
                    inventory.coverage.traversal_complete = false;
                    inventory.coverage.gaps.push(Gap {
                        path: worktree.path.clone(),
                        reason: "local inventory is incomplete; see local_inventories gaps".into(),
                    });
                }
                inventory.local_inventories.push(local);
            }
            Err(error) => {
                inventory.coverage.traversal_complete = false;
                inventory.coverage.gaps.push(Gap {
                    path: worktree.path.clone(),
                    reason: format!("local inventory: {error}"),
                });
            }
        }
    }
    inventory
        .not_collected
        .retain(|item| item != "local file contents, ignored files, index and reflog inventory");
    inventory.not_collected.push("local manifests do not establish complete ACL/xattr capture, shared reflog/object preservation or a filesystem snapshot; see per-worktree gaps and not_collected".into());
    inventory.finish();
}

const PLAN_OPTIONS: &[&str] = &[
    "--compare-local",
    "--compare-reference",
    "--html",
    "--open",
    "--verbose",
    "--color",
    "--budget",
    "--worktree",
    "--target",
    "--target-map",
    "--changelog-path",
    "--no-context",
    "--filter-repo",
    "--path-prefix",
    "--branch",
    "--local-state",
    "--verdict",
    "--group-by",
    "--forge",
    "--out",
    "--format",
    "--summary",
    "--run-dir",
];

fn plan(args: &[OsString]) -> Result<(), String> {
    let mut allowed = PLAN_OPTIONS.to_vec();
    allowed.push("--inventory");
    let opts = options(args, &allowed, &["--target", "--changelog-path"])?;
    let settings = review_settings(&opts)?;
    let input = one(&opts, "--inventory").ok_or("plan requires --inventory FILE")?;
    let value: serde_json::Value = read_artifact(&input).map_err(|e| e.to_string())?;
    if value.get("schema").and_then(serde_json::Value::as_str) == Some("wt-sweep.review/1") {
        return Err("this is a historical review/1 archive, not an inventory; it provides no current C5 verification. Use wt-sweep review with an explicit scope, or create a fresh scan inventory for plan".into());
    }
    let inventory: Inventory = serde_json::from_value(value).map_err(|e| e.to_string())?;
    let mut progress = terminal::Progress::new(&opts);
    produce_review(&opts, inventory, settings, false, &mut progress)
}

fn review(args: &[OsString]) -> Result<(), String> {
    let mut allowed = PLAN_OPTIONS.to_vec();
    allowed.extend([
        "--repo",
        "--root",
        "--all",
        "--workers",
        "--disk-usage",
        "--disk-budget",
        "--disk-root",
    ]);
    let opts = options(
        args,
        &allowed,
        &["--root", "--disk-root", "--target", "--changelog-path"],
    )?;
    let settings = review_settings(&opts)?;
    let disk_limits = disk_settings(&opts)?;
    let mut progress = terminal::Progress::new(&opts);
    let mut inventory = collect_inventory(&opts, &mut progress)?;
    collect_disk(&mut inventory, disk_limits.as_ref(), &mut progress)?;
    produce_review(&opts, inventory, settings, true, &mut progress)
}

struct ReviewSettings {
    filters: ReviewFilters,
    group_by: String,
    use_view: bool,
    target_map: Option<TargetMap>,
    forge: Option<ForgeExport>,
    targets: Vec<BytePath>,
    review_options: ReviewOptions,
}

fn review_settings(opts: &Options) -> Result<ReviewSettings, String> {
    if opts.contains_key("--summary") && opts.get("--format").is_some_and(|v| v[0] == "json") {
        return Err("--summary is incompatible with --format json".into());
    }
    if opts.contains_key("--target") && opts.contains_key("--target-map") {
        return Err("--target and --target-map are mutually exclusive".into());
    }
    let value = |name: &str| -> Result<Option<String>, String> {
        opts.get(name)
            .map(|v| {
                v[0].to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("{name} must be UTF-8"))
            })
            .transpose()
    };
    let filters = ReviewFilters {
        repository: one(opts, "--filter-repo").map(|p| BytePath::from_path(&p)),
        path_prefix: one(opts, "--path-prefix").map(|p| BytePath::from_path(&p)),
        branch: one(opts, "--branch").map(|p| BytePath::from_path(&p)),
        local_state: value("--local-state")?,
        verdict: value("--verdict")?,
    };
    let group_by = value("--group-by")?.unwrap_or_else(|| "repo".into());
    wt_sweep::view::validate_filters(&filters, &group_by).map_err(|e| e.to_string())?;
    let use_view = [
        "--filter-repo",
        "--path-prefix",
        "--branch",
        "--local-state",
        "--verdict",
        "--group-by",
    ]
    .iter()
    .any(|name| opts.contains_key(*name));
    let target_map: Option<TargetMap> = one(opts, "--target-map")
        .map(|p| read_artifact(&p))
        .transpose()
        .map_err(|e| e.to_string())?;
    let forge: Option<ForgeExport> = one(opts, "--forge")
        .map(|p| read_artifact(&p))
        .transpose()
        .map_err(|e| e.to_string())?;
    let targets: Vec<_> = opts
        .get("--target")
        .into_iter()
        .flatten()
        .map(|t| BytePath::from_path(&PathBuf::from(t)))
        .collect();
    if opts.contains_key("--no-context") && opts.contains_key("--changelog-path") {
        return Err("--no-context and --changelog-path are mutually exclusive".into());
    }
    if opts.contains_key("--compare-reference") && !opts.contains_key("--compare-local") {
        return Err("--compare-reference requires --compare-local".into());
    }
    if opts.contains_key("--compare-local")
        && (opts.contains_key("--out")
            || (!opts.contains_key("--run-dir") && !opts.contains_key("--html")))
    {
        return Err("--compare-local requires --run-dir BASE or --html, without --out".into());
    }
    let mut review_options = ReviewOptions {
        worktree: one(opts, "--worktree").map(|p| BytePath::from_path(&p)),
        ..ReviewOptions::with_budget(value("--budget")?.as_deref().unwrap_or("standard"))
            .map_err(|e| e.to_string())?
    };
    if opts.contains_key("--compare-local") {
        review_options.comparison = Some(Default::default());
    }
    review_options.context.enabled = !opts.contains_key("--no-context");
    if let Some(paths) = opts.get("--changelog-path") {
        review_options.context.changelog_paths = paths
            .iter()
            .map(|path| BytePath::from_path(&PathBuf::from(path)))
            .collect();
    }
    Ok(ReviewSettings {
        filters,
        group_by,
        use_view,
        target_map,
        forge,
        targets,
        review_options,
    })
}

fn first_collected_observation(
    observation: &Option<wt_sweep::review::ProgressObservation>,
    observed_history: &mut bool,
    observed_local: &mut bool,
) -> bool {
    use wt_sweep::review::ProgressObservation;
    // Pending events remain available in verbose and live progress. A pipe gets
    // its one milestone only after a relationship or numeric count was collected.
    match observation {
        Some(ProgressObservation::History {
            head,
            target_oid,
            result,
            complete,
            ..
        }) if *complete
            && head.is_some()
            && target_oid.is_some()
            && matches!(
                result.as_str(),
                "identical_commit" | "ancestor" | "not_ancestor"
            ) =>
        {
            !std::mem::replace(observed_history, true)
        }
        Some(ProgressObservation::Local { summary, .. })
            if [
                &summary.staged,
                &summary.unstaged,
                &summary.conflicts,
                &summary.untracked,
                &summary.ignored,
            ]
            .iter()
            .any(|count| count.complete || count.observed > 0) =>
        {
            !std::mem::replace(observed_local, true)
        }
        _ => false,
    }
}

fn produce_review(
    opts: &Options,
    inventory: Inventory,
    settings: ReviewSettings,
    combined: bool,
    progress: &mut terminal::Progress,
) -> Result<(), String> {
    let ReviewSettings {
        filters,
        group_by,
        use_view,
        target_map,
        forge,
        targets,
        review_options,
    } = settings;
    // Save exactly the input bound by the review digest. Never compact it after judgement.
    let source_json = (combined || opts.contains_key("--compare-local"))
        .then(|| bounded_json(&inventory, PublicationLimits::default().json_bytes))
        .transpose()
        .map_err(|e| format!("review input cannot be published; select a narrower scope: {e}"))?;
    let target_input = target_map
        .as_ref()
        .map(TargetInput::Map)
        .unwrap_or(TargetInput::Simple(&targets));
    let mut observed_history = false;
    let mut observed_local = false;
    let facts = enrich_with_options(
        &inventory,
        target_input,
        forge.as_ref(),
        &review_options,
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            let first_observation = first_collected_observation(
                &event.observation,
                &mut observed_history,
                &mut observed_local,
            );
            progress.update(
                &wt_sweep::presentation::review_progress(&event),
                first_observation || matches!(event.phase, "controls" | "complete"),
            );
        },
    )
    .map_err(|e| e.to_string())?;
    let review = judge(&facts).map_err(|e| e.to_string())?;
    if canonical_digest(&review).map_err(|e| e.to_string())? != review.digest_sha256 {
        return Err("review digest mismatch".into());
    }
    let comparison = if let Some(store) = &review_options.comparison {
        let bound = wt_sweep::review::targets::FamilyTargets::bind(&inventory, target_input)
            .map_err(|e| e.to_string())?;
        let reference = one(opts, "--compare-reference").map(|p| BytePath::from_path(&p));
        Some(
            wt_sweep::local::comparison::collect(
                &inventory,
                &review,
                &bound,
                store,
                reference.as_ref(),
                review_options.worktree.as_ref(),
                if review_options.budget_profile == "thorough" {
                    32768
                } else {
                    4096
                },
            )
            .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let comparison_json = comparison
        .as_ref()
        .map(|doc| bounded_json(doc, wt_sweep::local::comparison::DOCUMENT_BYTES))
        .transpose()
        .map_err(|e| e.to_string())?;
    let view = (use_view || opts.contains_key("--html"))
        .then(|| wt_sweep::view::build(&inventory, &review, filters, &group_by))
        .transpose()
        .map_err(|e| e.to_string())?;
    let publication_limits = PublicationLimits::default();
    let mut html = if opts.contains_key("--html") {
        let view = view
            .as_ref()
            .ok_or("HTML review requires family bindings")?;
        Some(
            wt_sweep::presentation::html::review(view, publication_limits.report_bytes)
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let view = if use_view { view } else { None };
    let mut detail = if let Some(view) = &view {
        wt_sweep::presentation::view_detail_bounded(view, publication_limits.report_bytes)
    } else {
        wt_sweep::presentation::detail_bounded(&review, publication_limits.report_bytes)
    }
    .map_err(|e| e.to_string())?;
    if let Some(comparison) = &comparison {
        detail.push_str(
            &comparison
                .text_bounded(publication_limits.report_bytes.saturating_sub(detail.len()))
                .map_err(|e| e.to_string())?,
        );
        if let Some(html) = &mut html {
            let navigation = "<div class=\"nav-item\"><a href=\"#local-comparison\"><span aria-hidden=\"true\">↔ </span>Comparer les fichiers</a></div>";
            let nav_at = html
                .find("</nav>")
                .ok_or("HTML report has no navigation boundary")?;
            html.insert_str(nav_at, navigation);
            let at = html
                .rfind("</main>")
                .ok_or("HTML report has no main boundary")?;
            html.insert_str(
                at,
                &comparison
                    .html_bounded(publication_limits.report_bytes.saturating_sub(html.len()))
                    .map_err(|e| e.to_string())?,
            );
            if html.len() > publication_limits.report_bytes {
                return Err("HTML comparison exceeds publication byte limit".into());
            }
        }
    }
    let executable = std::env::current_exe()
        .ok()
        .map(|p| BytePath::from_path(&p))
        .unwrap_or_else(|| BytePath::from_bytes(b"wt-sweep"));
    let mut table = if opts.contains_key("--summary") {
        match (&view, opts.contains_key("--verbose")) {
            (Some(view), true) => wt_sweep::presentation::view_summary(view),
            (None, true) => wt_sweep::presentation::summary(&review),
            (Some(view), false) => {
                wt_sweep::presentation::compact_view_summary_with_executable(view, &executable)
            }
            (None, false) => {
                wt_sweep::presentation::compact_summary_with_executable(&review, &executable)
            }
        }
    } else {
        detail.clone()
    };
    if opts.contains_key("--summary")
        && let Some(comparison) = &comparison
    {
        table.push_str(
            &comparison
                .text_bounded(publication_limits.report_bytes.saturating_sub(table.len()))
                .map_err(|e| e.to_string())?,
        );
    }
    if opts.contains_key("--summary")
        && !opts.contains_key("--run-dir")
        && !opts.contains_key("--html")
    {
        table.push_str("\nDetails: omit --summary, or use --run-dir to save report.txt.\n");
    }
    let json = if let Some(view) = &view {
        bounded_json(view, publication_limits.json_bytes)
    } else {
        bounded_json(&review, publication_limits.json_bytes)
    }
    .map_err(|e| e.to_string())?;
    let mut boundaries = if opts.contains_key("--out")
        || opts.contains_key("--run-dir")
        || opts.contains_key("--html")
    {
        wt_sweep::output::revalidate_boundaries(&inventory).map_err(|e| e.to_string())?
    } else {
        inventory
    };
    if let Some(reference) = one(opts, "--compare-reference") {
        let mut paths = match &boundaries.scope {
            ScanScope::Repo { path } => vec![path.clone()],
            ScanScope::Roots { paths } => paths.clone(),
            ScanScope::AllMounted => Vec::new(),
        };
        paths.push(BytePath::from_path(&reference));
        boundaries.scope = ScanScope::Roots { paths };
    }
    progress.finish();
    deliver(
        opts,
        json,
        table,
        &boundaries,
        RunInfo {
            command: if combined { "review" } else { "plan" },
            name: "review.json",
            partial: review
                .disk_usage
                .as_ref()
                .is_some_and(|usage| !usage.complete)
                || !review.coverage.gaps.is_empty()
                || !review.coverage.traversal_complete
                || review.coverage.remaining.is_some()
                || review.coverage.publication.is_some()
                || review.items.iter().any(|item| !item.unknowns.is_empty())
                || comparison.as_ref().is_some_and(|c| c.partial()),
            detail: Some(&detail),
            inventory_json: source_json.as_deref(),
            html: html.as_deref(),
            comparison: comparison_json.as_deref(),
        },
    )
}

fn forge(args: &[OsString]) -> Result<(), String> {
    match args {
        [command] if command == "commandes" => print(b"Capture raw stdout, exit status, argv, scope, timestamps and pagination with the executing client.\nGitLab template: glab api --hostname HOST --paginate 'projects/PROJECT/merge_requests?state=all&per_page=100'\nGitHub template: gh api --hostname HOST --paginate 'repos/OWNER/REPO/pulls?state=all&per_page=100'\nCommands are descriptions only; no network execution. Pagination alone never proves a coherent snapshot.\n"),
        [command, rest @ ..] if command == "ingerer" => {
            let opts = options(rest, &["--raw", "--receipt", "--out", "--format", "--run-dir"], &[])?;
            let raw = one(&opts, "--raw").ok_or("forge ingerer requires --raw FILE")?;
            let receipt = one(&opts, "--receipt");
            let export = wt_sweep::forge::ingest(&raw, receipt.as_deref()).map_err(|e| e.to_string())?;
            let scope = Inventory::new(ScanScope::Roots { paths: Vec::new() });
            let table = format!("Forge requests: {}; pagination complete: {}; temporal consistency: {}; absence proven: {}\n", export.requests.len(), export.coverage.pagination_complete, export.coverage.temporal_consistency, export.coverage.absence_proven);
            let mut opts = opts;
            if !opts.contains_key("--out") && !opts.contains_key("--format") { opts.insert("--format".into(), vec![OsString::from("json")]); }
            deliver(&opts, bounded_json(&export, PublicationLimits::default().json_bytes).map_err(|e| e.to_string())?, table, &scope, RunInfo { command: "forge ingerer", name: "forge.json", partial: !export.coverage.gaps.is_empty() || !export.coverage.pagination_complete, detail: None, inventory_json: None, html: None, comparison: None })
        }
        _ => Err("use forge commandes or forge ingerer --raw FILE".into()),
    }
}

struct RunInfo<'a> {
    command: &'static str,
    name: &'static str,
    partial: bool,
    detail: Option<&'a str>,
    inventory_json: Option<&'a [u8]>,
    html: Option<&'a str>,
    comparison: Option<&'a [u8]>,
}

fn deliver(
    opts: &Options,
    json: Vec<u8>,
    table: String,
    inventory: &Inventory,
    run: RunInfo<'_>,
) -> Result<(), String> {
    deliver_with_opener(opts, json, table, inventory, run, &report_access::open)
}

fn deliver_with_opener(
    opts: &Options,
    json: Vec<u8>,
    table: String,
    inventory: &Inventory,
    run: RunInfo<'_>,
    opener: &dyn Fn(&std::path::Path) -> Result<(), String>,
) -> Result<(), String> {
    let limits = PublicationLimits::default();
    if json.len() > limits.json_bytes || run.detail.unwrap_or(&table).len() > limits.report_bytes {
        return Err(
            "serialized artifact exceeds the 32 MiB read limit; select a narrower scope".into(),
        );
    }
    if !json.ends_with(b"\n") {
        return Err("serialized artifact is missing its final newline".into());
    }
    let base = one(opts, "--run-dir")
        .map(Ok)
        .or_else(|| {
            opts.contains_key("--html")
                .then(report_access::default_base)
        })
        .transpose()?;
    if let Some(base) = base {
        if !opts.contains_key("--run-dir") {
            wt_sweep::output::ensure_run_base(&base, inventory).map_err(|e| e.to_string())?;
        }
        let artifact = wt_sweep::run::Artifact {
            command: run.command,
            name: run.name,
            json: &json,
            report: run.detail.unwrap_or(&table),
            html: run.html,
            partial: run.partial,
        };
        let path = if let Some(comparison) = run.comparison {
            wt_sweep::run::publish_with_comparison(
                &base,
                artifact,
                inventory,
                run.inventory_json
                    .ok_or("comparison requires source inventory")?,
                comparison,
            )
        } else if let Some(source) = run.inventory_json {
            wt_sweep::run::publish_with_inventory(&base, artifact, inventory, source)
        } else {
            wt_sweep::run::publish(&base, artifact, inventory)
        }
        .map_err(|e| e.to_string())?;
        let html_path = if run.html.is_some() {
            let html = path.join("report.html");
            let mut location = format!("  HTML: {}\n", BytePath::from_path(&html).display());
            if let Some(command) = report_access::reopen_command(&html) {
                location.push_str(&format!("  Open HTML: {command}\n"));
            }
            location
        } else {
            String::new()
        };
        let printed = if opts.get("--format").is_some_and(|v| v[0] == "json") {
            print(&json)
        } else if opts.contains_key("--summary") {
            print_table(
                opts,
                &format!(
                    "{table}\nREPORT\n  {}\n{html_path}",
                    BytePath::from_path(&path.join("report.txt")).display()
                ),
            )
        } else {
            print_table(
                opts,
                &format!(
                    "{table}Run: {}\n{html_path}",
                    BytePath::from_path(&path).display()
                ),
            )
        };
        // Publication is complete before asking a desktop application to open it.
        if opts.contains_key("--open") && !wt_sweep::cancel::requested() {
            let html = path.join("report.html");
            if let Err(error) = opener(&html) {
                eprintln!(
                    "wt-sweep: report saved; could not open HTML: {}",
                    BytePath::from_bytes(error.as_bytes()).display()
                );
                eprintln!("  HTML: {}", BytePath::from_path(&html).display());
                if let Some(command) = report_access::reopen_command(&html) {
                    eprintln!("  Open HTML: {command}");
                }
            }
        }
        printed
    } else if let Some(out) = one(opts, "--out") {
        write_new(&out, &json, inventory).map_err(|e| e.to_string())?;
        print_table(opts, &table)
    } else if opts.get("--format").is_some_and(|v| v[0] == "json") {
        print(&json)
    } else {
        print_table(opts, &table)
    }
}

fn print_table(opts: &Options, table: &str) -> Result<(), String> {
    print(terminal::decorate(opts, table).as_bytes())
}

fn print(bytes: &[u8]) -> Result<(), String> {
    std::io::stdout()
        .lock()
        .write_all(bytes)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod progress_tests {
    use super::first_collected_observation;
    use wt_sweep::{
        model::{BytePath, LocalSummary},
        review::ProgressObservation,
    };

    fn history(result: &str, complete: bool, bound: bool) -> Option<ProgressObservation> {
        Some(ProgressObservation::History {
            path: BytePath::from_bytes(b"/fixture/worktree"),
            head: Some("a".repeat(40)),
            target_ref: BytePath::from_bytes(b"refs/heads/main"),
            target_oid: bound.then(|| "b".repeat(40)),
            result: result.into(),
            complete,
        })
    }

    fn local(observed: usize, complete: bool) -> Option<ProgressObservation> {
        let mut summary = LocalSummary::default();
        summary.staged.observed = observed;
        summary.staged.complete = complete;
        Some(ProgressObservation::Local {
            path: BytePath::from_bytes(b"/fixture/worktree"),
            summary: Box::new(summary),
        })
    }

    #[test]
    fn pending_history_does_not_hide_later_collected_relationship() {
        let (mut history_seen, mut local_seen) = (false, false);
        for event in [
            history("unknown", false, true),
            history("unknown", true, true),
            history("ancestor", false, true),
            history("ancestor", true, false),
            None,
        ] {
            assert!(!first_collected_observation(
                &event,
                &mut history_seen,
                &mut local_seen
            ));
            assert!(!history_seen);
        }
        assert!(first_collected_observation(
            &history("ancestor", true, true),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(!first_collected_observation(
            &history("identical_commit", true, true),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(!local_seen);
    }

    #[test]
    fn local_unknown_is_followed_by_a_numeric_partial_observation() {
        let (mut history_seen, mut local_seen) = (false, false);
        assert!(!first_collected_observation(
            &local(0, false),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(!local_seen);
        assert!(first_collected_observation(
            &local(2, false),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(!first_collected_observation(
            &local(3, true),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(!history_seen);
    }

    #[test]
    fn exact_zero_and_nonintegration_are_useful_independent_facts() {
        let (mut history_seen, mut local_seen) = (false, false);
        assert!(first_collected_observation(
            &local(0, true),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(first_collected_observation(
            &history("not_ancestor", true, true),
            &mut history_seen,
            &mut local_seen
        ));
        assert!(history_seen && local_seen);
    }
}
