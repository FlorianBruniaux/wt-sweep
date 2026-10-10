// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-nav.cjs report.html NEW_OUTPUT_DIR
'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
if(process.argv.length!==4)throw Error('Usage: node tests/html-nav.cjs report.html NEW_OUTPUT_DIR');
const output=path.resolve(process.argv[3]);fs.mkdirSync(output,{recursive:false});
(async()=>{const browser=await chromium.launch({headless:true});try{
 for(const js of [true,false])for(const theme of ['light','dark'])for(const width of [1440,390]){
  // Exercise link navigation without smooth-scroll animation racing the next click.
  const page=await browser.newPage({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:900}});
  const errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith('file:'))requests.push(r.url());});
  await page.goto(pathToFileURL(path.resolve(process.argv[2])).href);
  const rows=await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset})));
  const entries=page.locator('.nav-item');assert.equal(await entries.count(),4,'four menu explanations required');
  for(let i=0;i<4;i++){
   const entry=entries.nth(i),link=entry.locator('a'),tip=entry.locator('[role="tooltip"]');
   assert.equal(await entry.locator('button').count(),0,'section labels are the only navigation controls');
   assert.equal(await link.getAttribute('aria-describedby'),await tip.getAttribute('id'));
   assert.ok((await tip.textContent()).trim().length>40);
   assert.equal(await link.getAttribute('title'),js?null:await tip.textContent(),'native help is the no-JS fallback only');
   if(js){
    await link.hover();await tip.waitFor({state:'visible'});
    await tip.hover();assert.equal(await tip.isVisible(),true,'tooltip must remain hoverable');
    await page.keyboard.press('Escape');await tip.waitFor({state:'hidden'});
    await page.mouse.move(width-1,899);await link.focus();await tip.waitFor({state:'visible'});
    await page.keyboard.press('Escape');await tip.waitFor({state:'hidden'});
   }
   await link.click();assert.equal(new URL(page.url()).hash,await link.getAttribute('href'),'label must still navigate');
   if(js){
    await tip.waitFor({state:'visible'});
    const box=await tip.boundingBox();assert.ok(box.x>=0&&box.y>=0&&box.x+box.width<=width+1&&box.y+box.height<=901,'tooltip outside viewport');
   }
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth)<=width+1);
   if(i===1)await page.screenshot({path:path.join(output,`${theme}-${width}-${js?'js':'nojs'}.png`)});
   await page.keyboard.press('Escape');await tip.waitFor({state:'hidden'});
  }
  if(js){
   await entries.last().locator('a').click();await entries.last().locator('[role="tooltip"]').waitFor({state:'visible'});
   await page.locator('h1').click();await entries.last().locator('[role="tooltip"]').waitFor({state:'hidden'});
  }
  await entries.nth(2).locator('a').click();assert.equal(new URL(page.url()).hash,'#disk');
  assert.deepEqual(await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset}))),rows);
  assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);await page.close();
 }
 console.log('PASS: 8 navigation renders; hover, keyboard, Escape, click, outside dismissal, native no-JS help attributes, intact anchors/data, no overflow or network');
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exit(1)});
