// Real rendered synthetic reports: user can reach a compact list, inspect details,
// and change the view without altering captured observations.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if(process.argv.length !== 5) throw Error('Usage: html-ux.cjs INVENTORY REVIEW NEW_OUTPUT');
(async()=>{
 const output=path.resolve(process.argv[4]); fs.mkdirSync(output,{recursive:false});
 const browser=await chromium.launch({headless:true}); const checks=[];
 try {
  for(const [mode,file] of [['inventory',process.argv[2]],['review',process.argv[3]]])
  for(const js of [true,false]) for(const theme of ['light','dark']) for(const width of [1440,390]) {
   const page=await browser.newPage({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:1000}});
   const errors=[],network=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith('file:'))network.push(r.url());});
   await page.goto(pathToFileURL(path.resolve(file)).href);
   assert.equal(await page.locator('.toolbar').isVisible(),js,'unavailable search/filter controls must be hidden without JavaScript');
   const list=page.locator('#worktree-families');
   assert.equal(await list.evaluate(e=>e.open),true);
   const actions=page.locator('#recommended-actions');
   assert.equal(await actions.isVisible(),true,'recommended next steps must be visible on arrival');
   for(const href of await actions.locator('a').evaluateAll(es=>es.map(e=>e.getAttribute('href')))) {
    assert.equal(await page.locator(href).count(),1,'each action must lead to an existing section');
   }
   // Check the initial position, before keyboard actions can scroll the page.
   // Budget: one desktop viewport, or 1.8 mobile viewports for stacked controls.
   const first=page.locator('[data-worktree]').first();
   const firstTop=await first.evaluate(e=>e.getBoundingClientRect().top);
   assert.ok(firstTop < (width===1440?1000:1800), `${mode}/${width}/js=${js}: first entry at ${firstTop}`);
   if(mode==='inventory') {
    const example=page.locator('#overview .review-command');
    assert.equal(await example.evaluate(e=>e.open),false);
    await example.locator('summary').focus();await page.keyboard.press('Enter');
    assert.equal(await example.locator('code').isVisible(),true,'the command example must work with and without JavaScript');
    await example.locator('summary').focus();await page.keyboard.press('Enter');
   }
   await page.evaluate(()=>scrollTo(0,0));
   fs.writeFileSync(path.join(output,`${mode}-${theme}-${width}-${js?'js':'nojs'}-layout.json`),JSON.stringify(await page.evaluate(()=>Array.from(document.querySelectorAll('.top,#overview,#overview>.section-body>*:not(details),#worktree-families,.toolbar,.list-shortcuts,.family-header,.family-index'),e=>({tag:e.tagName,cls:e.className,top:e.getBoundingClientRect().top,height:e.getBoundingClientRect().height}))),null,2));
   await page.screenshot({path:path.join(output,`${mode}-${theme}-${width}-${js?'js':'nojs'}-arrival.png`),fullPage:true});
   fs.writeFileSync(path.join(output,`${mode}-${theme}-${width}-${js?'js':'nojs'}-accessibility.yml`),await page.locator('body').ariaSnapshot());
   // Technical sections do not precede the primary list on arrival.
   for(const id of ['disk','coverage']) assert.equal(await page.locator('#'+id).evaluate(e=>e.open),false);
   assert.equal(await page.locator('.overview-actions .primary-action').isVisible(),true);
   if(mode==='review') {
    assert.equal(await page.getByText('Possible cleanup',{exact:true}).count()>0,true);
    assert.equal(await first.locator('.row-details').first().evaluate(e=>e.open),false);
    await first.locator('.row-details > summary').first().focus();await page.keyboard.press('Enter');
    assert.equal(await first.locator('.row-details').first().evaluate(e=>e.open),true);
   }
   const snapshot=()=>page.evaluate(()=>({rows:Array.from(document.querySelectorAll('[data-worktree]'),r=>({...r.dataset})).sort((a,b)=>a.pathHex.localeCompare(b.pathHex)),evidence:Array.from(document.querySelectorAll('pre'),e=>e.textContent)}));
   const before=await snapshot();
   if(js) {
    if(mode==='review') {
     const family=page.locator('.family').first();
     const repository=(await family.locator('.family-details p.mono').textContent()).trim();
     await page.locator('#search').fill(repository);
     assert.equal(await page.locator('[data-worktree]:visible').count(),await family.locator('[data-worktree]').count(),'search must find grouped rows by their repository path');
     await page.keyboard.press('Meta+k');
     await page.locator('#command-query').fill(repository);
     assert.equal(await page.locator('#command-results button').count(),await family.locator('[data-worktree]').count(),'CmdK must use the same repository search');
     await page.keyboard.press('Escape');
     const closeState=await page.evaluate(()=>({open:document.getElementById('command-dialog').open,expanded:document.getElementById('command-query').getAttribute('aria-expanded'),focus:document.activeElement.id}));
     fs.writeFileSync(path.join(output,`${mode}-${theme}-${width}-palette-close.json`),JSON.stringify(closeState,null,2));
     await page.waitForFunction(()=>document.getElementById('command-query').getAttribute('aria-expanded')==='false' && document.activeElement===document.getElementById('search'));
     await page.locator('#search').fill('');
    }
    await page.locator('.list-shortcuts-disclosure > summary').focus(); await page.keyboard.press('Enter');
    const desired=mode==='inventory'?'present':'cleanup_candidate';
    const count=await page.locator(`[data-worktree][data-${mode==='inventory'?'state':'verdict'}="${desired}"]`).count();
    await page.locator('#search').fill('no-such-entry');
    await page.locator(`[data-list-filter="${desired}"]`).first().click();
    assert.equal(await page.locator('[data-worktree]:visible').count(),count);
    assert.equal(await page.locator('#search').inputValue(),'');
    await page.locator('[data-list-sort="allocated"]').first().click();
    assert.equal(await page.locator('[data-worktree]:visible').count(),before.rows.length);
    assert.equal(await page.locator('#sort').inputValue(),'allocated');
    await page.locator('#search').fill('no-such-entry');
    assert.equal(await page.locator('#no-matches').isVisible(),true);
    await page.locator('[data-list-reset]').first().click();
    assert.equal(await page.locator('[data-worktree]:visible').count(),before.rows.length);
    assert.equal(await page.locator('#sort').inputValue(),mode==='inventory'?'state':'path');
    assert.deepEqual(await snapshot(),before);
   }
   await page.locator('a[href="#disk"]').first().click();
   if(js) assert.equal(await page.locator('#disk').evaluate(e=>e.open),true);
   else if(!await page.locator('#disk').evaluate(e=>e.open)) {await page.locator('#disk > summary').focus();await page.keyboard.press('Enter');}
   assert.equal(await page.locator('[data-disk-dashboard]').isVisible(),true);
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
   await page.locator('#disk > summary').focus();await page.keyboard.press('Enter');
   await page.evaluate(()=>scrollTo(0,0));
   await page.screenshot({path:path.join(output,`${mode}-${theme}-${width}-${js?'js':'nojs'}.png`),fullPage:true});
   assert.deepEqual(errors,[]);assert.deepEqual(network,[]);
   checks.push({mode,theme,width,js,firstTop,sourcePreserved:true,noOverflow:true,noNetwork:true}); await page.close();
  }
  fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2)+'\n');
  console.log('PASS: 16 rendered UX states, reachable list/details, shortcut/reset, unchanged evidence and offline rendering');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exit(1);});
