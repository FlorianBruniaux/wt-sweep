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
  --format json \
  --run-dir "$report_dir" > "$report_dir/console.json" || review_status=$?
printf 'Exit status: %s\nReports: %s\n' "$review_status" "$report_dir"
```

`mktemp` replaces the six X characters itself. Keep that suffix verbatim. The
command creates a `run-*` child containing `inventory.json`, `review.json`,
`report.txt` and `run.json`; do not guess the child name. Check the exit status
before accepting the output. `0` can still describe incomplete observations;
`130` is interruption. Read other failure messages and do not assume publication.
`console.json` is only a capture of stdout, not a receipted run artifact.

Use `--summary` instead of `--format json` for a person reading the console.
They cannot be combined. Avoid `--out` when the agent expects stdout to be JSON.

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
