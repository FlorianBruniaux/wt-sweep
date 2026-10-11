// Real offline DOM behavior on a synthetic report; no installed dependencies are modified.
'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
if(process.argv.length!==4)throw Error('Usage: node tests/html-command.cjs SYNTHETIC_REPORT NEW_OUTPUT_DIR');
const output=path.resolve(process.argv[3]);fs.mkdirSync(output,{recursive:false});
(async()=>{const browser=await chromium.launch({headless:true});const checks=[];try{
 for(const js of [true,false])for(const theme of ['light','dark'])for(const width of [1440,390]){
  const page=await browser.newPage({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:950}});
  const errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith('file:'))requests.push(r.url());});
  await page.goto(pathToFileURL(path.resolve(process.argv[2])).href);
  const before=await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset})));
  const dashboard=await page.locator('[data-disk-dashboard]').innerText();
  const dialog=page.locator('#command-dialog');
  if(js){
   await page.keyboard.press('Meta+k');
   assert.equal(await dialog.isVisible(),true,'Command-K must open report search');
   const input=page.locator('#command-query');
   assert.equal(await input.evaluate(e=>e===document.activeElement),true);
   await input.fill('scan limits');await input.press('ArrowDown');await input.press('Enter');
   assert.equal(await dialog.isVisible(),false);
   assert.equal(await page.locator('#coverage').evaluate(e=>e.open),true);
   assert.equal(await page.locator('.scan-technical').evaluate(e=>e.open),false);
   assert.equal(await page.locator('.scan-technical .path-gap:visible').count(),0);
   await page.locator('#search').fill('no-such-visible-worktree');
   await page.locator('#verdict').selectOption('absent');
   await page.keyboard.press('Control+k');assert.equal(await dialog.isVisible(),true);
   await input.fill('no-such-command-result');assert.equal(await page.locator('#command-results button').count(),0);
   assert.ok((await page.locator('#command-status').textContent()).includes('No matches'));
   await input.fill('project');assert.ok(await page.locator('#command-results button').count()>=2);
   await input.press('ArrowDown');await input.press('Enter');
   assert.equal(await dialog.isVisible(),false);
   assert.equal(await page.locator('#search').inputValue(),'');assert.equal(await page.locator('#verdict').inputValue(),'');
   assert.equal(await page.locator('[data-worktree]:visible').count(),before.length);
   assert.equal(await page.locator('[data-worktree]').evaluateAll(es=>es.some(e=>e===document.activeElement)),true);
   assert.equal(await page.locator('#worktree-families').evaluate(e=>e.open),true);
   await page.locator('#command-open').click();await page.keyboard.press('Escape');
   assert.equal(await dialog.isVisible(),false);assert.equal(await page.locator('#command-open').evaluate(e=>e===document.activeElement),true);
   assert.equal(await page.locator('[data-disk-dashboard]').innerText(),dashboard);
   assert.deepEqual(await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset}))),before);
  }else{
   assert.equal(await dialog.isVisible(),false);assert.equal(await page.locator('#command-open').isVisible(),false);
  }
  const guide=page.locator('#reading-guide');await guide.locator(':scope > summary').click();
  assert.equal(await guide.locator('.guide-diagram svg').isVisible(),true);
  assert.ok((await guide.innerText()).includes('This report is an inventory'));
  assert.equal(await page.locator('.nav-item svg[aria-hidden="true"]').count(),4);
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
  await guide.scrollIntoViewIfNeeded();await page.screenshot({path:path.join(output,`${theme}-${width}-${js?'js':'nojs'}.png`)});
  assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);checks.push({js,theme,width,rows:before.length});await page.close();
 }
 fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2));
 console.log('PASS: 8 command/guide variants, keyboard search, filter reveal, focus return, offline SVG, closed diagnostics, source conservation');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exit(1)});
