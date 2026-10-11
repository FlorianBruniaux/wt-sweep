# Worktrunk dispatch qualification

Worktrunk can already dispatch `wt sweep` to an executable named `wt-sweep` on
`PATH`. This experiment tests that behavior without installing a wrapper, hook or
user configuration. It does not download either program.

Use Python 3.11 or newer and explicit paths to already obtained executables:

```sh
WORKTRUNK_BINARY=/absolute/path/to/wt \
WT_SWEEP_BINARY=/absolute/path/to/wt-sweep \
python3 -B experiments/worktrunk/test_dispatch.py
```

Without both variables, the tests are skipped. Each test owns a temporary root,
a synthetic Git repository, an empty HOME, an explicit Worktrunk configuration,
and an isolated PATH. Git hooks and global/system Git configuration are disabled.
A fixture executable records raw arguments, cwd, streams and exit status. The
actual Sweep binary also performs a scan and a review, including positive clean
candidate and untracked-file retention cases. Files, index and Git metadata are
compared before/after; access times are excluded.

The suite verifies literal metacharacters and non-UTF-8 arguments, inherited and
explicit cwd, separated streams, status 23, help/error forwarding, missing
commands, PATH order, user-alias precedence and built-in precedence. It does not
qualify every Worktrunk release or shell wrapper.

See the [decision and captured binary identities](../../docs/research/worktrunk-integration-2026-10-11.md).
