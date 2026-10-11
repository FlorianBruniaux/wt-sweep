// Synthetic report only; requires an already installed Playwright and Chromium.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 4) throw Error('Usage: node tests/html-dashboard.cjs SYNTHETIC_REPORT NEW_OUTPUT_DIR');
const report = path.resolve(process.argv[2]), output = path.resolve(process.argv[3]);
fs.mkdirSync(output, {recursive: false});
(async () => {
  const browser = await chromium.launch({headless: true}), checks = [];
  try {
    for (const javaScriptEnabled of [true, false]) for (const theme of ['light', 'dark']) for (const width of [1440, 390]) {
      const context = await browser.newContext({javaScriptEnabled, colorScheme: theme, reducedMotion: 'reduce', viewport: {width, height: 1000}});
      const page = await context.newPage(), errors = [], requests = [];
      page.on('pageerror', e => errors.push(e.message));
      page.on('request', r => { if (!r.url().startsWith('file:')) requests.push(r.url()); });
      await page.goto(pathToFileURL(report).href);
      const dashboard = page.locator('[data-disk-dashboard]');
      assert.equal(await dashboard.isVisible(), true);
      assert.equal(await dashboard.locator('meter:visible').count(), 2);
      const primary = dashboard.locator('[data-device="7"]');
      assert.equal(await primary.locator('meter').nth(1).getAttribute('value'), '5.0');
      assert.equal(await primary.locator('meter').nth(0).getAttribute('value'), '60.0');
      for (const meter of await primary.locator('meter').all()) assert.ok(await meter.getAttribute('aria-label'));
      assert.equal(await page.locator('#worktree-families').evaluate(e => e.open), true);
      assert.equal(await page.locator('[data-worktree]:visible').count(), 2);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      const before = await dashboard.innerText();
      if (javaScriptEnabled) {
        await page.locator('#search').fill('no-such-dashboard-worktree');
        assert.equal(await dashboard.innerText(), before);
        assert.equal(await page.locator('[data-worktree]:visible').count(), 0);
        await page.locator('#search').fill('');
      }
      const more = dashboard.locator('.other-volumes > summary');
      await more.focus(); await page.keyboard.press('Enter');
      assert.equal(await dashboard.locator('[data-device="8"]').isVisible(), true);
      assert.equal(await dashboard.locator('meter:visible').count(), 4);
      assert.equal(await dashboard.locator('[data-device="8"] meter').nth(1).getAttribute('value'), '2.5');
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1));
      await more.focus(); await page.keyboard.press('Enter');
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.screenshot({path: path.join(output, theme + '-' + width + '-' + (javaScriptEnabled ? 'js' : 'nojs') + '.png')});
      assert.deepEqual(errors, []); assert.deepEqual(requests, []);
      checks.push({theme, width, javaScriptEnabled, observedPercent: '5.0', unavailablePercent: '60.0', noOverflow: true, noNetwork: true});
      await context.close();
    }
    fs.writeFileSync(path.join(output, 'checks.json'), JSON.stringify(checks, null, 2) + '\n');
    console.log('PASS: eight dashboard renders, exact gauges, keyboard volume disclosure, source totals after filtering, no overflow/network');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
