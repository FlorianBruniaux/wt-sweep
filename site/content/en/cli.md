## Review one repository

Run from the built source checkout. Choose an absolute repository path and a full, confirmed integration ref.

```sh
(
  set -eu
  sweep_bin="$PWD/target/release/wt-sweep"
  printf 'Absolute clone path: '
  IFS= read -r repo_path
  printf 'Confirmed full target ref: '
  IFS= read -r target_ref
  /usr/bin/git --no-optional-locks -c core.fsmonitor=false -C "$repo_path" \
    show-ref --verify --quiet "$target_ref"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --disk-usage --summary --open
)
```

`review` discovers and collects fresh evidence in one run. `scan` saves an inventory; `plan --inventory` reviews a saved inventory. `review` does not accept `--local` or `--inventory`.

## Saved artifacts

With `--html` or `--open`, a new `run-*` directory contains `inventory.json`, `review.json`, `report.txt`, `report.html` and `run.json`. Keep them together. The report directory must be outside every inspected root and Git family. Its parent directories must be ordinary directories.

On macOS, HTML runs default to `~/Desktop/Worktree Sweep/run-*/`. The Desktop must exist. Add `--run-dir "/absolute/existing/folder"` to choose another destination outside the inspected scope. `--html` saves without opening; `--open` saves and asks macOS to open the HTML after publication. A failed opener leaves the files available and prints a warning. Human output includes a quoted command to reopen the report.

`scan --html` is an inventory, marked **not reviewed**, without invented verdicts.

## Read the console recap

The English summary starts with observed disk allocation in KB/MB/GB when collected. It separates commit ancestry from cleanup verdicts, then lists up to five selected worktrees by observed size, their blockers and the collection limits. A focused retry uses that row’s recorded targets; confirm they are still suitable. Partial sizes may under-rank a worktree, and observed allocation is not recoverable space.

## Options by purpose

| Purpose | Option |
| --- | --- |
| One repository | `--repo ABSOLUTE_PATH` |
| Explicit roots | repeated `--root ABSOLUTE_PATH` |
| Integration evidence | `--target FULL_REF` or a verified per-family `--target-map` |
| Measure disk | `--disk-usage`, independent `--disk-budget standard\|thorough` |
| Limit expensive Git/local evidence | `--worktree EXACT_PATH`, `--budget standard\|thorough` |
| Limit disk roots | repeated `--disk-root EXACT_PATH` with `--disk-usage` |
| Save and open HTML | `--html`, `--open`, `--run-dir EXISTING_DIRECTORY` |
| Presentation | `--summary`, `--verdict`, `--path-prefix`, `--group-by` |

Display filters do not reduce collection. [Targets and filters](/docs/targets/) and [disk scope](/docs/disk/) explain the distinction.

A command can exit with code `0` while its evidence remains partial. Read the coverage, missing conditions and diagnostics. [The full repository CLI reference](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/cli.md) defines all options and exit codes.
