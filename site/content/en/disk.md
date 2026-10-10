Disk size helps prioritize a review. It does not contribute to cleanup verdicts.

## Scope and budgets

Add `--disk-usage` to `scan` or `review`. By default, all inventoried worktree roots and shared Git directories are requested. `--worktree` and presentation filters do not narrow this disk collection.

Use repeated `--disk-root EXACT_ABSOLUTE_PATH` to choose recorded roots. Include the shared Git directory explicitly if you want it measured. Unknown paths are refused.

`--disk-budget standard|thorough` is separate from the Git/local `--budget`. Start with standard; higher limits can still leave gaps. `plan --inventory` retains captured sizes and observation dates without recounting them.

The console recap uses decimal KB/MB/GB and lists the five largest selected worktrees by observed size. Partial measurements may under-rank them; rows without a size remain explicit.

## Read the values

| Value | Meaning |
| --- | --- |
| Logical | Regular-file and symlink lengths. Sparse files can be much longer than their allocation. |
| Allocated | Filesystem-reported blocks multiplied by 512, including directory blocks. |
| Shared Git | Common Git administration, measured separately. |
| Deduplicated total | Each observed device/inode pair counted once. Row totals can overlap. |
| Complete | This root finished without a detected gap. Not an atomic snapshot. |
| Partial | Only accumulated observations are available. |
| Not requested | Recorded root outside the explicit disk selection. |
| Not measured | No available size, not zero. |

Hidden and ignored files remain in scope. Symlink targets are not followed. Nested registered roots have their own rows.

## What these sizes do not establish

Allocated size is not recoverable space. APFS clones, snapshots and shared extents can change what a deletion would free. External alternates and dependencies outside measured roots are not counted.

The current collector reads metadata one entry at a time. The native `getattrlistbulk` evaluation is not integrated. There is no cache-category breakdown or automatic cache deletion.

[Choose a focused review](/docs/targets/) and preserve each run’s scope and dates.
