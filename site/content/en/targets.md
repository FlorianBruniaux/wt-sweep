A target is a complete local ref for the branch against which you want to check integration. No default branch is guessed, and no fetch is performed.

## One family

Use `--repo` with `--target refs/heads/...` or `--target refs/remotes/...` after confirming the intended ref. A remote-tracking ref is only its locally recorded value.

## Several families

Choose roots explicitly and inspect the resulting inventory. A `--target-map` binds full refs to the exact recorded administration paths and device/inode identities. Do not reuse an old identity after a new scan changes it. An empty target list leaves integration unknown.

The [canonical target-map recipe](https://github.com/FlorianBruniaux/wt-sweep/blob/main/docs/targets-and-filters.md) preserves path bytes and refuses existing output files.

## Collect less or display less

| Control | Effect |
| --- | --- |
| `--worktree EXACT_PATH` | Narrows expensive Git and local evidence to a recorded worktree; other rows remain. |
| `--disk-root EXACT_PATH` | Narrows requested disk roots, separately. |
| `--path-prefix PATH`, `--verdict`, `--filter-repo` | Select displayed rows after collection. |

Presentation filters do not save collection work. Even an empty selection retains source coverage and gaps.

## A focused retry

Read the saved gaps first. If standard collection caps are the cause, choose a new output and retry the same exact worktree with `--budget thorough`. Collection remains bounded by higher limits; they do not fix access denial, missing objects or changed identities. Disk has its independent `--disk-budget`.

Read the focused result before widening the scope. Each run keeps its own dates and evidence. Usage by agents and terminals remains a separate check.
