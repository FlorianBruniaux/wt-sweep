# Targets and filters

[Documentation index](README.md) · [CLI reference](cli.md) · [JSON contracts](contracts.md)

A target is the branch against which you want to check integration. `main` in
one repository says nothing about `main` in another. Use `--target` for a scan of
one repository. For several repositories, choose targets interactively or provide a map.
All refs are local observations. The tool never fetches them.

## Guided selection

Source builds after alpha.3 can replace the map-building Python script with:

```sh
wt-sweep plan --inventory "$report_dir/inventory.json" \
  --interactive --compare-local --group-by repo --summary --open
```

Here `report_dir` is the existing run directory containing the inventory.
The CLI lists locally recorded branches per physical family, with common branch
names first. Choose displayed numbers or full refs, separated by spaces, then
confirm the complete selection with `yes`. There is no default target. `more`
pages through long lists; `skip` leaves that family’s integration unknown;
`quit`, EOF or Ctrl-C cancel before analysis. All rows remain in the report.

The prompts require a terminal on stdin and stderr. They do not fetch, infer a
merge from a branch name or change the verdict policy. Ref and directory checks
run again after confirmation. The new run keeps the input inventory and its
original disk observation interval. `--open` publishes and opens HTML; `--html`
only saves it. Add `--run-dir` for an existing destination outside the scanned
scope. For noninteractive automation, the explicit map below remains supported.

## One repository

After saving an inventory, keep your explicit target and add the view you need:

```sh
wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target refs/heads/main --verdict cleanup_candidate --summary

wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target refs/heads/main --local-state dirty --group-by directory
```

A candidate still needs usage confirmation before removal. The report does not
authorize deletion. `dirty` includes untracked and ignored content. `unknown`
means the current local comparison is incomplete; it does not mean clean.

## Several repositories

This manual map is for scripts and unattended runs. For terminal use, prefer
[guided selection](#guided-selection).

The map binds each repository to the exact administration path and device/inode
recorded by the scan. This script lists the families with stable indices and
writes an initially empty map. It preserves original path bytes.

```sh
python3 - "$report_dir/inventory.json" "$report_dir/targets.json" <<'PY'
import json, os, sys
from pathlib import Path
inventory = json.loads(Path(sys.argv[1]).read_text())
families = inventory["families"]
for number, family in enumerate(families):
    path = os.fsdecode(bytes.fromhex(family["common_dir"]["hex"]))
    print(number, repr(path))
# After reviewing the list, fill this mapping with the intended refs.
# Example only: choices = {0: ["refs/heads/main"], 1: ["refs/heads/develop"]}
choices = {}
entries = [dict(common_dir=family["common_dir"], identity=family["identity"],
                targets=[{"hex": ref.encode().hex()} for ref in choices.get(n, [])])
           for n, family in enumerate(families)]
with open(sys.argv[2], "x") as output:
    json.dump({"schema": "wt-sweep.targets/1", "families": entries}, output, indent=2)
    output.write("\n")
PY
```

Choose refs for the listed families and generate a new file with a new output
name. The script refuses an existing file. An empty list intentionally leaves
that family without an accepted target, so it cannot become a candidate through
integration evidence alone. Recreate the mapping after a new scan if a physical
identity changed; do not edit an old identity merely to bypass refusal.

Then inspect the mapped inventory:

```sh
wt-sweep plan --inventory "$report_dir/inventory.json" \
  --target-map "$report_dir/targets.json" --group-by repo --summary
```

For a repository selection, `--filter-repo` accepts the exact main checkout,
linked checkout or common administration path recorded in the inventory. A
missing path returns an empty selection. Paths containing spaces remain one
quoted argument. `--path-prefix` uses path components, so `/work/a` does not
select `/work/ab`. `--branch` takes a full ref such as `refs/heads/feature`.

## What a filtered report retains

Filters change the displayed rows after analysis. They do not save collection
work. The text keeps selected/source totals and full source coverage. JSON uses
`wt-sweep.review-view/1` with the complete original review under `source` and the
selected paths beside it. Both the source and the view have a digest. Even an
empty selection retains source failures and all repository bindings.

To collect less, start a new `scan --repo` or narrow the explicit scan roots.
A filtered report cannot be saved inside a hidden worktree either: destination
checks still use the full inventory.

## Complete the evidence for one worktree

`--worktree` changes the collection scope. It requires the exact absolute path
recorded by discovery. `--path-prefix` only changes the displayed rows. Combine
them to investigate one incomplete worktree while retaining the full source:

```sh
wt-sweep review --repo "$repo_path" --target "$target_ref" \
  --worktree "$worktree_path" --path-prefix "$worktree_path" \
  --summary --run-dir "$report_dir"
```

Set these variables to the confirmed repository, ref, recorded worktree path and
an existing report directory outside the source. The command creates a fresh run;
it neither resumes old reads nor overwrites the earlier report. Unselected rows
keep their identities and current control results, with local/history/private
checks explicitly uncollected. Shared stashes describe the selected repository
family. Per-worktree object, byte and history-depth limits still apply.

Start with the default `standard` profile. Read the saved source gaps and its
optional `collection_metrics`: a time stop, object cap and byte cap need different
interpretations. If the standard allowance is the remaining cause, retry the
same exact selected path with larger finite caps in a new base:

```sh
thorough_dir="$(mktemp -d /private/tmp/wt-sweep-thorough.XXXXXX)"
wt-sweep review --repo "$repo_path" --target "$target_ref" \
  --worktree "$worktree_path" --path-prefix "$worktree_path" \
  --budget thorough --summary --run-dir "$thorough_dir"
```

Inspect that run before choosing a whole-family thorough review. This is a new
observation, not a continuation of the standard run. Display filters still do
not reduce collection. `plan --inventory` accepts the same budget selector.
Larger allowances do not bypass access failures, missing objects, changed
identities or unsupported index interpretation. Ignored files remain included.
See [profile ceilings and metrics](cli.md#thorough-allowances).
