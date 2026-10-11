// Removing a diagram link or pointing it at the wrong panel must break navigation.
'use strict';
const assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
if(process.argv.length!==4&&process.argv.length!==5)throw Error('Usage: node tests/html-diagram.cjs INVENTORY_REPORT NEW_OUTPUT_DIR [REVIEW_REPORT]');
const output=path.resolve(process.argv[3]);fs.mkdirSync(output,{recursive:false});
const cases=[{file:process.argv[2],reviewed:false}];
if(process.argv[4])cases.push({file:process.argv[4],reviewed:true});
(async()=>{const browser=await chromium.launch({headless:true});const checks=[];try{
 for(const fixture of cases)for(const js of [true,false])for(const theme of ['light','dark'])for(const width of [1440,390]){
  console.log('CHECK diagram', {reviewed:fixture.reviewed,js,theme,width});
  const page=await browser.newPage({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:950}});
  const errors=[],requests=[];page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith('file:'))requests.push(r.url());});
  await page.goto(pathToFileURL(path.resolve(fixture.file)).href);
  const before=await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset})));
  const diagram=page.locator('.guide-diagram svg');
  assert.equal(await diagram.locator('a').count(),7,'each teaching block must have a native section link');
  assert.equal(await diagram.getAttribute('role'),'group','links must remain exposed to assistive technology');
  const destinations=[
   ['Explore the chosen folders',fixture.reviewed?'#overview':'#requested-scan-scope'],
   ['Find Git repositories','#worktree-families'],
   ['List registered worktrees','#worktree-families'],
   ['Scan limits','#coverage'],
   ['Inventory: paths and measured sizes','#disk'],
   ['Check current Git and local evidence',fixture.reviewed?'#worktree-families':'#review-guidance'],
   ['Keep / Verify / Cleanup candidate',fixture.reviewed?'#worktree-families':'#review-guidance']
  ];
  const openGuide=async()=>{
   for(const id of ['overview','reading-guide']){
    const details=page.locator(`#${id}`);
    if(!await details.evaluate(e=>e.open))await details.locator(':scope > summary').click();
   }
  };
  for(const [name,href] of destinations){
   await openGuide();const link=diagram.getByRole('link',{name,exact:true});
   assert.equal(await link.getAttribute('href'),href);
   const hitArea=await link.boundingBox();
   assert.ok(hitArea.width>=44&&hitArea.height>=44,`${name}: touch target must be at least 44px`);
   for(const keyboard of [false,true]){
    const target=page.locator(href);assert.equal(await target.count(),1);
    const owner=await target.evaluate(e=>e.closest('details.report-section')?.id);
    if(owner && owner!=='overview'){
     const section=page.locator(`#${owner}`);
     if(await section.evaluate(e=>e.open))await section.locator(':scope > summary').click();
    }
    if(keyboard){await link.focus();assert.equal(await link.evaluate(e=>e===document.activeElement),true);await page.keyboard.press('Enter');}
    else await link.click({position:{x:hitArea.width/2,y:2}});
    assert.equal(new URL(page.url()).hash,href);
    if(js)assert.equal(await target.evaluate(e=>{for(let a=e;a;a=a.parentElement)if(a.matches('details')&&!a.open)return false;return true;}),true,'collapsed destinations must open, including repeated fragments');
   }
  }
  assert.deepEqual(await page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({...e.dataset}))),before);
  assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);
  assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
  await openGuide();
  await diagram.getByRole('link',{name:'Explore the chosen folders',exact:true}).focus();
  await page.locator('.guide-diagram').screenshot({path:path.join(output,`${fixture.reviewed?'review':'inventory'}-${theme}-${width}-js${js}.png`)});
  checks.push({reviewed:fixture.reviewed,js,theme,width,links:7});await page.close();
 }
 fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2));
 console.log(`PASS: ${checks.length} diagram variants, native links, mouse, keyboard, repeated collapsed targets, source conservation, offline`);
}finally{await browser.close();}})().catch(e=>{console.error(e);process.exit(1)});
