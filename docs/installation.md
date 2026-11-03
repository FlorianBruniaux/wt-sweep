# Persistent installation and guided analysis

[Getting started](getting-started.md) · [Release checklist](release.md) · [CLI reference](cli.md)

These instructions apply to [v0.1.0-alpha.4](https://github.com/FlorianBruniaux/wt-sweep/releases/tag/v0.1.0-alpha.4).
Use the [download instructions](getting-started.md#download-the-apple-silicon-alpha)
to verify and extract the archive. No Rust or Python
is needed to run the packaged binary or its Bash installer on macOS Apple Silicon.

## Install an extracted release

First verify the archive with its accompanying `SHA256SUMS`, then extract it.
From the extracted `wt-sweep-v0.1.0-alpha.4-aarch64-apple-darwin` directory:

```sh
bash install.sh
"$HOME/.local/bin/wt-sweep" --version
"$HOME/.local/bin/wt-sweep" plan --help
```

The installer verifies `FILES.sha256`, copies the distribution, licenses and
manifest into `~/.local/lib/wt-sweep/0.1.0-alpha.4/`, then creates the launcher
`~/.local/bin/wt-sweep`. Removing the original download does not remove the
installed copy. It does not change shell configuration or require `sudo`.
The internal checksum detects damaged files; verify the published archive
checksum before running any code from a download.

Use the absolute launcher path in scripts. To use `wt-sweep` in the current shell:

```sh
export PATH="$HOME/.local/bin:$PATH"
command -v wt-sweep
wt-sweep --version
```

For another destination, run `bash install.sh --prefix "/absolute/install/path"`.
An existing command is preserved unless you explicitly pass `--replace`.
An update installs a new version and switches the launcher; previous version
files remain under `lib/wt-sweep/`. An existing version directory is never
overwritten, even with `--replace`. To test the same version again, choose a
new prefix. A directory at the launcher path is always refused.

## Inventory, then choose the targets

This example requests discovery across accessible roots and visible mounted
volumes, including hidden directories. Access restrictions and finite budgets
can leave gaps. The saved inventory can be reused without another disk measurement.

```sh
(
  set -eu
  sweep_bin="$HOME/.local/bin/wt-sweep"
  "$sweep_bin" --version
  report_base="$HOME/Desktop/Worktree Sweep"
  mkdir -p "$report_base"
  report_dir="$(mktemp -d "$report_base/guided.XXXXXX")"

  "$sweep_bin" scan --all --disk-usage \
    --format json --out "$report_dir/inventory.json"

  "$sweep_bin" plan --inventory "$report_dir/inventory.json" \
    --interactive --group-by repo --summary --open --run-dir "$report_dir"

  printf '\nReports: %s\n' "$report_dir"
)
```

For a narrower discovery, replace `--all` with explicit `--root` directories.
The report directory must be outside every explicit root: `--root "$HOME"`
cannot write reports to your Desktop. For that scope, choose a fresh report
directory under `/private/tmp` instead. Check coverage before calling a scan
complete. Worktree allocation does not explain all Mac disk usage or establish
how much deleting a worktree would reclaim on APFS.

At each `Targets>` prompt, select numbers or full refs such as
`refs/heads/develop`, according to that repository. `more` shows more suggestions,
`skip` leaves that family without an integration target, and `quit` cancels.
There is no assumed default branch. Review the complete selection and type
`yes` to start. `--open` saves the HTML report and asks macOS to open it in the
default browser. The opener can fail independently of report generation.

The terminal must be interactive. For automation, use explicit `--target` or
`--target-map` instead. A successful exit can still contain partial evidence.
Read `keep`, `verify` and `cleanup_candidate` together with the missing checks;
this flow never removes a worktree.

## Troubleshooting

- **Command not found:** use `"$HOME/.local/bin/wt-sweep"` and check `--version`.
  A shell may still resolve a previous binary earlier in `PATH`.
- **Unknown `--interactive`:** you are running alpha.3 or an older binary.
- **Checksum failure:** stop and acquire a fresh verified archive.
- **Version already installed:** use a new prefix for a fresh test; the installer
  does not overwrite an installed version.
- **macOS blocks the downloaded binary:** retain the protection. Browser
  quarantine and Apple notarization are not qualified by the local archive test.
