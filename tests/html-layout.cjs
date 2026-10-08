// Optional regression check on a saved report containing at least one disk gap.
// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-layout.cjs report.html NEW_OUTPUT_DIR
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 4) throw Error('Usage: node tests/html-layout.cjs report.html NEW_OUTPUT_DIR');
const report = path.resolve(process.argv[2]), output = path.resolve(process.argv[3]);
fs.mkdirSync(output, {recursive:false});
(async () => {
  const browser = await chromium.launch({headless:true}), checks = [];
  try {
    for (const theme of ['light','dark']) for (const width of [1440,390]) {
      const page = await browser.newPage({colorScheme:theme, reducedMotion:'reduce', viewport:{width,height:1000}});
      const errors = [], requests = [];
      page.on('pageerror', error => errors.push(error.message));
      page.on('request', request => { if (!request.url().startsWith('file:')) requests.push(request.url()); });
      await page.addInitScript(() => Object.defineProperty(navigator, 'clipboard', {value: {
        async writeText(value) { window.copiedPath = value; },
      }}));
      await page.goto(pathToFileURL(report).href);
      const group = page.locator('#disk .copyable-details').filter({has:page.locator('li code')}).first();
      assert.equal(await group.count(), 1, 'fixture needs a disk measurement with a path-specific gap');
      const details = group.locator(':scope > details');
      const summary = details.locator(':scope > summary');
      const rootCopy = group.locator(':scope > [data-copy-path]');
      const expected = Buffer.from(await rootCopy.getAttribute('data-copy-path'), 'hex').toString('utf8');
      await rootCopy.click();
      assert.equal(await page.evaluate(() => window.copiedPath), expected);
      assert.equal(await details.evaluate(e => e.open), false, 'copy must not toggle disclosure');
      await summary.focus();
      await page.keyboard.press('Enter');
      assert.equal(await details.evaluate(e => e.open), true);
      const metrics = await group.evaluate(group => {
        const rect = e => { const r = e.getBoundingClientRect(); return {left:r.left, right:r.right, top:r.top, bottom:r.bottom, width:r.width, height:r.height}; };
        const gaps = Array.from(group.querySelectorAll('li')).filter(li => li.querySelector('code') && li.querySelector('button')).map(li => {
          const code = li.querySelector('code'), button = li.querySelector('button');
          const walker = document.createTreeWalker(li, NodeFilter.SHOW_TEXT);
          const nodes = []; let node;
          while ((node = walker.nextNode())) if (node.textContent.trim() && !code.contains(node) && !button.contains(node)) nodes.push(node);
          if (!nodes.length) throw Error('gap reason missing');
          const range = document.createRange(); range.setStart(nodes[0],0); range.setEnd(nodes.at(-1),nodes.at(-1).textContent.length);
          const reason = range.getBoundingClientRect();
          return {code:rect(code), button:rect(button), reason:{top:reason.top, bottom:reason.bottom}, row:rect(li)};
        });
        return {gaps, group:rect(group), details:rect(group.querySelector('details')), scroll:document.documentElement.scrollWidth};
      });
      assert.ok(metrics.gaps.length > 0);
      for (const gap of metrics.gaps) {
        assert.ok(gap.reason.top >= Math.max(gap.code.bottom,gap.button.bottom)+6, 'gap reason must be on its own line below path and copy control');
        assert.ok(gap.button.left >= gap.code.right+6 || gap.button.top >= gap.code.bottom+6, 'path and copy control need a visible gap');
        assert.ok(gap.button.width>=44 && gap.button.height>=44, 'copy target below 44px');
        assert.ok(gap.button.right <= metrics.group.right+1, 'copy control escapes its group');
      }
      assert.ok(metrics.details.width >= metrics.group.width-2, 'expanded details must use the full group width');
      assert.ok(metrics.scroll<=width+1, 'document overflow');
      await summary.scrollIntoViewIfNeeded();
      await page.evaluate(() => window.scrollBy(0,-24));
      await page.screenshot({path:path.join(output,`${theme}-${width}.png`)});
      await rootCopy.click();
      assert.equal(await details.evaluate(e => e.open), true, 'copy must preserve expanded state');
      await summary.focus(); await page.keyboard.press('Enter');
      assert.equal(await details.evaluate(e => e.open), false);
      assert.deepEqual(errors,[]); assert.deepEqual(requests,[]);
      checks.push({theme,width,...metrics});
      await page.close();
    }
    fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2)+'\n');
    console.log('PASS: four disk-gap layouts, full-width evidence, spacing, copy without disclosure changes, keyboard, no overflow or network');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exit(1); });
