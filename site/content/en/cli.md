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
  report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"
  "$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
    --disk-usage --summary --html --run-dir "$report_dir"
  open "$report_dir"/run-*/report.html
)
```

`review` discovers and collects fresh evidence in one run. `scan` saves an inventory; `plan --inventory` reviews a saved inventory. `review` does not accept `--local` or `--inventory`.

## Saved artifacts

With `--html --run-dir`, a new `run-*` directory contains `inventory.json`, `review.json`, `report.txt`, `report.html` and `run.json`. Keep them together. The report directory must be outside every inspected root and Git family. Its parent directories must be ordinary directories.

`--html` requires `--run-dir`. `scan --html` is an inventory, marked **not reviewed**, without invented verdicts.

## Options by purpose

| Purpose | Option |
| --- | --- |
| One repository | `--repo ABSOLUTE_PATH` |
| Explicit roots | repeated `--root ABSOLUTE_PATH` |
| Integration evidence | `--target FULL_REF` or a verified per-family `--target-map` |
| Measure disk | `--disk-usage`, independent `--disk-budget standard\|thorough` |
| Limit expensive Git/local evidence | `--worktree EXACT_PATH`, `--budget standard\|thorough` |
| Limit disk roots | repeated `--disk-root EXACT_PATH` with `--disk-usage` |
| Presentation | `--summary`, `--verdict`, `--path-prefix`, `--group-by` |

Display filters do not reduce collection. [Targets and filters](/docs/targets/) and [disk scope](/docs/disk/) explain the distinction.

A command can exit with code `0` while its evidence remains partial. Read the coverage, missing conditions and diagnostics. [The full repository CLI reference](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/cli.md) defines all options and exit codes.
