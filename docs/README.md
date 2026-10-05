# Documentation

[Project overview](../README.md) · [Changelog](../CHANGELOG.md) · [Contributing](../CONTRIBUTING.md)

These guides cover the read-only macOS MVP and were checked against the
implementation on 2026-10-05. User guides are in English; design and validation
records retain their original French and the dates of their observations.

## Use the CLI

| Document | Contents |
| --- | --- |
| [Getting started](getting-started.md) | macOS setup, disposable demo, report interpretation and troubleshooting. |
| [CLI reference](cli.md) | Implemented commands, output rules, exit codes and collection limits. |
| [Offline forge imports](forge.md) | Supported exports, capture receipts and limits of MR/PR association. |
| [Evidence and JSON contracts](contracts.md) | Artifact fields, classifications, path encoding, digests and receipts. |

## Develop and verify

| Document | Contents |
| --- | --- |
| [Contributing](../CONTRIBUTING.md) | Local checks, fixture ownership and change requirements. |
| [README illustrations](assets/README.md) | Editable SVG example, previous Gemini workflow, sources and review notes. |
| [Architecture](architecture.md) | Module responsibilities, data flow, read boundaries and test strategy. |
| [Native boundaries](native-boundaries.md) | Three reviewed private unsafe boundaries and their evidence limits. |
| [AI development harness](harness.md) | Canonical instructions, generated host files and separate checks for native activation. |
| [Implementation results](progress.md) | U1–U4 behavior and the original local validation record. |
| [Finishing ledger](finish-ledger.md) | RETEX additions, final local checks, skips and remaining gates. |

## Design and decisions

| Document | Contents |
| --- | --- |
| [Acceptance plan](plan.md) | U1–U7 and E1 acceptance criteria. |
| [Implementation and ownership](implementation.md) | Development units, responsibilities and the recorded model allocation. |
| [Decision index](decisions/README.md) | Failed writer-exclusion experiments and the decision to keep `apply` unavailable. |
| [Cleanup-session lessons](retex-sessions.md) | Sourced lessons and disposable tests; historical shared inventories are not reused. |
| [Related tools and references](references.md) | Candidates for comparison; no behavioral equivalence has been established. |

The [project README](../README.md#roadmap-and-validation) summarizes current
status. Read the [validation ledger](finish-ledger.md) for test results and
the [apply decision](decisions/apply-readiness.md) for the unmet requirements
that keep removal unavailable.
