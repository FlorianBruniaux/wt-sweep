import { readFile, writeFile, mkdir, rm, stat } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve, dirname, relative, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
export async function prepareDocs({root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')} = {}) {
  const site=resolve(root,'site');
  const pages=JSON.parse(await readFile(resolve(site,'published-pages.json'),'utf8'));
  const docs=[];
  for(const page of pages) {
    if(!/^(?:[a-z][a-z0-9-]*|index)$/.test(page.slug) || !/^(?:docs\/[\w./-]+\.md|LICENSE\.md|CHANGELOG\.md)$/.test(page.source) || page.source.includes('..')) throw new Error('Invalid published page');
    const bytes=await readFile(resolve(root,page.source));
    if(createHash('sha256').update(bytes).digest('hex')!==page.sourceHash) throw new Error(`Stale reviewed source: ${page.source}`);
    for(const locale of ['en','fr']) {
      let body=await readFile(resolve(site,`content/${locale}/${page.slug}.md`),'utf8');
      const links = [...body.matchAll(/\[([^\]]+)\]\(([^)]+)\)/g)];
      for (const match of links.reverse()) {
        const [, label, href] = match;
        if (/^(?:[a-z][a-z0-9+.-]*:|\/|#)/i.test(href)) continue;
        const [path, hash = ''] = href.split(/(?=#)/);
        const source = path.startsWith('../../')
          ? relative(root, resolve(root, path.slice(6))).split(sep).join('/')
          : relative(root, resolve(root, dirname(page.source), path)).split(sep).join('/');
        if (source.startsWith('../') || source === '..' || !source) throw new Error(`Unsafe relative link: ${href}`);
        const targetPath = resolve(root, source);
        if (!(await stat(targetPath).catch(() => null))?.isFile()) throw new Error(`Missing relative link: ${page.source}: ${href}`);
        const target = path.startsWith('../../') && pages.find(p => p.source === source);
        const prefix = locale === 'fr' ? '/fr' : '';
        const destination = target
          ? `${prefix}/docs/${target.slug === 'index' ? '' : target.slug + '/'}${hash}`
          : `https://github.com/FlorianBruniaux/wt-sweep/blob/main/${source}${hash}`;
        body = body.slice(0, match.index) + `[${label}](${destination})` + body.slice(match.index + match[0].length);
      }
      const canonical=`https://github.com/FlorianBruniaux/wt-sweep/blob/main/${page.source}`;
      const front=`---\ntitle: ${JSON.stringify(page.title[locale])}\ndescription: ${JSON.stringify(page.description[locale])}\ncanonicalSource: ${JSON.stringify(canonical)}\n---\n`;
      docs.push({path:resolve(site,`src/content/docs/${locale==='fr'?'fr/':''}docs/${page.slug==='index'?'index':page.slug}.md`),body:front+body+`\n\n[${locale==='fr'?'Source canonique':'Canonical source'}](${canonical})\n`});
    }
  }
  // Validate all sources before replacing any generated page.
  await rm(resolve(site,'src/content/docs'),{recursive:true,force:true});
  for(const doc of docs) { await mkdir(dirname(doc.path),{recursive:true}); await writeFile(doc.path,doc.body); }
  const cargo=await readFile(resolve(root,'Cargo.toml'),'utf8');
  const version=cargo.match(/^version = "([^"]+)"/m)?.[1];
  const rust=cargo.match(/^rust-version = "([^"]+)"/m)?.[1];
  if(!version || !rust) throw new Error('Missing Cargo metadata');
  await mkdir(resolve(site,'src/data'),{recursive:true});
  await writeFile(resolve(site,'src/data/product.json'),JSON.stringify({version,rust,license:'FSL-1.1-ALv2',published:false},null,2)+'\n');
  return docs.length;
}
if(process.argv[1] && resolve(process.argv[1])===fileURLToPath(import.meta.url)) console.log(`Prepared ${await prepareDocs()} reviewed pages.`);
