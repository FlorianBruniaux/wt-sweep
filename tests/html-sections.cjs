// Optional offline check using an existing Playwright/Chromium installation.
// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-sections.cjs report.html NEW_OUTPUT_DIR
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 4) throw Error('Usage: node tests/html-sections.cjs report.html NEW_OUTPUT_DIR');
const url = pathToFileURL(path.resolve(process.argv[2])).href;
const output = path.resolve(process.argv[3]);
fs.mkdirSync(output, {recursive: false});
(async () => {
  const browser = await chromium.launch({headless: true});
  try {
    for (const js of [true, false]) for (const theme of ['light', 'dark']) for (const width of [1440, 390]) {
      const page = await browser.newPage({javaScriptEnabled: js, colorScheme: theme, reducedMotion: 'reduce', viewport: {width, height: 1000}});
      const errors = [], requests = [];
      page.on('pageerror', e => errors.push(e.message));
      page.on('request', r => { if (!r.url().startsWith('file:')) requests.push(r.url()); });
      await page.goto(url);
      const sections = page.locator('details.report-section');
      const ids = ['disk-priority', 'overview', 'worktree-families', 'disk', 'coverage'];
      assert.deepEqual(await sections.evaluateAll(es => es.map(e => e.id)), ids);
      const states = () => sections.evaluateAll(es => es.map(e => e.open));
      assert.deepEqual(await states(), ids.map(() => true));
      const snapshot = () => page.evaluate(() => ({
        rows: Array.from(document.querySelectorAll('[data-worktree]'), e => ({...e.dataset})),
        evidence: Array.from(document.querySelectorAll('pre'), e => e.textContent),
      }));
      const before = await snapshot();
      for (const id of ids) {
        const section = page.locator('#' + id), summary = section.locator(':scope > summary');
        await summary.focus();
        await page.keyboard.press('Enter');
        assert.equal(await section.evaluate(e => e.open), false);
        assert.equal(await section.locator(':scope > .section-body').isVisible(), false);
        // Native details can briefly show Chromium's default ring before style settles.
        const deadline = Date.now() + 2000;
        let focus = await summary.evaluate(e => getComputedStyle(e).outlineWidth);
        while (focus !== '2px' && Date.now() < deadline) {
          await new Promise(resolve => setTimeout(resolve, 50));
          focus = await summary.evaluate(e => getComputedStyle(e).outlineWidth);
        }
        assert.equal(focus, '2px', `${js}/${theme}/${width}/${id}: focus ring`);
        await page.keyboard.press('Space');
        assert.equal(await section.evaluate(e => e.open), true);
        assert.ok((await summary.boundingBox()).height >= 44);
      }
      const nested = page.locator('tbody details').first();
      if (await nested.count()) await nested.locator(':scope > summary').click();
      const nestedBefore = await page.locator('details:not(.report-section)').evaluateAll(es => es.map(e => e.open));
      if (js) {
        await page.locator('#collapse-sections').click();
        assert.deepEqual(await states(), ids.map(() => false));
        assert.equal(await page.locator('.notice').first().isVisible(), true);
        for (const id of ['overview', 'worktree-families', 'disk', 'coverage']) {
          await page.locator(`.sidebar a[href="#${id}"]`).click();
          assert.equal(await page.locator('#' + id).evaluate(e => e.open), true);
          // Clicking the same fragment again must reopen it too.
          await page.locator(`#${id} > summary`).click();
          await page.locator(`.sidebar a[href="#${id}"]`).click();
          assert.equal(await page.locator('#' + id).evaluate(e => e.open), true);
        }
        await page.locator('#collapse-sections').click();
        await page.mouse.move(width - 1, 999);
        await page.locator('#expand-sections').focus();
        await page.screenshot({path: path.join(output, `${theme}-${width}-collapsed.png`)});
        await page.locator('#expand-sections').click();
        assert.deepEqual(await states(), ids.map(() => true));
        assert.deepEqual(await page.locator('details:not(.report-section)').evaluateAll(es => es.map(e => e.open)), nestedBefore);
        await page.locator('#search').fill('no-such-worktree-sections-check');
        assert.equal(await page.locator('[data-worktree]:visible').count(), 0);
        await page.locator('#search').fill('');
      } else {
        assert.equal(await page.locator('#collapse-sections').isVisible(), false);
        assert.equal(await page.locator('#expand-sections').isVisible(), false);
        for (const id of ids) await page.locator(`#${id} > summary`).click();
      }
      // Printing must retain section titles and contents even after folding them.
      await sections.evaluateAll(es => es.forEach(e => { e.open = false; }));
      await page.emulateMedia({media: 'print'});
      for (const id of ids) {
        assert.equal(await page.locator(`#${id} > summary`).isVisible(), true);
        assert.equal(await page.locator(`#${id} > .section-body`).isVisible(), true);
      }
      await page.emulateMedia({media: 'screen'});
      assert.deepEqual(await states(), ids.map(() => false));
      assert.deepEqual(await snapshot(), before);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth) <= width + 1);
      if (js) {
        await page.goto(url + '#family-0');
        assert.equal(await page.locator('#worktree-families').evaluate(e => e.open), true);
        assert.ok(await page.locator('#family-0').isVisible());
      }
      assert.deepEqual(errors, []);
      assert.deepEqual(requests, []);
      await page.close();
    }
    console.log('PASS: 8 section variants; keyboard/native toggles, bulk actions, repeated anchors, nested state, filters, print, unchanged evidence, no network/overflow');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
