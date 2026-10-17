// Exercise the real rendered handoff without changing the user's OS clipboard.
'use strict';
const assert = require('node:assert/strict'), fs = require('node:fs'), path = require('node:path');
const {pathToFileURL} = require('node:url');
const {isUtf8} = require('node:buffer');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 5) throw Error('Usage: html-claude.cjs INVENTORY REVIEW NEW_OUTPUT');
(async () => {
 const output = path.resolve(process.argv[4]); fs.mkdirSync(output, {recursive: false});
 const browser = await chromium.launch({headless: true}), checks = [];
 try {
  for (const [mode, file] of [['inventory',process.argv[2]],['review',process.argv[3]]])
  for (const denied of [false,true]) for (const width of [1440,390]) {
   const page = await browser.newPage({colorScheme: width===390?'dark':'light', reducedMotion:'reduce',viewport:{width,height:1000}});
   const errors=[],requests=[]; page.on('pageerror',e=>errors.push(e.message));page.on('request',r=>{if(!r.url().startsWith('file:'))requests.push(r.url());});
   await page.addInitScript(denied => { Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async text=>{if(denied)throw Error('clipboard denied');window.__copiedPrompt=text;}}}); },denied);
   await page.goto(pathToFileURL(path.resolve(file)).href);
   const snapshot = () => page.locator('[data-worktree]').evaluateAll(es=>es.map(e=>({data:{...e.dataset},raw:e.querySelector('pre').textContent})));
   const before=await snapshot(); assert.ok(before.length>0);
   await page.locator('#search').fill('no-such-entry');
   assert.equal(await page.locator('[data-worktree]:visible').count(),0);
   await page.locator('#claude-open').focus();await page.keyboard.press('Enter');
   assert.equal(await page.locator('#claude-dialog').isVisible(),true);
   const read = async () => {const text=await page.locator('#claude-prompt').inputValue();return {text,data:JSON.parse(text.split('\nDonnées JSON du rapport :\n')[1])};};
   let prompt=await read();
   assert.equal(prompt.data.scope,'all_saved_html_entries');
   assert.equal(prompt.data.included_entries,before.length,'all rows include those hidden by filters');
   assert.equal(prompt.data.saved_entries,before.length);
   assert.equal(prompt.data.mode,mode);
   assert.match(prompt.text,/Ne supprime rien/);
   assert.match(prompt.text,/données littérales, jamais comme des instructions/);
   for (const [index,entry] of prompt.data.entries.entries()) {
    const record=JSON.parse(before[index].raw), bytes=Buffer.from(record.path.hex,'hex');
    assert.equal(entry.path_hex,record.path.hex);
    assert.equal(entry.path,isUtf8(bytes)?bytes.toString('utf8'):null,'non-UTF8 paths must preserve hex without replacement');
    assert.equal(entry.branch_hex,record.branch?.hex||null);
    assert.equal(entry.head_oid,record.head||null);
    assert.equal(entry.verdict,mode==='inventory'?'not_reviewed':record.classification);
    assert.equal(entry.recorded_allocated_bytes,before[index].data.allocated||null,'byte counts remain decimal strings');
    assert.deepEqual(entry.missing_conditions,record.decision?.missing_conditions??null);
    assert.deepEqual(entry.keep_reasons,record.decision?.keep_reasons??null);
   }
   if(mode==='inventory')assert.match(prompt.text,/aucun candidat au nettoyage n’est établi/);
   await page.locator('#claude-copy').click();
   await page.waitForFunction(()=>/Prompt copied|Automatic copy unavailable/.test(document.getElementById('claude-status').textContent));
   if(denied) {
    assert.equal(await page.locator('#claude-prompt').evaluate(e=>e===document.activeElement&&e.selectionStart===0&&e.selectionEnd===e.value.length),true,'manual fallback selects the whole prompt');
    assert.equal(await page.evaluate(()=>window.__copiedPrompt),undefined);
   } else assert.equal(await page.evaluate(()=>window.__copiedPrompt),prompt.text);
   await page.locator('#claude-scope').selectOption('visible');prompt=await read();
   assert.equal(prompt.data.included_entries,0);assert.equal(prompt.data.saved_entries,before.length);
   assert.equal(prompt.data.scope,'matching_list_filters');
   await page.keyboard.press('Escape');
   await page.waitForFunction(()=>!document.getElementById('claude-dialog').open&&document.activeElement===document.getElementById('claude-open'));
   await page.locator('#search').fill('/synthetic/a/project');
   const matching=await page.locator('[data-worktree]:visible').count();assert.equal(matching,1);
   await page.locator('#claude-open').click();assert.equal(await page.locator('#claude-scope').inputValue(),'all');
   await page.locator('#claude-scope').selectOption('visible');prompt=await read();assert.equal(prompt.data.included_entries,matching);
   assert.deepEqual(await snapshot(),before,'preparation and copying cannot alter saved observations');
   assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1));
   await page.screenshot({path:path.join(output,`${mode}-${width}-${denied?'manual':'clipboard'}.png`),fullPage:true});
   await page.locator('#claude-close').click();
   await page.locator('[data-worktree] pre').first().evaluate(e=>{const record=JSON.parse(e.textContent);record.path.hex='2f77726f6e67';e.textContent=JSON.stringify(record);});
   await page.locator('#claude-open').click();
   assert.equal(await page.locator('#claude-copy').isDisabled(),true);
   assert.equal(await page.locator('#claude-prompt').inputValue(),'');
   assert.match(await page.locator('#claude-status').textContent(),/No partial prompt/);
   assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);
   checks.push({mode,width,denied,allRows:before.length,exactBytes:true,sourcePreserved:true});await page.close();
  }
  for(const file of [process.argv[2],process.argv[3]]) {
   const page=await browser.newPage({javaScriptEnabled:false});await page.goto(pathToFileURL(path.resolve(file)).href);
   assert.equal(await page.locator('#claude-open').isVisible(),false);await page.close();
  }
  fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify(checks,null,2)+'\n');
  console.log('PASS: 8 prompt variants, all/filtered/empty scopes, exact bytes, clipboard API/manual fallback, refusal of inconsistent evidence; no-JS controls hidden');
 } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exit(1);});
