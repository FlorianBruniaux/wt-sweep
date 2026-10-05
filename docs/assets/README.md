# Workflow infographic

[Documentation index](../README.md) · [Project overview](../../README.md)

Generated with Gemini 3 Pro Image and reviewed on 2026-10-05. The third response
is accepted and embedded in the project README. The two earlier drafts remain
recorded with their defects.

![Worktree Sweep workflow: explicit scope, inventory, Git checks and review; a matching HEAD can coexist with local notes that need preservation.](worktree-sweep-workflow.jpg)

## Intent and source claims

Explain the read-only journey from explicit scope to inventory, Git relationship
checks and review. The separate example shows a HEAD matching an integration
branch while an untracked local note remains. This follows the
[disposable demo](../getting-started.md#disposable-demo).

The optional forge input is a captured export. Files and index are optional
inventory evidence. Unknowns remain visible, and backup, restore and removal
remain unavailable. The [CLI](../cli.md) and [evidence contracts](../contracts.md)
define these limits; the illustration does not replace them.

The visual references were the workflow graphic in
[yt-insights](https://github.com/FlorianBruniaux/youtube-video-insights) and the
terminal presentation in Agent Router, a private repository inspected locally.
The yt-insights reference was also inspected locally. Only the text brief was
sent to Gemini. The accepted render uses a navy header and white content area,
a cosmetic departure from the fully dark brief that preserves clear contrast.

## Editable sources and provenance

- [Accepted generation brief](worktree-sweep-workflow-v2.prompt.md): closed text
  inventory, layout, four directed edges and product boundaries.
- [Original brief](worktree-sweep-workflow.prompt.md): retained for attempts 1–2.
- [Mermaid source](worktree-sweep-workflow.mmd): editable logical content and the
  separate local-preservation example.
- [Generation record](generation.json): model and CLI hash, prompt hashes, call
  authorization, native hashes, outcomes, metadata, QA and final image hash.
- [Delivered JPEG](worktree-sweep-workflow.jpg): 1376 × 768 pixels, byte-identical
  to the accepted native response. No crop, stretch, upscale or retouching.

## Observed review

The accepted image contains the 23 specified labels once, with no added textual
annotations. Classification labels sit below the empty report pictogram. The
four arrows have the specified direction and endpoints; the local example has
no connector to the workflow. All text was checked at native size and at 960px
README width. This is an assistant visual check, not an independent review or a
runtime validation of the product.

The request specified 16:9; the provider returned 1376 × 768. Native dimensions
are retained. A smaller copy was used only for the readability check.

## Attempt history and cost

| Attempt | Native output | Outcome | Estimated image output |
| --- | --- | --- | --- |
| 1 | [JPEG, 1376 × 768](history/workflow-attempt-01.jpg) | Rejected: added HEAD and commit annotations in the example icon. | $0.134 |
| 2 | [JPEG, 1376 × 768](history/workflow-attempt-02.jpg) | Rejected: duplicated classification labels inside the icon; Active missing from the main rows. | $0.134 |
| 3 | [JPEG, 1376 × 768](history/workflow-attempt-03.jpg) | Accepted after an explicitly authorized extra call and a revised standalone brief. | $0.134 |

Attempt 1 was renamed from .png to .jpg to match its actual MIME; its bytes were
unchanged. Both original briefs and all three native responses are preserved.
No paid attempt remains authorized.

Google lists standard image output at $0.134 for 1K/2K on the
[official pricing page](https://ai.google.dev/gemini-api/docs/pricing), checked on
2026-10-05. The latest response adds an estimated $0.134; cumulative image output
is estimated at $0.402. Input, text/thinking, grounding and taxes are excluded.
Actual billed usage is unknown because the CLI does not expose provider usage
metadata. Three final image responses are observed; that count is not a bill.
