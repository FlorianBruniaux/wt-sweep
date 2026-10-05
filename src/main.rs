use std::{collections::BTreeMap, ffi::OsString, io::Write, path::PathBuf, process::ExitCode};
use wt_sweep::{
    discover::{DiscoveryOptions, LocalFilesystem, scan_scope_with},
    model::*,
    output::{canonical_digest, inventory_json, inventory_table, read_artifact, write_new},
    review::{enrich, judge},
};

const HELP: &str = "Worktree Sweep (wt-sweep)

Usage:
  wt-sweep scan (--repo PATH | --root PATH... | --all) [--workers 1..8] [--local] [--format json|table] [--out FILE | --run-dir BASE]
  wt-sweep plan --inventory FILE [--target FULL_REF]... [--forge FILE] [--summary] [--format json|table] [--out FILE | --run-dir BASE]
  wt-sweep forge commandes
  wt-sweep forge ingerer --raw FILE [--receipt FILE] [--out FILE | --run-dir BASE]
  wt-sweep --help | --version

Read-only inventory and evidence review. Scans require explicit scope.
JSON preserves path bytes as hexadecimal; tables escape control bytes.
--out creates a new JSON file outside inspected targets and prints a table.
An integration target must be explicitly named with --target, e.g. refs/heads/main.
Global discovery reports gaps and does not establish a machine-wide snapshot.
--workers bounds parallel metadata reads; aggregation order stays deterministic.
--local hashes local files and index within explicit budgets; gaps remain visible.
--run-dir creates an isolated run below an existing directory outside inspected targets.
apply is unavailable until conservation and approval gates are verified.
";

type Options = BTreeMap<String, Vec<OsString>>;

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!(
                "wt-sweep: {}",
                BytePath::from_bytes(message.as_bytes()).display()
            );
            ExitCode::from(2)
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
        let value = if matches!(arg, "--all" | "--summary" | "--local") {
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
        _ => {}
    }
    match args.first().and_then(|arg| arg.to_str()) {
        Some("apply") => {
            Err("apply is unavailable: conservation and approval gates are not verified".into())
        }
        Some("scan") => scan(&args[1..]),
        Some("plan") => plan(&args[1..]),
        Some("forge") => forge(&args[1..]),
        _ => Err("unsupported arguments; use wt-sweep --help".into()),
    }
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
            "--run-dir",
        ],
        &["--root"],
    )?;
    let modes = ["--repo", "--root", "--all"]
        .iter()
        .filter(|name| opts.contains_key(**name))
        .count();
    if modes != 1 {
        return Err("scan requires exactly one explicit scope: --repo, --root or --all".into());
    }
    let scope = if let Some(repo) = one(&opts, "--repo") {
        ScanScope::Repo {
            path: BytePath::from_path(&repo),
        }
    } else if let Some(roots) = opts.get("--root") {
        ScanScope::Roots {
            paths: roots
                .iter()
                .map(|r| BytePath::from_path(&PathBuf::from(r)))
                .collect(),
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
    let mut inventory =
        scan_scope_with(&scope, &limits, &LocalFilesystem).map_err(|e| e.to_string())?;
    if opts.contains_key("--local") {
        collect_local(&mut inventory);
    }
    deliver(
        &opts,
        inventory_json(&inventory).map_err(|e| e.to_string())?,
        inventory_table(&inventory),
        &inventory,
        RunInfo {
            command: "scan",
            name: "inventory.json",
            partial: !inventory.coverage.traversal_complete || !inventory.coverage.gaps.is_empty(),
            detail: None,
        },
    )
}

fn collect_local(inventory: &mut Inventory) {
    use wt_sweep::local::{LocalLimits, collect};
    let started = std::time::Instant::now();
    let mut entries_left = 25_000;
    let mut bytes_left = 256 * 1024 * 1024;
    let mut manifest_left = 24 * 1024 * 1024;
    for worktree in inventory
        .families
        .iter()
        .flat_map(|family| &family.worktrees)
    {
        let remaining = std::time::Duration::from_secs(120).saturating_sub(started.elapsed());
        if entries_left == 0 || bytes_left == 0 || manifest_left < 4096 || remaining.is_zero() {
            inventory.coverage.gaps.push(Gap {
                path: worktree.path.clone(),
                reason: "whole-scan local inventory budget exhausted; worktree not collected"
                    .into(),
            });
            inventory.coverage.traversal_complete = false;
            continue;
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

fn plan(args: &[OsString]) -> Result<(), String> {
    let opts = options(
        args,
        &[
            "--inventory",
            "--target",
            "--forge",
            "--out",
            "--format",
            "--summary",
            "--run-dir",
        ],
        &["--target"],
    )?;
    if opts.contains_key("--summary") && opts.get("--format").is_some_and(|v| v[0] == "json") {
        return Err("--summary is incompatible with --format json".into());
    }
    let input = one(&opts, "--inventory").ok_or("plan requires --inventory FILE")?;
    let inventory: Inventory = read_artifact(&input).map_err(|e| e.to_string())?;
    let forge: Option<ForgeExport> = one(&opts, "--forge")
        .map(|p| read_artifact(&p))
        .transpose()
        .map_err(|e| e.to_string())?;
    let targets: Vec<_> = opts
        .get("--target")
        .into_iter()
        .flatten()
        .map(|t| BytePath::from_path(&PathBuf::from(t)))
        .collect();
    let facts = enrich(&inventory, &targets, forge.as_ref()).map_err(|e| e.to_string())?;
    let review = judge(&facts).map_err(|e| e.to_string())?;
    if canonical_digest(&review).map_err(|e| e.to_string())? != review.digest_sha256 {
        return Err("review digest mismatch".into());
    }
    let detail = wt_sweep::presentation::detail(&review);
    let table = if opts.contains_key("--summary") {
        wt_sweep::presentation::summary(&review)
    } else {
        detail.clone()
    };
    let mut boundaries = inventory.clone();
    if opts.contains_key("--out") || opts.contains_key("--run-dir") {
        // Preserve the original scope and union current registrations. A stale
        // inventory must never authorize writing inside a newly moved worktree.
        for family in &inventory.families {
            let entrypoint = family
                .worktrees
                .first()
                .map(|tree| &tree.path)
                .unwrap_or(&family.common_dir)
                .to_path()
                .map_err(|e| e.to_string())?;
            let current = wt_sweep::git::scan_family(&entrypoint)
                .map_err(|e| format!("cannot revalidate output boundaries: {e}"))?;
            if current
                .families
                .first()
                .is_none_or(|f| f.identity != family.identity)
            {
                return Err("output boundary identity changed".into());
            }
            boundaries.families.extend(current.families);
        }
    }
    deliver(
        &opts,
        serde_json::to_vec_pretty(&review).map_err(|e| e.to_string())?,
        table,
        &boundaries,
        RunInfo {
            command: "plan",
            name: "review.json",
            partial: !review.coverage.gaps.is_empty()
                || !review.coverage.traversal_complete
                || review.items.iter().any(|item| !item.unknowns.is_empty()),
            detail: Some(&detail),
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
            deliver(&opts, serde_json::to_vec_pretty(&export).map_err(|e| e.to_string())?, table, &scope, RunInfo { command: "forge ingerer", name: "forge.json", partial: !export.coverage.gaps.is_empty() || !export.coverage.pagination_complete, detail: None })
        }
        _ => Err("use forge commandes or forge ingerer --raw FILE".into()),
    }
}

struct RunInfo<'a> {
    command: &'static str,
    name: &'static str,
    partial: bool,
    detail: Option<&'a str>,
}

fn deliver(
    opts: &Options,
    mut json: Vec<u8>,
    table: String,
    inventory: &Inventory,
    run: RunInfo<'_>,
) -> Result<(), String> {
    if json.len() >= 32 * 1024 * 1024 {
        return Err(
            "serialized artifact exceeds the 32 MiB read limit; select a narrower scope".into(),
        );
    }
    json.push(b'\n');
    if let Some(base) = one(opts, "--run-dir") {
        let path = wt_sweep::run::publish(
            &base,
            wt_sweep::run::Artifact {
                command: run.command,
                name: run.name,
                json: &json,
                report: run.detail.unwrap_or(&table),
                partial: run.partial,
            },
            inventory,
        )
        .map_err(|e| e.to_string())?;
        if opts.get("--format").is_some_and(|v| v[0] == "json") {
            print(&json)
        } else if opts.contains_key("--summary") {
            let mut lines: Vec<_> = table.lines().map(str::to_owned).collect();
            if let Some(detail_line) = lines.get_mut(3) {
                *detail_line = format!(
                    "Read-only review; removal unavailable. Detail: {}",
                    BytePath::from_path(&path.join("report.txt")).display()
                );
            }
            print(format!("{}\n", lines.join("\n")).as_bytes())
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
