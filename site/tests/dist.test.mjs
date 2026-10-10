import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { resolve, join } from 'node:path';
const dist=resolve('dist');
async function htmlFiles(dir) { const result=[]; for(const e of await readdir(dir,{withFileTypes:true})) { const path=join(dir,e.name); if(e.isDirectory()) result.push(...await htmlFiles(path)); else if(path.endsWith('.html')) result.push(path); } return result; }
test('all internal routes, assets and anchors resolve in the built bilingual site',async()=> {
 const files=await htmlFiles(dist); assert.ok(files.length>=25);
 for(const file of files) {
  const html=await readFile(file,'utf8');
  assert.ok(!html.includes('search-console.bruniaux.com'),file);
  for(const [,href] of html.matchAll(/(?:href|src)="([^" ]+)"/g)) {
   if(/^(?:[a-z][a-z0-9+.-]*:|\/\/)/i.test(href)) continue;
   const fileRoute=file.slice(dist.length).replace(/index\.html$/,'');
   const url=new URL(href,`http://local.test${fileRoute}`);
   const [path,hash]=[url.pathname,url.hash.slice(1)];
   const target=resolve(dist,'.'+(path||file.slice(dist.length))+(path.endsWith('/')?'index.html':''));
   const content=await readFile(target,'utf8');
   if(hash && target.endsWith('.html')) assert.ok(content.includes(`id="${decodeURIComponent(hash)}"`),`${file}: ${href}`);
  }
 }
});
test('landing publishes matching language alternates, honest structured data and local noindex', async()=> {
 for(const [lang,path,other] of [['en','index.html','/fr/'],['fr','fr/index.html','/']]) {
  const html=await readFile(join(dist,path),'utf8');
  assert.match(html,new RegExp(`<html lang="${lang}"`));
  assert.match(html,/noindex, nofollow/);
  assert.match(html,/hreflang="x-default"/);
  assert.ok(html.includes(other));
  const blocks=[...html.matchAll(/<script type="application\/ld\+json">(.*?)<\/script>/gs)].map(x=>JSON.parse(x[1]));
  assert.equal(blocks.find(x=>x['@type']==='FAQPage').mainEntity.length,5);
  const software=blocks.find(x=>x['@type']==='SoftwareApplication');
  assert.equal(software.operatingSystem,'macOS');
  assert.equal(software.license,'https://github.com/FlorianBruniaux/wt-sweep/blob/main/LICENSE.md');
  assert.ok(!software.offers,'no invented release price');
 }
});
test('local preview cannot advertise an invented public sitemap',async()=> {
 const {existsSync}=await import('node:fs');
 assert.equal(existsSync(join(dist,'sitemap-index.xml')),false);
 assert.equal(await readFile(join(dist,'robots.txt'),'utf8'),'User-agent: *\nDisallow: /\n');
});
