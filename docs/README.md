# Documentation

[Project overview](../README.md) · [Changelog](../CHANGELOG.md) · [Contributing](../CONTRIBUTING.md)

User documentation for the read-only macOS MVP, checked against the implementation
on 2026-10-05. The guides are in English; design and validation records remain in
their original French. Dates on those records identify their own evidence window.

## Use the CLI

| Document | Purpose and limit |
| --- | --- |
| [Getting started](getting-started.md) | Build, replay a disposable demo and understand reports; requires macOS. |
| [CLI reference](cli.md) | Implemented commands, output rules, exit codes and collection limits. |
| [Offline forge imports](forge.md) | Import local exports and receipts; no current server-state guarantee. |
| [Evidence and JSON contracts](contracts.md) | Consume artifacts without promoting unknowns or hashes into cleanup authority. |

## Develop and verify

| Document | Purpose and limit |
| --- | --- |
| [Contributing](../CONTRIBUTING.md) | Local checks, fixture ownership and change requirements. |
| [Workflow infographic](assets/README.md) | Gemini workflow illustration, reviewed 2026-10-05; editable source, native outputs and generation provenance. |
| [Architecture](architecture.md) | Current modules and I/O boundaries; no removal engine exists. |
| [Native boundaries](native-boundaries.md) | Three reviewed private unsafe boundaries and their evidence limits. |
| [AI development harness](harness.md) | Canonical instructions and generated host files; file validation does not prove activation. |
| [Implementation results](progress.md) | U1–U4 behavior and the original local validation record. |
| [Finishing ledger](finish-ledger.md) | RETEX additions, final local checks, skips and remaining gates. |

## Design and decisions

| Document | Purpose and limit |
| --- | --- |
| [Acceptance plan](plan.md) | U1–U7 and E1 criteria; a criterion is not a claim of completion. |
| [Implementation and ownership](implementation.md) | Development units, responsibilities and the recorded model allocation. |
| [Decision index](decisions/README.md) | Writer-exclusion counterexamples and the blocked apply boundary. |
| [Cleanup-session lessons](retex-sessions.md) | Sourced lessons and disposable tests; historical shared inventories are not reused. |
| [Related tools and references](references.md) | Candidates for comparison; no behavioral equivalence has been established. |

For product status, start with the README. For an acceptance claim, follow the
plan to its validation record. For future removal, start with E1 in the decision
index; a successful read-only scan does not open that gate.
