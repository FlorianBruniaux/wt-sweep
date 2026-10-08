# Documentation

[Project overview](../README.md) · [Présentation en français](../README.fr.md) · [Changelog](../CHANGELOG.md) · [Contributing](../CONTRIBUTING.md)

These guides cover the read-only macOS MVP and the T01-T09 triage workflow.
The [validation ledger](finish-ledger.md#triage-t01-t09) separates final checks
from historical results. The project overview is available in English and
French, with a French hands-on tutorial. Other detailed user guides are in
English; design and validation records retain their original French and the
dates of their observations.

## Use the CLI

| Document | Contents |
| --- | --- |
| [Tutoriel en français](tutorial.fr.md) | One-command review, disposable trial, tests, advanced replay and a copyable assistant prompt. |
| [Assistant skill (French)](assistant-skill.md) | Claude/Codex invocation, progressive report reading, cleanup suggestions and local verification. |
| [Getting started](getting-started.md) | macOS setup, disposable demo, report interpretation and troubleshooting. |
| [Espace disque](disk-usage.md) | Taille par worktree, Git partagé, total dédupliqué et limites de la mesure. |
| [Rapport HTML par projet](html-report-plan.md) | HTML hors ligne, lecture par famille, thème BoldGuy, unités lisibles, copie des chemins et reçus. |
| [CLI reference](cli.md) | Implemented commands, output rules, exit codes and collection limits. |
| [Targets and filters](targets-and-filters.md) | Select refs per repository and inspect a subset while retaining source evidence. |
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
| [Collection measurements](collection-performance.md) | Discovery throughput, reproducible Rust benchmark, budget profiles, phase metrics and observed coverage. |
| [Collection benchmark, 07/10](research/collection-benchmark-2026-10-07.md) | Before/after release samples at 50 and 100 worktrees, first provisional fact, comparable evidence and rejected measurements. |
| [Review benchmark runner](../scripts/benchmark-review.py) | Frozen binaries, synthetic Git families, first observed fact, pipe/PTY output and conservation checks. Results need their binary hashes and evidence coverage. |
| [Implementation results](progress.md) | Current checkpoint links followed by the dated U1-U4 validation record. |
| [Recette réelle multi-projets, 07/10](research/multi-project-acceptance-2026-10-07.md) | 69 registered worktrees, matching text/JSON/HTML, partial disk and local coverage, and delivery limits. |
| [Finishing ledger](finish-ledger.md) | Historical RETEX checkpoint, current triage checks, repository trial and remaining gates. |

## Design and decisions

| Document | Contents |
| --- | --- |
| [Roadmap](roadmap.md) | Delivered scope, large-repository collection limits, next validation steps and separate removal gates. |
| [Plan de tri des worktrees, v2](superpowers/plans/2026-10-05-worktree-triage-v2.md) | Completed T01-T09 plan, decision conditions, acceptance checks and conditional extensions. |
| [Critique du plan et de l'architecture](superpowers/plans/2026-10-05-worktree-triage-review.md) | Findings against the first proposal and current code, alternatives, failure modes and corrections adopted in v2. |
| [Acceptance plan](plan.md) | U1–U7 and E1 acceptance criteria. |
| [Implementation and ownership](implementation.md) | Development units, responsibilities and the recorded model allocation. |
| [Decision index](decisions/README.md) | Failed writer-exclusion experiments and the decision to keep `apply` unavailable. |
| [Cleanup-session lessons](retex-sessions.md) | Sourced lessons and disposable tests; historical shared inventories are not reused. |
| [Worktree source analysis, 07/10](worktree-source-analysis-2026-10-07.md) | Twelve cloned tools, implementation mechanisms, UI/CLI ideas, specialist notes and bounded reuse decisions. |
| [Worktree ecosystem review, 07/10](worktree-ecosystem-2026-10-07.md) | Direct competitors, lifecycle managers, evidence limits and reuse experiments before expanding scope. |
| [Controlled runtime comparison](research/worktree-runtime-comparison-2026-10-07.md) | 48 synthetic Sweep, Holt and AgentRinse audits; native verdicts, writing effects and decisions by capability. |
| [Related tools and references](references.md) | Dated source readings, runtime evidence and limits of reuse. |

The [roadmap](roadmap.md) records delivered scope and remaining work; the
[project README](../README.md#roadmap-and-validation) summarizes current status.
Read the [validation ledger](finish-ledger.md) for test results and
the [apply decision](decisions/apply-readiness.md) for the unmet requirements
that keep removal unavailable.
