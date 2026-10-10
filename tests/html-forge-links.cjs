// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-forge-links.cjs report.html NEW_OUTPUT_DIR
'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
if(process.argv.length!==4)throw Error('Usage: node tests/html-forge-links.cjs report.html NEW_OUTPUT_DIR');
const output=path.resolve(process.argv[3]);fs.mkdirSync(output,{recursive:false});
(async()=>{const browser=await chromium.launch({headless:true});const checks=[];try{
 for(const js of [true,false])for(const theme of ['light','dark'])for(const width of [1440,390]){
  const context=await browser.newContext({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:1000}});
  const attempted=[];
  // Observe the click target without sending a request to any real forge.
  await context.route(/^https?:/,route=>{attempted.push(route.request().url());return route.abort();});
  const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(pathToFileURL(path.resolve(process.argv[2])).href);
  const pr=page.getByRole('link',{name:'PR #17',exact:true}).first();
  await pr.scrollIntoViewIfNeeded();
  assert.equal(await pr.getAttribute('href'),'https://github.com/example-team/worktree-demo/pull/17');
  assert.equal(await pr.getAttribute('target'),'_blank');
  assert.equal(await pr.getAttribute('rel'),'noopener noreferrer');
  const block=pr.locator('xpath=ancestor::*[contains(@class,"forge-navigation")]');
  assert.ok((await block.textContent()).includes('HEAD matches the local tracking ref'));
  assert.ok((await block.textContent()).includes('captured state: open'));
  const remotes=block.locator('.forge-remotes');
  assert.equal(await remotes.evaluate(e=>e.open),false,'remote detail must start collapsed');
  await remotes.locator(':scope > summary').click();
  const branch=block.getByRole('link',{name:'Branch',exact:true}).first();
  assert.equal(await branch.isVisible(),true,'native disclosure must expose clickable branch links');
  assert.equal(await branch.getAttribute('href'),'https://github.com/example-contributor/worktree-demo/tree/topic');
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth)<=width+1,'horizontal overflow');
  assert.deepEqual(attempted,[],'report load must stay offline');
  await remotes.locator(':scope > summary').click();
  await pr.scrollIntoViewIfNeeded();
  await page.screenshot({path:path.join(output,`${theme}-${width}-${js?'js':'nojs'}.png`)});
  const popupPromise=context.waitForEvent('page');await pr.click();const popup=await popupPromise;
  await popup.waitForLoadState('domcontentloaded').catch(()=>{});
  assert.deepEqual(attempted,['https://github.com/example-team/worktree-demo/pull/17']);
  assert.equal(await popup.evaluate(()=>window.opener),null);
  assert.deepEqual(errors,[]);checks.push({js,theme,width,offlineLoad:true,clickTarget:'PR #17',opener:null});
  await context.close();
 }
 fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2));console.log(`${checks.length} browser cases passed`);
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exitCode=1;});
