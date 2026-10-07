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
  wt-sweep scan (--repo PATH | --root PATH... | --all) [--workers 1..8] [--local] [--disk-usage] [--disk-budget standard|thorough] [--format json|table] [--out FILE | --run-dir BASE]
  wt-sweep plan --inventory FILE [--target FULL_REF... | --target-map FILE] [--filter-repo PATH] [--path-prefix PATH] [--branch FULL_REF] [--local-state clean|dirty|unknown] [--verdict keep|cleanup_candidate|verify] [--group-by repo|directory] [--changelog-path RELATIVE...] [--no-context] [--forge FILE] [--summary] [--format json|table] [--out FILE | --run-dir BASE]
  wt-sweep forge commandes
  wt-sweep forge ingerer --raw FILE [--receipt FILE] [--out FILE | --run-dir BASE]
  wt-sweep --help | --version

Read-only inventory and evidence review. Scans require explicit scope.
JSON preserves path bytes as hexadecimal; tables escape control bytes.
--out creates a new JSON file outside inspected targets and prints a table.
Use --target for one repository family, or --target-map for explicit targets per family.
Filters select reviewed rows; JSON views retain the full source and its coverage.
Global discovery reports gaps and does not establish a machine-wide snapshot.
--workers bounds parallel metadata reads; aggregation order stays deterministic.
--disk-usage measures file sizes and allocated blocks without reading contents (scan and review).
--local hashes local files and index within explicit budgets; gaps remain visible.
review and plan report readable progress on stderr and keep JSON on stdout.
--summary prints verdicts, collection coverage and remaining checks.
--worktree PATH focuses evidence collection; unselected rows remain explicitly incomplete.
--run-dir creates an isolated run below an existing directory outside inspected targets.
apply is unavailable until conservation and approval gates are verified.
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
            "--all" | "--summary" | "--local" | "--no-context" | "--disk-usage"
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
        values
            .entry(arg.to_string())
            .or_insert_with(Vec::new)
            .push(value);
    }
    if values.contains_key("--out") && values.contains_key("--run-dir") {
        return Err("--out and --run-dir are mutually exclusive".into());
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
            "wt-sweep scan (--repo PATH | --root PATH... | --all) [--workers 1..8] [--local] [--disk-usage] [--disk-budget standard|thorough] [--format json|table] [--out FILE | --run-dir BASE]"
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
            "Discovery requires an explicit scope. --disk-usage measures registered roots, including ignored files, without hashing.\nDisk budgets: 30s/500,000 entries per root; 600s/2,000,000 entries overall; depth 128.\n--disk-budget thorough: 120s/1,000,000 entries per root; 1800s/10,000,000 overall.\nAllocated blocks are not a reclaim estimate; partial measurements remain explicit.\n--local additionally hashes local content within budgets.\nProgress is on stderr, with elapsed time and average listing throughput.\nReports retain discovery metrics and coverage separately. Gaps and unfinished scope remain in the inventory.\n"
        }
        "plan" | "review" => {
            "REVIEW OPTIONS:\n  --budget standard|thorough (default: standard; larger finite collection allowances)\n  --target FULL_REF... | --target-map FILE\n  --worktree ABSOLUTE_PATH (collect evidence for this exact recorded worktree)\n  --filter-repo PATH  --path-prefix PATH  --branch FULL_REF\n  --local-state clean|dirty|unknown  --verdict keep|cleanup_candidate|verify\n  --group-by repo|directory  --forge FILE\n  --changelog-path RELATIVE... | --no-context\n  --summary  --format json|table  --out FILE | --run-dir BASE\n\nTargets must be explicit; no target leaves integration unknown.\nFilters select rows after judgement and retain source coverage in JSON views.\n--worktree narrows history/local/private/context reads; full inventory and identity checks remain.\nOther rows explicitly retain missing evidence. Add --path-prefix for a focused display.\n--summary cannot be combined with --format json. Progress is on stderr.\n--run-dir needs an existing directory outside inspected targets.\nreview accepts --disk-usage [--disk-budget standard|thorough] for metadata-only sizes.\nDisk measurement covers all inventoried roots, independently of --worktree and display filters.\nIts budget is separate from --budget; defaults: 30s/root, 600s overall; thorough: 120s/root, 1800s overall.\nreview saves inventory.json, review.json, report.txt and run.json together.\nplan reuses saved disk observations with their original interval; it does not recount sizes.\nplan saves review.json, report.txt and run.json.\nA cleanup_candidate still requires confirmation of its use. apply is unavailable.\n"
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
            "--local",
            "--disk-usage",
            "--disk-budget",
            "--run-dir",
        ],
        &["--root"],
    )?;
    let disk_limits = disk_settings(&opts)?;
    let mut inventory = collect_inventory(&opts)?;
    collect_disk(&mut inventory, disk_limits.as_ref())?;
    let publication_limits = PublicationLimits::default();
    if opts.contains_key("--local") {
        collect_local(&mut inventory, &publication_limits);
    }
    let prepared = wt_sweep::output::prepare_inventory(&inventory, &publication_limits)
        .map_err(|e| e.to_string())?;
    let boundaries = if opts.contains_key("--out") || opts.contains_key("--run-dir") {
        wt_sweep::output::revalidate_boundaries(&inventory).map_err(|e| e.to_string())?
    } else {
        inventory
    };
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
        },
    )
}

fn disk_settings(opts: &Options) -> Result<Option<wt_sweep::disk::DiskLimits>, String> {
    let limits = match opts.get("--disk-budget").and_then(|values| values.first()) {
        Some(_) if !opts.contains_key("--disk-usage") => {
            return Err("--disk-budget requires --disk-usage".into());
        }
        Some(value) if value == "standard" => wt_sweep::disk::DiskLimits::default(),
        Some(value) if value == "thorough" => wt_sweep::disk::DiskLimits::thorough(),
        Some(_) => return Err("--disk-budget must be standard or thorough".into()),
        None => wt_sweep::disk::DiskLimits::default(),
    };
    Ok(opts.contains_key("--disk-usage").then_some(limits))
}

fn collect_disk(
    inventory: &mut Inventory,
    limits: Option<&wt_sweep::disk::DiskLimits>,
) -> Result<(), String> {
    if let Some(disk_limits) = limits {
        eprintln!("Measuring registered worktrees and shared Git directories...");
        wt_sweep::disk::collect(inventory, disk_limits, &mut |row| {
            let size = row
                .bytes
                .as_ref()
                .map(|b| wt_sweep::disk::human_bytes(b.allocated))
                .unwrap_or_else(|| "unknown".into());
            eprintln!(
                "  {}: {size} ({})",
                row.path.display(),
                if row.complete {
                    "complete"
                } else {
                    "incomplete"
                }
            );
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn collect_inventory(opts: &Options) -> Result<Inventory, String> {
    let modes = ["--repo", "--root", "--all"]
        .iter()
        .filter(|name| opts.contains_key(**name))
        .count();
    if modes != 1 {
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
    let inventory = scan_scope_with_progress(&scope, &limits, &LocalFilesystem, &mut |event| {
        eprintln!("{}", wt_sweep::presentation::discovery_progress(event));
    })
    .map_err(|e| e.to_string())?;
    Ok(inventory)
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
    produce_review(&opts, inventory, settings, false)
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
    ]);
    let opts = options(args, &allowed, &["--root", "--target", "--changelog-path"])?;
    let settings = review_settings(&opts)?;
    let disk_limits = disk_settings(&opts)?;
    let mut inventory = collect_inventory(&opts)?;
    collect_disk(&mut inventory, disk_limits.as_ref())?;
    produce_review(&opts, inventory, settings, true)
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
    let mut review_options = ReviewOptions {
        worktree: one(opts, "--worktree").map(|p| BytePath::from_path(&p)),
        ..ReviewOptions::with_budget(value("--budget")?.as_deref().unwrap_or("standard"))
            .map_err(|e| e.to_string())?
    };
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

fn produce_review(
    opts: &Options,
    inventory: Inventory,
    settings: ReviewSettings,
    combined: bool,
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
    let source_json = combined
        .then(|| bounded_json(&inventory, PublicationLimits::default().json_bytes))
        .transpose()
        .map_err(|e| format!("review input cannot be published; select a narrower scope: {e}"))?;
    let target_input = target_map
        .as_ref()
        .map(TargetInput::Map)
        .unwrap_or(TargetInput::Simple(&targets));
    let facts = enrich_with_options(
        &inventory,
        target_input,
        forge.as_ref(),
        &review_options,
        &wt_sweep::local::NativeHooks,
        &mut |event| {
            eprintln!("{}", wt_sweep::presentation::review_progress(&event));
        },
    )
    .map_err(|e| e.to_string())?;
    let review = judge(&facts).map_err(|e| e.to_string())?;
    if canonical_digest(&review).map_err(|e| e.to_string())? != review.digest_sha256 {
        return Err("review digest mismatch".into());
    }
    let view = use_view
        .then(|| wt_sweep::view::build(&inventory, &review, filters, &group_by))
        .transpose()
        .map_err(|e| e.to_string())?;
    let publication_limits = PublicationLimits::default();
    let detail = if let Some(view) = &view {
        wt_sweep::presentation::view_detail_bounded(view, publication_limits.report_bytes)
    } else {
        wt_sweep::presentation::detail_bounded(&review, publication_limits.report_bytes)
    }
    .map_err(|e| e.to_string())?;
    let mut table = if opts.contains_key("--summary") {
        if let Some(view) = &view {
            wt_sweep::presentation::view_summary(view)
        } else {
            wt_sweep::presentation::summary(&review)
        }
    } else {
        detail.clone()
    };
    if opts.contains_key("--summary") && !opts.contains_key("--run-dir") {
        table.push_str("\nDetails: omit --summary, or use --run-dir to save report.txt.\n");
    }
    let json = if let Some(view) = &view {
        bounded_json(view, publication_limits.json_bytes)
    } else {
        bounded_json(&review, publication_limits.json_bytes)
    }
    .map_err(|e| e.to_string())?;
    let boundaries = if opts.contains_key("--out") || opts.contains_key("--run-dir") {
        wt_sweep::output::revalidate_boundaries(&inventory).map_err(|e| e.to_string())?
    } else {
        inventory
    };
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
                || review.items.iter().any(|item| !item.unknowns.is_empty()),
            detail: Some(&detail),
            inventory_json: source_json.as_deref(),
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
            deliver(&opts, bounded_json(&export, PublicationLimits::default().json_bytes).map_err(|e| e.to_string())?, table, &scope, RunInfo { command: "forge ingerer", name: "forge.json", partial: !export.coverage.gaps.is_empty() || !export.coverage.pagination_complete, detail: None, inventory_json: None })
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
}

fn deliver(
    opts: &Options,
    json: Vec<u8>,
    table: String,
    inventory: &Inventory,
    run: RunInfo<'_>,
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
    if let Some(base) = one(opts, "--run-dir") {
        let artifact = wt_sweep::run::Artifact {
            command: run.command,
            name: run.name,
            json: &json,
            report: run.detail.unwrap_or(&table),
            partial: run.partial,
        };
        let path = if let Some(source) = run.inventory_json {
            wt_sweep::run::publish_with_inventory(&base, artifact, inventory, source)
        } else {
            wt_sweep::run::publish(&base, artifact, inventory)
        }
        .map_err(|e| e.to_string())?;
        if opts.get("--format").is_some_and(|v| v[0] == "json") {
            print(&json)
        } else if opts.contains_key("--summary") {
            print(
                format!(
                    "{table}\nREPORT\n  {}\n",
                    BytePath::from_path(&path.join("report.txt")).display()
                )
                .as_bytes(),
            )
        } else {
            print(format!("{table}Run: {}\n", BytePath::from_path(&path).display()).as_bytes())
        }
    } else if let Some(out) = one(opts, "--out") {
        write_new(&out, &json, inventory).map_err(|e| e.to_string())?;
        print(table.as_bytes())
    } else if opts.get("--format").is_some_and(|v| v[0] == "json") {
        print(&json)
    } else {
        print(table.as_bytes())
    }
}

fn print(bytes: &[u8]) -> Result<(), String> {
    std::io::stdout()
        .lock()
        .write_all(bytes)
        .map_err(|e| e.to_string())
}
