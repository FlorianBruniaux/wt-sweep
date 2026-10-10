import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtemp,readFile,rm} from 'node:fs/promises';
import {join} from 'node:path';
import {tmpdir} from 'node:os';
test('an explicit public origin reaches canonical URLs, alternates, robots and XML sitemap',async()=> {
 const out=await mkdtemp(join(tmpdir(),'wt-site-public-build-'));
 try {
  const result=spawnSync(process.execPath,['node_modules/astro/astro.js','build','--outDir',out],{env:{...process.env,SITE_URL:'https://wt-sweep.example.invalid'},encoding:'utf8',timeout:60000});
  assert.equal(result.status,0,result.stderr);
  for(const path of ['index.html','fr/index.html','docs/installation/index.html','fr/docs/installation/index.html']) {
   const html=await readFile(join(out,path),'utf8');
   assert.ok(!html.includes('noindex, nofollow'),path);
   assert.match(html,/rel="canonical" href="https:\/\/wt-sweep.example.invalid\//);
   assert.ok(!html.includes('http://localhost'),path);
   assert.ok(!html.includes('/Users/'),path);
   assert.match(html,/hreflang="x-default"/);
  }
  assert.equal(await readFile(join(out,'robots.txt'),'utf8'),'User-agent: *\nAllow: /\nSitemap: https://wt-sweep.example.invalid/sitemap-index.xml\n');
  const xml=await readFile(join(out,'sitemap-0.xml'),'utf8');
  assert.match(xml,/<loc>https:\/\/wt-sweep.example.invalid\/fr\/docs\/demo\/<\/loc>/);
  assert.ok(!xml.includes('/demo/report.html'));
  assert.ok(!xml.includes('/404'));
 } finally { await rm(out,{recursive:true,force:true}); }
});
