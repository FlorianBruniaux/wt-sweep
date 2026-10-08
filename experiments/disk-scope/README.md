# Fixed-corpus disk comparison

Run two already-built release binaries on the same owned fixtures. The script
creates one repository and four linked worktrees for each corpus size: 20, 200
and 2,000 ordinary files per linked worktree. Fixture creation is outside timing.
Each binary gets one warm-up followed by three alternating measured samples.

```sh
python3 experiments/disk-scope/benchmark.py \
  --baseline /absolute/path/to/previous/wt-sweep \
  --candidate ./target/release/wt-sweep \
  --out /private/tmp/wt-sweep-disk-comparison.json
```

Use a new output path. Run after other test suites finish. Git, Python and the
binaries must already be available; the script installs nothing. Git commands
operate only on uniquely owned temporary repositories, which are removed when
the script exits. No real repository or machine-wide scan is run.

An independent `lstat` walk checks per-root logical and allocated bytes and the
inode-deduplicated total for every sample. Source path sets, file bytes, modes,
sizes, mtimes and ctimes must remain unchanged. Atime is excluded. Every measured
root must complete; a refusal or partial result fails the comparison.

Disk timing comes from each CLI's monotonic measurement; wall time includes
Git discovery and JSON output. The OS cache is not cleared. Small millisecond
samples are coarse, and three samples do not establish a general speedup.
This corpus does not simulate slow system calls or APFS reclaimable storage.

## Observed on 08 October 2026

[Captured samples and fingerprints](result-2026-10-08.json), comparing the
`main` base `9d69df9` with the disk-selection change:

| Files per linked worktree | Entries | Previous median | Candidate median |
| --- | ---: | ---: | ---: |
| 20 | 174 | 1 ms | 1 ms |
| 200 | 894 | 2 ms | 2 ms |
| 2,000 | 8,094 | 18 ms | 19 ms |

Every sample completed all six roots, matched the independent byte counts and
preserved the fixture. No general speedup is demonstrated. The change provides
explicit scope and budget distribution; it does not replace metadata syscalls.
