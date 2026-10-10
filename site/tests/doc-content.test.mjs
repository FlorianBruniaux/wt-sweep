import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { existsSync } from 'node:fs';
const moduleUrl = new URL('../scripts/prepare-doc-content.mjs', import.meta.url);
test('publishes reviewed bilingual pages and refuses stale sources before writing', async () => {
  assert.ok(existsSync(moduleUrl), 'document publisher is not implemented');
  const { prepareDocs } = await import(moduleUrl);
  const root = await mkdtemp(join(tmpdir(), 'sweep-site-docs-'));
  try {
    for (const path of ['docs','site/content/en','site/content/fr']) await mkdir(join(root,path),{recursive:true});
    await writeFile(join(root,'Cargo.toml'),'[package]\nversion = \"0.1.0-test\"\nrust-version = \"1.95\"\n');
    const source = '# Source\nKnown evidence.\n';
    await writeFile(join(root,'docs/guide.md'),source);
    await writeFile(join(root,'docs/peer.md'),'# Peer\n');
    await writeFile(join(root,'docs/private.md'),'# Internal plan\n');
    const sha256=createHash('sha256').update(source).digest('hex');
    const manifest=[{slug:'guide',source:'docs/guide.md',sourceHash:sha256,title:{en:'Guide',fr:'Guide français'},description:{en:'Reviewed guide',fr:'Guide relu'}}];
    await writeFile(join(root,'site/published-pages.json'),JSON.stringify(manifest));
    await writeFile(join(root,'site/content/en/guide.md'),'Read [guide](../../docs/guide.md) and [internal plan](../../docs/private.md). Also [peer](peer.md) and [self section](guide.md#known).');
    await writeFile(join(root,'site/content/fr/guide.md'),'Lire [guide](../../docs/guide.md).');
    await prepareDocs({root});
    const output=await readFile(join(root,'site/src/content/docs/docs/guide.md'),'utf8');
    assert.match(output,/href|canonicalSource/);
    assert.match(output,/\]\(\/docs\/guide\/\)/);
    assert.match(output,/\]\(https:\/\/github.com\/FlorianBruniaux\/wt-sweep\/blob\/main\/docs\/peer.md\)/);
    assert.match(output,/\]\(https:\/\/github.com\/FlorianBruniaux\/wt-sweep\/blob\/main\/docs\/guide.md#known\)/);
    assert.match(output,/https:\/\/github.com\/FlorianBruniaux\/wt-sweep\/blob\/main\/docs\/private.md/);
    assert.match(await readFile(join(root,'site/src/content/docs/fr/docs/guide.md'),'utf8'),/title: "Guide français"/);
    assert.equal(JSON.parse(await readFile(join(root,'site/src/data/product.json'),'utf8')).published,false);
    await writeFile(join(root,'CHANGELOG.md'),'## [Unreleased]\n\n## [0.1.0-test] - 2026-10-10\n- First package.\n');
    await prepareDocs({root});
    assert.equal(JSON.parse(await readFile(join(root,'site/src/data/product.json'),'utf8')).published,true);
    await writeFile(join(root,'site/content/en/guide.md'),'Read [missing](missing.md).');
    await assert.rejects(prepareDocs({root}),/Missing relative link/);
    assert.equal(await readFile(join(root,'site/src/content/docs/docs/guide.md'),'utf8'),output);
    await writeFile(join(root,'docs/guide.md'),source+'New fact.');
    await assert.rejects(prepareDocs({root}),/Stale reviewed source/);
    assert.equal(await readFile(join(root,'site/src/content/docs/docs/guide.md'),'utf8'),output);
  } finally { await rm(root,{recursive:true,force:true}); }
});

test('native website demo is generated only from a conserved disposable Git family',async()=> {
 const {spawnSync}=await import('node:child_process');
 const out=await mkdtemp(join(tmpdir(),'sweep-site-native-test-'));
 const result=spawnSync(process.env.WT_SITE_PYTHON || 'python3',['-B','scripts/generate-demo.py','--out',out],{cwd:new URL('..',import.meta.url),encoding:'utf8',timeout:60000});
 assert.equal(result.status,0,result.stderr);
 const provenance=JSON.parse(await readFile(join(out,'provenance.json'),'utf8'));
 assert.deepEqual(provenance.verdicts,{keep:3,cleanup_candidate:1});
 assert.equal(provenance.fixture_conserved,true);
 assert.equal(provenance.receipt_verified,true);
 await rm(out,{recursive:true,force:true});
});
