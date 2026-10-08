# Resume a directory discovery

[CLI reference](cli.md) · [JSON contracts](contracts.md) · [French tutorial](tutorial.fr.md)

Use a discovery checkpoint when an explicit-root scan reaches its directory,
entry or time allowance. The next pass continues the saved traversal and reads
known Git families again. It does not resume local-file hashing, disk measurement
or review evidence.

## Save, then continue

From the wt-sweep checkout, choose the absolute directory to inspect. Keep the
checkpoint and reports outside that directory:

```sh
scan_dir="$(mktemp -d /private/tmp/wt-sweep-discovery.XXXXXX)"
./target/release/wt-sweep scan --root /absolute/path/to/projects \
  --checkpoint-out "$scan_dir/pass-1.json" --run-dir "$scan_dir" --html
```

If the report says discovery is incomplete, continue with a new output filename:

```sh
./target/release/wt-sweep scan --resume "$scan_dir/pass-1.json" \
  --checkpoint-out "$scan_dir/pass-2.json" --run-dir "$scan_dir" --html
```

The checkpoint supplies the original roots. An optional repeated `--root` list
must match them exactly, including order. Each output is created exclusively;
a rerun cannot overwrite an earlier checkpoint. The text report prints a
continuation command for an incomplete traversal. Add `--run-dir` and `--html`
if you also want a saved HTML report for that pass.

Check `coverage.traversal_complete`, gaps and publication omissions in the new
inventory. A zero exit code can still describe partial observations. Interruptions
return 130; check that the checkpoint was published before resuming it. Interruption
before initial root identities are collected can refuse without publishing one.

## Per-pass allowances

| Option | Default | Allowed range |
| --- | --- | --- |
| `--discovery-directories` | 100,000 | 1 to 100,000 |
| `--discovery-entries` | 1,000,000 | 1 to 1,000,000 |
| `--discovery-seconds` | 60 | 1 to 600 |

An incomplete directory is replayed from its beginning: the underlying listing
can stop before sorting, so the last sorted filename is not a valid cursor.
A wide directory can stall at the same allowance. Increase the entry allowance
within its range, or start a fresh scan of narrower roots. Saved directory and
alias identities are rechecked on each pass, with their own entry allowance and
within the same time budget. A checkpoint whose validation exhausts that allowance
retains its frontier and reports no traversal progress.

Only `scan` with explicit roots supports this workflow. `--repo`, `--all` and
`review` do not accept discovery checkpoints. Review budgets remain controlled
by `--budget standard|thorough`.

## What the checkpoint establishes

The versioned file records root identities, completed directories, pending paths,
the interrupted directory, known repository markers and observation intervals.
Paths retain their original bytes. Files larger than 24 MiB, unknown versions,
invalid shapes and SHA-256 mismatches are refused. The digest detects accidental
alteration; it is not a signature. Treat checkpoints as local input from a trusted
scan, and do not edit their contents to bypass a refusal.

A changed recorded directory identity or observed symlink target causes refusal.
Paths still pending are read freshly. Start a fresh scan to observe a refused scope. Completed directories are historical observations:
a repository created inside one between passes need not be found by the resume.
A fresh scan is required to revisit it. Git HEADs and family records are absent
from the checkpoint; discovered candidate families are read again during the
latest pass, subject to its limits.

Text, JSON and HTML retain each discovery interval. Several passes do not form
an atomic snapshot. Neither completion nor a checkpoint authorizes cleanup.
