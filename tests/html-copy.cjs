// Optional offline browser check with an existing Playwright/Chromium installation.
// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-copy.cjs report.html
'use strict';
const assert = require('node:assert/strict');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 3) throw Error('Usage: node tests/html-copy.cjs report.html');

(async () => {
  const browser = await chromium.launch({headless: true});
  try {
    const page = await browser.newPage({viewport: {width: 390, height: 844}});
    const errors = [], requests = [];
    page.on('pageerror', e => errors.push(e.message));
    page.on('request', r => { if (!r.url().startsWith('file:')) requests.push(r.url()); });
    await page.addInitScript(() => {
      window.copiedPaths = [];
      window.clipboardMode = 'allow';
      Object.defineProperty(navigator, 'clipboard', {configurable: true, value: {
        async writeText(value) {
          if (window.clipboardMode === 'deny') throw Error('Clipboard denied by test');
          window.copiedPaths.push(value);
        },
      }});
    });
    await page.goto(pathToFileURL(path.resolve(process.argv[2])).href);
    const button = page.locator('[data-worktree] [data-copy-path]').first();
    assert.equal(await button.count(), 1, 'worktree needs a copy control');
    assert.equal(await page.locator('a button, summary button, h2 button').count(), 0);
    const snapshot = () => page.locator('[data-worktree]').evaluateAll(rows => rows.map(row => ({
      identity: {...row.dataset}, evidence: Array.from(row.querySelectorAll('pre'), e => e.textContent),
    })));
    const before = await snapshot();
    const hex = await button.getAttribute('data-copy-path');
    const expected = Buffer.from(hex, 'hex').toString('utf8');
    await button.focus();
    await page.keyboard.press('Enter');
    await page.waitForFunction(() => document.getElementById('copy-status').textContent === 'Path copied.');
    assert.deepEqual(await page.evaluate(() => window.copiedPaths), [expected]);
    assert.equal(await button.evaluate(e => e === document.activeElement), true);
    // UTF-8, spaces, quotes, literal backslashes, shell metacharacters and BOM are data.
    const samples = ['/tmp/été & "notes"/résumé', '/tmp/a\\b/$literal;`text`', '\ufeff/tmp/bom'];
    for (const value of samples) {
      await button.evaluate((e, h) => { e.dataset.copyPath = h; }, Buffer.from(value).toString('hex'));
      await button.click();
      assert.equal(await page.evaluate(() => window.copiedPaths.at(-1)), value);
    }
    const copied = await page.evaluate(() => window.copiedPaths.length);
    // Do not substitute U+FFFD or silently normalize newlines/control bytes.
    for (const bytes of [Buffer.from([47, 255]), Buffer.from('/tmp/a\nb'), Buffer.from('/tmp/a\rb'), Buffer.from('/tmp/a\tb'), Buffer.from('/tmp/\u202ebidi'), Buffer.from([47, 0])]) {
      await button.evaluate((e, h) => { e.dataset.copyPath = h; }, bytes.toString('hex'));
      await button.click();
      assert.match(await page.locator('#copy-status').textContent(), /Cannot copy/);
      assert.equal(await page.evaluate(() => window.copiedPaths.length), copied);
    }
    for (const mode of ['deny', 'absent']) {
      const value = '/tmp/été & "manual"/notes';
      await button.evaluate((e, h) => { e.dataset.copyPath = h; }, Buffer.from(value).toString('hex'));
      await page.evaluate(mode => {
        window.clipboardMode = mode;
        if (mode === 'absent') Object.defineProperty(navigator, 'clipboard', {value: undefined});
      }, mode);
      await button.click();
      const dialog = page.locator('#copy-dialog');
      await dialog.waitFor({state: 'visible'});
      const input = page.locator('#copy-value');
      assert.equal(await input.inputValue(), value);
      assert.equal(await input.evaluate(e => e === document.activeElement && e.selectionStart === 0 && e.selectionEnd === e.value.length), true);
      assert.doesNotMatch(await page.locator('#copy-status').textContent(), /^Path copied\./);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth) <= 391);
      await page.keyboard.press('Escape');
      assert.equal(await dialog.isVisible(), false);
      assert.equal(await button.evaluate(e => e === document.activeElement), true);
    }
    await button.evaluate((e, h) => { e.dataset.copyPath = h; }, hex);
    assert.deepEqual(await snapshot(), before);
    await page.locator('#search').fill('copy path');
    assert.equal(await page.locator('[data-worktree]:visible').count(), 0, 'control labels must not affect search');
    assert.deepEqual(errors, []);
    assert.deepEqual(requests, []);
    const nojs = await browser.newPage({javaScriptEnabled: false});
    await nojs.goto(pathToFileURL(path.resolve(process.argv[2])).href);
    assert.equal(await nojs.locator('[data-copy-path]:visible').count(), 0);
    assert.ok(await nojs.locator('[data-worktree]:visible').count() > 0);
    console.log('PASS: exact UTF-8 copy, byte refusals, denied/unavailable clipboard, keyboard/manual fallback, mobile, unchanged evidence, no network, no-JS');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exit(1); });
