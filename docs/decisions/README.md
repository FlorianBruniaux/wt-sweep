# Decision records

[Documentation index](../README.md) · [Acceptance plan](../plan.md) · [Architecture](../architecture.md)

Writer exclusion is **not demonstrated**. These French records preserve the
tested mechanisms, their limits and the conditions for further work. Index
reviewed on 2026-10-05; experiment dates remain in each record.

| Record | Status and scope |
| --- | --- |
| [Exclusion et conservation](exclusion-et-conservation.md) | Counterexamples executed: cooperative locks, renames and cwd checks do not exclude every writer. |
| [Apply readiness](apply-readiness.md) | Native capability probe recorded; U5–U7 remain blocked until another authority boundary satisfies E1. |

The [cleanup-session lessons](../retex-sessions.md) add disposable witnesses about
ignored files, index state and open descriptors. They inform the read-only
manifests and reports, without authorizing real cleanup.
