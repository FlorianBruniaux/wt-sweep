# wt-sweep site

Astro 5 + Starlight, EN/FR, local-first delivery. Product base: c3e38f7810424ae57f58d6ea10c4db996e9122fb. Design: Search Console landing architecture, BoldGuy tokens.

[Observed validation and reference status](VERIFICATION.md).

The header, source-updates banner and hero share the Search Console 90rem envelope. Standard sections retain a 75rem container and prose keeps its own width limit. The banner reads the reviewed canonical changelog; maintain its `unreleased-updated` marker when adding source changes. It never substitutes the build date or advertises a published release.

## Run

Node 22+, pnpm 9, Python 3.11+ for fixture tests. Build the CLI from the repository root with `cargo build --locked --offline` before running the native-demo tests. Install locked packages with `pnpm install --frozen-lockfile`.

```sh
cd site
pnpm dev --host 127.0.0.1 --port 49331
pnpm verify
```

No public hostname is assumed. Local builds emit noindex, disallow crawling and omit XML sitemap. Set `SITE_URL` to the approved HTTPS origin when preparing a public build; this does not deploy it. Canonical, alternate and social URLs use that origin.

## Documentation

`published-pages.json` is the explicit allowlist. EN/FR portal summaries live in `content/` and link canonical repository docs. Hashes bind each reviewed source; preparation refuses stale sources before touching generated files. When editing canonical docs, review both language summaries before updating their sourceHash. Legal text and the changelog retain their canonical English. `src/content/docs/` and `src/data/product.json` are generated, ignored files.

Screenshots retain the separate 2026-10-08 synthetic provenance. `scripts/generate-demo.py` creates a new owned disposable Git family using the compiled local CLI and verifies artifact receipts and fixture conservation; it never scans real roots. `public/demo` contains that native report and sanitized generation records. Regenerate deliberately with `python3 -B scripts/generate-demo.py`; the tests generate their own temporary output. If `python3` resolves to Apple’s older interpreter, put your existing Python 3.11+ first in PATH or set `WT_SITE_PYTHON` for the test suite.

No analytics, external font, form collection, browser CLI execution, automatic deletion or deployment.
