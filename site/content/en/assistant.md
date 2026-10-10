The assistant explains CLI evidence. It does not replace deterministic verdicts or authorize removal. The CLI runs without a model; assisted reading uses your existing client session.

## Start in the right checkout

Open a session in the wt-sweep checkout containing the repository skill. The repository being reviewed can be elsewhere. A session opened only in that other repository may not have this skill.

| Client | Invocation | Repository file |
| --- | --- | --- |
| Claude Code | `/wt-sweep` | `.claude/skills/wt-sweep/SKILL.md` |
| Codex | `$wt-sweep` | `.agents/skills/wt-sweep/SKILL.md` |

Native discovery depends on client version and project trust. After adding the skill, open a fresh session and inspect its skill selector. File validation does not prove native activation.

## Begin with the demo

```text
$wt-sweep Read docs/getting-started.md and verify the compiled CLI.
Run only its disposable demo, then explain the saved HTML report.
Preserve the original verdicts and coverage gaps.
Do not scan my machine or remove anything.
```

Replace `$wt-sweep` with `/wt-sweep` for Claude Code.

## Read an existing report

```text
$wt-sweep Read the report folder /absolute/path/to/run-existing.
Do not launch a new scan. Verify artifacts against run.json.
Give the scope and coverage, then review five worktrees at a time.
For each verify, name the missing condition and next useful check.
Distinguish recorded observations from current state, which was not rechecked.
```

The path must identify the `run-*` directory containing the receipt and artifacts. A historical report cannot decide a present-day deletion. [Focused reviews](/docs/targets/) may gather a missing proof in a new run.
