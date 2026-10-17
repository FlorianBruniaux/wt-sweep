# CLI recipes

[Workflow](../SKILL.md) · [Report interpretation](reports.md)

Resolve `sweep_root`, `repo_path` and `target_ref` from the current task before
running a recipe. `sweep_root` is the wt-sweep source checkout, not the repository
being inspected. Use an absolute binary path. These variables are intentionally
not assigned to a guessed user directory.

## Check the interface

```sh
sweep_bin="$sweep_root/target/release/wt-sweep"
"$sweep_bin" --version
"$sweep_bin" --help
"$sweep_bin" review --help
```

If the binary is absent, and dependencies are already cached:

```sh
(cd "$sweep_root" && cargo build --release --locked --offline)
```

The collector currently requires macOS. A successful compile on another system
does not establish runtime support. Help is the authority for available flags.

## Review one family

The user must have selected `repo_path` and confirmed `target_ref`, for example a
full `refs/heads/...` or `refs/remotes/...` name. Reading a remote-tracking ref
does not refresh it from the remote.

```sh
report_dir="$(mktemp -d /private/tmp/wt-sweep-report.XXXXXX)"
review_status=0
"$sweep_bin" review \
  --repo "$repo_path" \
  --target "$target_ref" \
  --group-by directory \
  --budget standard \
  --format json --html --disk-usage \
  --run-dir "$report_dir" > "$report_dir/console.json" || review_status=$?
printf 'Exit status: %s\nReports: %s\n' "$review_status" "$report_dir"
```

`mktemp` replaces the six X characters itself. Keep that suffix verbatim. The
command creates a `run-*` child containing `inventory.json`, `review.json`,
`report.txt`, `report.html` and `run.json`; do not guess the child name.
After checking the receipt, use `open "$report_dir"/run-*/report.html` on macOS.
The report is self-contained and opens offline; no external script or service is
needed. `--html` requires `--run-dir` and leaves JSON stdout undecorated. Check the exit status
before accepting the output. `0` can still describe incomplete observations;
`130` is interruption. Read other failure messages and do not assume publication.
`console.json` is only a capture of stdout, not a receipted run artifact.

Use `--summary` instead of `--format json` for a person reading the console.
They cannot be combined. Avoid `--out` when the agent expects stdout to be JSON.

## Retry a capped worktree

Read the saved run before collecting again. If its standard cap is the cause,
set `worktree_path` to the exact recorded path and use a new output base:

```sh
thorough_dir="$(mktemp -d /private/tmp/wt-sweep-thorough.XXXXXX)"
review_status=0
"$sweep_bin" review --repo "$repo_path" --target "$target_ref" \
  --worktree "$worktree_path" --path-prefix "$worktree_path" \
  --budget thorough --format json --html --disk-usage \
  --disk-root "$worktree_path" --run-dir "$thorough_dir" \
  > "$thorough_dir/console.json" || review_status=$?
printf 'Exit status: %s\nReports: %s\n' "$review_status" "$thorough_dir"
```

Inspect this run before selecting the whole family for thorough collection.
`--worktree` focuses evidence I/O; `--path-prefix` only filters display. The
original inventory and other rows remain in the saved source with their gaps.
`--disk-root` separately selects an exact registered disk root; it requires
`--disk-usage` and can be repeated. Other disk rows are retained as `not requested`.
Omit `--disk-root` only when the requested task needs all registered sizes.
A focused size total never covers the whole inventory, even if its one root
finishes. Use the report's requested/measured/unrequested counts.
`plan --inventory` accepts the same budget selector. Profile ceilings are in the
checkout's `docs/cli.md`; larger allowances still retain ignored files and may
stop with unknown evidence. A new run is not a resumed snapshot.

## Discover before choosing per-repository targets

```sh
"$sweep_bin" scan --help
inventory_dir="$(mktemp -d /private/tmp/wt-sweep-discovery.XXXXXX)"
"$sweep_bin" scan --root "$chosen_root" --out "$inventory_dir/inventory.json"
```

Repeat `--root` for additional authorized roots. Inspect discovered families,
path identities, gaps and remaining scope. Ask for any missing target refs.
Then run `review --repo` for each chosen family with its own confirmed ref and
unique run directory. Alternatively use `plan --inventory` with an explicit
`--target-map`: consult `plan --help` and the checkout's
`docs/targets-and-filters.md` for the current identity-bound map format.

An explicit machine-wide request may use `scan --all`, subject to the same
coverage limits. Never use that mode for a development smoke test.

## Continue explicit-root discovery

Check `scan --help` first. To retain progress, start an authorized root scan with
`--checkpoint-out "$inventory_dir/pass-1.json"`. When its traversal stops:

```sh
"$sweep_bin" scan --resume "$inventory_dir/pass-1.json" \
  --checkpoint-out "$inventory_dir/pass-2.json" --run-dir "$inventory_dir" --html
```

Use a new destination at every pass. The checkpoint supplies the original roots;
never silently expand them. `--repo`, `--all` and `review` cannot resume this state.
Read `coverage.traversal_complete`, gaps and `discovery_intervals` before calling
it complete. An incomplete directory is replayed; a wide listing may need a
larger `--discovery-entries` allowance. A changed recorded directory identity or observed alias target is refused.
Paths still pending are read freshly.
Completed subtrees remain historical observations, while known Git candidates
are collected again. Preserve the separate intervals; this is not a snapshot.
Checkpoint state contains no saved HEAD, verdict, local hashes or disk measures.

Completion: the new checkpoint and report exist, or the exit diagnostic is
reported without claiming saved progress. A failed digest or identity check
requires a fresh authorized scan, not manual checkpoint editing.

## Compare local content

Check `review --help` for these options before using them. A content comparison
requires a saved run, preserves the review verdict and publishes a separate
`local-comparison.json` with source bindings:

```sh
"$sweep_bin" review --repo "$chosen_repo" --target "$confirmed_target" \
  --worktree "$chosen_worktree" --compare-local --html --run-dir "$report_base"
```

If the user supplies a retained checkout in the same family, add
`--compare-reference "$reference_checkout"`. This observes the current copy;
it does not establish that those bytes are committed, integrated or backed up.
The option refuses self-comparison and an ambiguous checkout across families.
Use confirmed targets per family for a multi-repo run. `plan --inventory` also
supports comparison and retains the source inventory in that run.

Read the file-level results and omissions before proposing inspection or a
focused retry with the existing `--budget thorough`. Do not promise that a
larger budget will finish. Display filters do not reduce collection work.

## Inspect an existing run in pages

Set `run_dir` to the exact saved `run-*` directory. Read `run.json` first, check
its declared artifacts, then read `report.txt` for the human explanation. When
JSON is large, this standard-library recipe prints totals plus a page of original
rows. It does not validate receipt hashes, recompute the verdict or attest to the
current filesystem. Check the receipt separately as described in
[report interpretation](reports.md).

```sh
python3 - "$run_dir/review.json" 0 5 <<'PYCODE'
import collections, json, pathlib, sys
path = pathlib.Path(sys.argv[1])
offset, limit = map(int, sys.argv[2:])
if offset < 0 or not 1 <= limit <= 20:
    raise SystemExit("Use an offset >= 0 and a page size from 1 to 20")
with path.open("rb") as stream:
    payload = stream.read(32 * 1024 * 1024 + 1)
if len(payload) > 32 * 1024 * 1024:
    raise SystemExit("Review exceeds this reader's size limit")
data = json.loads(payload)
view = data.get("schema") == "wt-sweep.review-view/1"
source = data["source"] if view else data
if source.get("schema") != "wt-sweep.review/2" or source.get("policy_version") != "triage/1":
    raise SystemExit("Unsupported review; inspect its version-specific contract")
items = source["items"]
selected = {p["hex"] for p in data["selected_paths"]} if view else {r["path"]["hex"] for r in items}
rows = [r for r in items if r["path"]["hex"] in selected]
print(json.dumps({
    "source_total": len(items), "selected_total": len(rows),
    "source_verdicts": dict(collections.Counter(r["classification"] for r in items)),
    "coverage": source["coverage"], "not_collected": source.get("not_collected"),
    "offset": offset, "next_offset": offset + limit if offset + limit < len(rows) else None,
    "rows": rows[offset:offset + limit],
}, ensure_ascii=True, indent=2))
PYCODE
```

Increase the offset by the page size until `next_offset` is null. A large row or
coverage object may still exceed the client's output capacity: select its fields
in smaller reads and retain a checklist of the omitted fields. Do not call a
truncated tool response a complete review. JSON escaping is for display; use
`bytes.fromhex(path["hex"])` when a subsequent operation needs exact path bytes.
