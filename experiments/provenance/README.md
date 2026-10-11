# Passive provenance experiment

This experiment reads one explicitly selected Aristote schema-v1 card and,
optionally, the selected main checkout's published registry generation. It
preserves declared creator/session context, dates, unknown identity, source
hashes and disagreements. The shipped Sweep CLI does not load this reader.

Run from the repository root, with Python 3.11 or newer:

```sh
python3 -B -m unittest discover -s experiments/provenance -p 'test_*.py' -v
python3 -B experiments/provenance/replay.py
```

All payloads are synthetic. Tests own and remove their temporary Git fixtures.
The real-Git test recreates a worktree at the same path, branch and HEAD, copies
the old card and confirms that identity remains unknown. Git configuration is
isolated; executable hooks are disabled.

To check the fixtures against the original inspected source validator, pass an
existing Aristote code checkout explicitly. Node must already be available:

```sh
python3 -B experiments/provenance/replay.py --source-root /absolute/path/to/aristote-code-checkout
```

This optional check reads the pinned implementation/documentation files in
`source-reference.json`, verifies hashes before and after, and imports the
original `validateCard` function on synthetic input. It reads no actual cards,
registry generations, credentials, environments or conversations.

The experimental reader caps cards at 64 KiB, index documents at 1 MiB,
index entries at 128, and session/PR/problem lists at 32. It refuses non-UTF-8
identity paths, source symlinks, special files and redirected generations.
Freshness describes source age against an explicit 86,400-second display
threshold. It cannot establish activity, integration or writer exclusion.

[Evaluation and decision](../../docs/research/provenance-evaluation-2026-10-11.md).
