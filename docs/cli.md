# CLI reference

[Documentation index](README.md) · [Getting started](getting-started.md) · [JSON contracts](contracts.md)

Run `wt-sweep --help` for global help. The executable is
`./target/release/wt-sweep` after a release build. Options are separate arguments,
for example `--workers 4`. Subcommand-specific `--help` is not implemented.

## Commands

```text
wt-sweep scan (--repo PATH | --root PATH... | --all)
  [--workers 1..8] [--local] [--format json|table]
  [--out FILE | --run-dir BASE]

wt-sweep plan --inventory FILE [--target FULL_REF]...
  [--forge FILE] [--summary] [--format json|table]
  [--out FILE | --run-dir BASE]

wt-sweep forge commandes
wt-sweep forge ingerer --raw FILE [--receipt FILE]
  [--format json|table] [--out FILE | --run-dir BASE]

wt-sweep --help | -h
wt-sweep --version | -V
```

The `forge` subcommand names are currently French: `commandes` prints command
templates; `ingerer` imports an export. English aliases are not implemented.
Running with no arguments prints help and does not scan anything.

## scan

Exactly one scope is required. Repeated `--root` values are allowed;
`--repo`, `--root` and `--all` cannot be combined.

| Option | Behavior |
| --- | --- |
| `--repo PATH` | Inspect one Git family through an explicit repository path. Include its registered worktrees. |
| `--root PATH` | Discover repositories below a selected root; repeat for multiple roots. |
| `--all` | Discover from kernel-reported visible mounts, preserving coverage gaps. |
| `--workers N` | Parallel metadata reads, from 1 to 8; default 1. Git calls remain sequential. |
| `--local` | Collect bounded resident-file manifests, index evidence and the worktree's own HEAD reflog. Off by default. |

Discovery does not silently exclude hidden or ignored directories. It also
looks for nested repositories, bare repositories and separated administration.
Unavailable paths, contradictory registrations and traversal errors remain
observations. No repair, pruning or fetch is performed.

## plan

| Option | Behavior |
| --- | --- |
| `--inventory FILE` | Required saved `wt-sweep.inventory/1` artifact. |
| `--target FULL_REF` | Explicit integration ref under `refs/heads/` or `refs/remotes/`; repeat for several. No default is guessed. |
| `--forge FILE` | Optional normalized `wt-sweep.forge/1` export. Its raw source and receipt, when supplied, must still be readable and match. |
| `--summary` | Print exactly five lines; incompatible with `--format json`. |

The command enriches the inventory with current local Git observations, then
applies deterministic judgement. It needs access to the repositories. It does
not reread the local files represented by saved manifests and never contacts a
forge. The resulting review contains no executable actions.

## forge

`forge commandes` prints GitHub/GitLab capture templates and executes nothing.
`forge ingerer` requires a raw local file; `--receipt` supplies the executor's
record of the capture. Without it, positive facts have partial provenance.

Use the [forge guide](forge.md) for supported payloads, receipt fields and a
fully local example. Raw inputs are limited to 16 MiB and receipts to 1 MiB.

## Output modes

| Mode | stdout | Saved files |
| --- | --- | --- |
| Default `scan` / `plan` | Human-readable table / detailed review | None |
| Default `forge ingerer` | JSON | None |
| `--format json` | JSON only | None, unless another output option is used |
| `--out FILE` | Human-readable output, even with `--format json` | New JSON file |
| `--run-dir BASE` | Selected format; human output also gives the run path | New run directory with JSON, `report.txt`, `run.json` |
| `plan --summary --run-dir BASE` | Five lines, including the saved detail path | Full detail in `report.txt`, JSON and receipt |

`--out` and `--run-dir` are mutually exclusive. `--out` always saves JSON;
`--summary` changes the human output, not the saved review. Forge ingestion keeps
its default JSON stdout with `--run-dir`; add `--format table` to see the run path.

Output parents must already exist. Files and run directories are created
exclusively, outside protected scan/repository boundaries, without following
symlink ancestors. Existing files are refused. `plan` rechecks current worktree
registrations before publication to account for moved worktrees.

Prefer these output options to shell redirection: the shell creates or truncates
a redirected file before the CLI can check its location. A successful receipt
binds the artifacts it lists; it is not a backup or a snapshot.

## Resource limits

| Scope | Default ceiling |
| --- | --- |
| Root/global discovery | 1,000,000 entries, 100,000 directories, depth 256, 60 seconds |
| One controlled Git call | 10 seconds, 8 MiB combined captured output |
| Local manifest per worktree | 20,000 combined file/index/administration entries; 64 MiB per file; 256 MiB read; 8 MiB compact manifest; depth 256; 60 seconds |
| Local manifests across one scan | 25,000 combined entries; 256 MiB published hashed content; 24 MiB compact manifests; 120 seconds |
| Published inventory, review or forge JSON | At most 32 MiB including the final newline |

The whole-scan content budget counts published hashed content, not all possible
rereads. Limit hits produce explicit gaps or a command refusal. Deadlines bound
controlled work; a syscall blocked in the kernel may not return immediately.
These ceilings are not performance measurements. Only `--workers` is exposed
as a resource-setting CLI option.

Commit subjects are bounded to 240 bytes. Human output escapes control bytes
and bounds descriptive titles and links. Descriptions never affect classification;
the JSON retains the full HEAD and evidence needed to interpret the result.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Command completed. Evidence may still be partial or unknown. |
| `2` | Invalid arguments, unsupported operation, read/publication error, or another explicit refusal. |

Diagnostics go to stderr. There is no separate exit code for incomplete coverage;
consumers must inspect the JSON and [receipt status](contracts.md#run-receipts).
`apply` always returns `2` because preservation and approval gates are not verified.
