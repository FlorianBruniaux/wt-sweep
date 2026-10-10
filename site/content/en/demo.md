This trial creates a new Git family in a uniquely owned temporary directory. It touches only that fixture and saves reports outside its repository. Run it from the built wt-sweep checkout on macOS.

## Create and review the fixture

```sh
(
  set -eu
  export GIT_CONFIG_NOSYSTEM=1 GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_COUNT=0
  unset GIT_DIR GIT_WORK_TREE GIT_COMMON_DIR GIT_INDEX_FILE
  unset GIT_OBJECT_DIRECTORY GIT_ALTERNATE_OBJECT_DIRECTORIES

  demo_dir="$(mktemp -d /private/tmp/wt-sweep-demo.XXXXXX)"
  mkdir "$demo_dir/reports"
  git init --template= --initial-branch=main "$demo_dir/repo"
  printf 'Disposable worktree triage demo\n' > "$demo_dir/repo/README.txt"
  git -C "$demo_dir/repo" add README.txt
  git -C "$demo_dir/repo" -c user.name=Demo -c user.email=demo@example.invalid \
    -c commit.gpgSign=false commit -m "Demo baseline"
  git -C "$demo_dir/repo" worktree add -b demo "$demo_dir/worktree"
  git -C "$demo_dir/repo" worktree add -b clean-demo "$demo_dir/clean-worktree"
  printf 'Local note, absent from the commit\n' > "$demo_dir/worktree/notes.txt"

  ./target/release/wt-sweep review --repo "$demo_dir/repo" \
    --target refs/heads/main --disk-usage --summary --html --run-dir "$demo_dir/reports"
  printf 'Demo and reports: %s\n' "$demo_dir"
)
```

The command prints the temporary directory and report paths. Open the saved `report.html` in your browser.

## Expected reading

| Fixture | Verdict | Why |
| --- | --- | --- |
| `repo` | `keep` | Main checkout, the repository entry point. |
| `worktree` | `keep` | `notes.txt` is an untracked local note. |
| `clean-worktree` | `cleanup_candidate` | Current C1–C5 conditions established on a complete run. |

All three HEADs match `refs/heads/main`. The local note still needs preserving. The candidate’s usage remains unchecked. If evidence collection is partial, retain its gaps and follow the reported conditions rather than forcing this expected result.

## Browse an already generated example

[Open the interactive native report](/demo/report.html). That separate website fixture contains four worktrees: three `keep`, one `cleanup_candidate`. Its report comes from a temporary test repo, with [generation provenance](/demo/provenance.json).

The landing screenshots were captured on 8 October 2026 from another four-worktree fixture. Their [capture record](/assets/report-captures.json) is separate from the interactive example.

No cleanup runs in either demo. A receipt binds artifact bytes; it is not a backup.
