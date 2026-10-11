// Optional browser check; use an already installed Playwright and Chromium.
// PLAYWRIGHT_MODULE=/path/to/playwright node tests/html-theme.cjs report.html output-dir
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
if (process.argv.length !== 4) throw Error('Usage: node tests/html-theme.cjs report.html NEW_OUTPUT_DIR');
const report = path.resolve(process.argv[2]), output = path.resolve(process.argv[3]);
fs.mkdirSync(output, {recursive:false});
(async()=>{
  const browser=await chromium.launch({headless:true}), checks=[];
  try {
    for(const js of [true,false]) for(const theme of ['light','dark']) for(const width of [2560,1440,390]) {
      const context=await browser.newContext({javaScriptEnabled:js,colorScheme:theme,reducedMotion:'reduce',viewport:{width,height:width===390?844:1000}});
      const page=await context.newPage(), errors=[], requests=[];
      page.on('pageerror',e=>errors.push(e.message));
      page.on('request',r=>{if(!r.url().startsWith('file:'))requests.push(r.url());});
      await page.goto(pathToFileURL(report).href);
      const rows=await page.locator('[data-worktree]').count();
      assert.ok(rows>0); assert.equal(await page.locator('[data-worktree]:visible').count(),rows);
      for (const heading of await page.locator('.report-section > summary h2').all()) {
        const offset = await heading.evaluate(e => e.getBoundingClientRect().top - e.parentElement.getBoundingClientRect().top);
        assert.ok(offset <= 16, 'section heading wraps below its disclosure marker');
      }
      const metrics=await page.evaluate(()=>{
        const color=v=>v.match(/[\d.]+/g).map(Number);
        const bg=el=>{const chain=[];for(let e=el;e;e=e.parentElement)chain.unshift(e);let result=[255,255,255];for(const e of chain){const c=color(getComputedStyle(e).backgroundColor),a=c[3]??1;result=result.map((v,i)=>c[i]*a+v*(1-a));}return result;};
        const lum=c=>c.slice(0,3).map(v=>v/255).map(v=>v<=.04045?v/12.92:((v+.055)/1.055)**2.4).reduce((s,v,i)=>s+v*[.2126,.7152,.0722][i],0);
        const samples=[];
        for(const selector of ['body','.meta','.eyebrow','.sidebar a','.sidebar .boundary','.notice','.badge.keep','.badge.verify','.badge.cleanup_candidate','.view-status','a.primary-action','.row-details > summary','input','button']) {
          const el=document.querySelector(selector);if(!el||!el.getClientRects().length)continue;
          const a=lum(color(getComputedStyle(el).color)),b=lum(bg(el));samples.push({selector,ratio:(Math.max(a,b)+.05)/(Math.min(a,b)+.05)});
        }
        const targets=Array.from(document.querySelectorAll('button,input,select,summary,.sidebar nav a')).filter(e=>e.getClientRects().length).map(e=>({tag:e.tagName,id:e.id,height:e.getBoundingClientRect().height,width:e.getBoundingClientRect().width}));
        const main=document.querySelector('.main').getBoundingClientRect(),sidebar=document.querySelector('.sidebar').getBoundingClientRect();
        return {main:{left:main.left,right:main.right},sidebarRight:sidebar.right,viewportWidth:document.documentElement.clientWidth,background:getComputedStyle(document.body).backgroundColor,scroll:document.documentElement.scrollWidth,scrollBehavior:getComputedStyle(document.documentElement).scrollBehavior,samples,targets};
      });
      assert.equal(metrics.background,theme==='light'?'rgb(245, 240, 235)':'rgb(10, 10, 10)');
      assert.ok(metrics.main.right>=metrics.viewportWidth-1,'report leaves unused space on the right');
      assert.ok(metrics.main.left<=(width>800?metrics.sidebarRight:0)+1,'report is centered inside an unnecessary width cap');
      assert.ok(metrics.scroll<=width+1,'document overflow');assert.equal(metrics.scrollBehavior,'auto');
      for(const c of metrics.samples)assert.ok(c.ratio>=4.5,theme+'/'+width+' contrast '+c.selector+'='+c.ratio);
      for(const t of metrics.targets)assert.ok(t.height>=44&&t.width>=44,'small target '+JSON.stringify(t));
      await page.keyboard.press('Tab');
      assert.equal(await page.locator('.skip').evaluate(e=>e===document.activeElement),true);
      const focus=await page.locator('.skip').evaluate(e=>({width:getComputedStyle(e).outlineWidth,style:getComputedStyle(e).outlineStyle,color:getComputedStyle(e).outlineColor}));
      assert.ok(parseFloat(focus.width)>=2,'keyboard focus outline must be at least 2px');assert.notEqual(focus.style,'none');assert.equal(focus.color,theme==='light'?'rgb(194, 65, 12)':'rgb(249, 115, 22)');
      const summary=page.locator('tbody > tr > td:last-child > details > summary').first();
      const evidenceWidth=await summary.evaluate(e=>e.parentElement.parentElement.getBoundingClientRect().width);
      await summary.focus();await page.keyboard.press('Enter');assert.equal(await summary.evaluate(e=>e.parentElement.open),true);
      if(width>=1440) {
        // Poll from Node: no-JS pages cannot rely on browser animation callbacks.
        const deadline=Date.now()+3000;
        let expanded=await summary.evaluate(e=>e.parentElement.parentElement.getBoundingClientRect().width);
        while(expanded<evidenceWidth*1.8&&Date.now()<deadline) {
          await new Promise(resolve=>setTimeout(resolve,50));
          expanded=await summary.evaluate(e=>e.parentElement.parentElement.getBoundingClientRect().width);
        }
        assert.ok(expanded>=evidenceWidth*1.8,'expanded evidence remains too narrow');
      }
      assert.ok(await page.evaluate(()=>document.documentElement.scrollWidth)<=width+1,'expanded document overflow');
      if(js) {
        await page.locator('#search').fill('no-such-worktree-theme-test');
        assert.equal(await page.locator('[data-worktree]:visible').count(),0);
        await page.locator('#search').fill('');
        const inventory=await page.locator('[data-flat-list]').count()>0;
        const filter=inventory?'present':'keep';
        const selected=await page.locator(inventory?'[data-worktree][data-state="present"]':'[data-worktree][data-verdict="keep"]').count();
        await page.locator('#verdict').selectOption(filter);
        assert.equal(await page.locator('[data-worktree]:visible').count(),selected);
        await page.locator('#verdict').selectOption('');
        await page.locator('#sort').selectOption('allocated');
      }
      await page.evaluate(()=>window.scrollTo(0,0));await page.screenshot({path:path.join(output,theme+'-'+width+'-'+(js?'js':'nojs')+'.png')});
      assert.deepEqual(errors,[]);assert.deepEqual(requests,[]);checks.push({theme,width,js,rows,...metrics,focus});await context.close();
    }
    for(const saved of ['dark','light','invalid',null]) {
      const context=await browser.newContext({colorScheme:'light'});
      await context.addInitScript(value=>{localStorage.removeItem('wt-sweep.theme');if(value!==null)localStorage.setItem('wt-sweep.theme',value);document.addEventListener('DOMContentLoaded',()=>{window.themeAtDomReady=document.documentElement.dataset.theme||null;});},saved);
      const page=await context.newPage();await page.goto(pathToFileURL(report).href);
      assert.equal(await page.evaluate(()=>window.themeAtDomReady),saved==='dark'||saved==='light'?saved:null);
      assert.equal(await page.locator('#theme').textContent(),saved==='dark'?'Switch to light':'Switch to dark');
      await page.locator('#theme').click();assert.equal(await page.evaluate(()=>localStorage.getItem('wt-sweep.theme')),saved==='dark'?'light':'dark');await context.close();
    }
    const reloadContext=await browser.newContext({colorScheme:'light'});
    const reloadPage=await reloadContext.newPage();
    await reloadPage.goto(pathToFileURL(report).href);
    for(const selected of ['dark','light']) {
      await reloadPage.locator('#theme').click();
      assert.equal(await reloadPage.evaluate(()=>localStorage.getItem('wt-sweep.theme')),selected);
      await reloadPage.reload();
      assert.equal(await reloadPage.locator('html').getAttribute('data-theme'),selected);
      assert.equal(await reloadPage.locator('#theme').textContent(),selected==='dark'?'Switch to light':'Switch to dark');
    }
    await reloadContext.close();
    const context=await browser.newContext({colorScheme:'dark'});
    await context.addInitScript(()=>Object.defineProperty(window,'localStorage',{get(){throw Error('storage denied by test');}}));
    const page=await context.newPage(),errors=[];page.on('pageerror',e=>errors.push(e.message));await page.goto(pathToFileURL(report).href);
    assert.equal(await page.locator('#theme').textContent(),'Switch to light');await page.emulateMedia({colorScheme:'light'});await page.waitForFunction(()=>document.getElementById('theme').textContent==='Switch to dark');await page.locator('#theme').click();assert.equal(await page.locator('html').getAttribute('data-theme'),'dark');assert.deepEqual(errors,[]);
    await page.emulateMedia({media:'print'});assert.equal(await page.locator('body').evaluate(e=>getComputedStyle(e).backgroundColor),'rgb(255, 255, 255)');await context.close();
    fs.writeFileSync(path.join(output,'checks.json'),JSON.stringify({renders:checks,savedPreferenceCases:4,preferenceReload:true,deniedStorage:true,systemThemeChange:true,print:true},null,2)+'\n');
    console.log('PASS: '+checks.length+' renders, preferences, keyboard, contrast, targets, reduced motion, print');
  } finally {await browser.close();}
})().catch(e=>{console.error(e);process.exit(1);});
