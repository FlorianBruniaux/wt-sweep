import {test} from 'node:test';
import assert from 'node:assert/strict';
import {existsSync} from 'node:fs';
const moduleUrl = new URL('../src/data/updates.mjs', import.meta.url);
test('source banner uses the canonical update date and refuses missing or impossible dates', async () => {
 assert.ok(existsSync(moduleUrl), 'source update metadata is not implemented');
 const {sourceUpdateDate, formatUpdateDate} = await import(moduleUrl);
 assert.equal(sourceUpdateDate('# Changelog\n## [Unreleased]\n<!-- unreleased-updated: 2026-10-10 -->\n### Added\n- A reviewed feature.\n'), '2026-10-10');
 assert.equal(formatUpdateDate('2026-10-10', 'fr'), '10 octobre 2026');
 assert.equal(formatUpdateDate('2026-10-10', 'en'), '10 October 2026');
 for (const date of ['', '2026-02-30', '2026-13-10']) {
  assert.throws(() => sourceUpdateDate(`## [Unreleased]\n<!-- unreleased-updated: ${date} -->\n- A feature.\n`), /valid source update date/);
 }
 assert.throws(() => sourceUpdateDate('## [Unreleased]\n## [1.0] - 2026-10-09\n<!-- unreleased-updated: 2026-10-09 -->\n- Published elsewhere.\n'), /nonempty Unreleased/);
});

test('release banner uses the dated version when Unreleased is empty', async () => {
 const {sourceUpdateDate} = await import(moduleUrl);
 assert.equal(sourceUpdateDate('## [Unreleased]\n\n## [0.1.0-alpha.1] - 2026-10-10\n\n### Added\n- First alpha.\n'), '2026-10-10');
 assert.throws(() => sourceUpdateDate('## [Unreleased]\n\n## [0.1.0-alpha.1] - 2026-02-30\n- First alpha.\n'), /valid source update date/);
});
