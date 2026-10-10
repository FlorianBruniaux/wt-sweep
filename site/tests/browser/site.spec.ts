import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import {readFileSync} from 'node:fs';
const packageVersion = readFileSync(new URL('../../../Cargo.toml', import.meta.url), 'utf8').match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const sourceUpdateDate = readFileSync(new URL('../../../CHANGELOG.md', import.meta.url), 'utf8').match(/<!-- unreleased-updated: (\d{4}-\d{2}-\d{2}) -->/)?.[1];
for(const lang of ['en','fr']) for(const theme of ['light','dark']) for(const width of [390,768,1024,1440]) {
 test(`${lang} ${theme} ${width}: accessible report journey and working navigation`,async({page,context})=>{
  const path=lang==='fr'?'/fr/':'/';
  await context.grantPermissions(['clipboard-read','clipboard-write']);
  await page.setViewportSize({width,height:900});
  await page.addInitScript(t=>localStorage.setItem('theme',t),theme);
  const errors:string[]=[];page.on('pageerror',e=>errors.push(e.message));
  await page.goto(path);
  await expect(page.locator('h1')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme',theme);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  if(width<1024) {
   await page.locator('#mobile-menu-toggle').click();
   await expect(page.locator('#primary-navigation')).toHaveAttribute('role','dialog');
   const openAxe=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21aa']).analyze();expect(openAxe.violations).toEqual([]);
   await page.keyboard.press('Shift+Tab');await expect(page.locator('.header-install')).toBeFocused();
   await page.keyboard.press('Tab');await expect(page.locator('[data-mobile-menu-close]')).toBeFocused();
   await page.keyboard.press('Escape');
   await expect(page.locator('#mobile-menu-toggle')).toBeFocused();
   await page.locator('#mobile-menu-toggle').click();
   await page.locator('[data-nav-trigger="start"]').click();
   await page.locator('[data-nav-panel="start"] a').first().click();
   await expect(page.locator('#mobile-menu-toggle')).toHaveAttribute('aria-expanded','false');
  } else {
   await page.locator('[data-nav-trigger="start"]').focus();await page.keyboard.press('ArrowDown');
   await expect(page.locator('[data-nav-panel="start"] a').first()).toBeFocused();
   await page.keyboard.press('Escape');await expect(page.locator('[data-nav-trigger="start"]')).toBeFocused();
  }
  await page.locator('[data-copy-command]').click();
  expect(await page.evaluate(()=>navigator.clipboard.readText())).toContain('cargo build --release --locked --offline');
  await expect(page.locator('#command-status')).toContainText(lang==='fr'?'copiée':'copied');
  await page.locator('[data-copy-prompt]').click();
  expect(await page.evaluate(()=>navigator.clipboard.readText())).toContain('$wt-sweep');
  const axe=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21aa']).analyze();expect(axe.violations).toEqual([]);
  await page.locator('a[data-doc-start]').click();
  await expect(page.locator('h1')).toBeVisible();
  expect(page.url()).toContain('/docs/installation/');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  const docsAxe=await new AxeBuilder({page}).withTags(['wcag2a','wcag2aa','wcag21aa']).analyze();expect(docsAxe.violations).toEqual([]);
  expect(errors).toEqual([]);
 });
}
test('theme persists and locale switch keeps its page',async({page})=>{
 await page.goto('/');await page.locator('[data-theme-toggle]').click();const theme=await page.locator('html').getAttribute('data-theme');await page.reload();await expect(page.locator('html')).toHaveAttribute('data-theme',theme!);
 await page.goto('/updates/');await page.locator('.header-actions a[hreflang]').click();expect(new URL(page.url()).pathname).toBe('/fr/updates/');
});
test('essential content and report links are usable with JavaScript disabled',async({browser})=>{
 const context=await browser.newContext({javaScriptEnabled:false,viewport:{width:390,height:900}});const page=await context.newPage();await page.goto('http://localhost:49331/fr/');await expect(page.locator('h1')).toBeVisible();await page.locator('main a[data-doc-start]').click();await expect(page.locator('h1')).toBeVisible();await context.close();
});

for(const width of [390,1440]) test(`native synthetic report ${width}: filters and copied paths retain source rows`,async({page,context})=>{
 await context.grantPermissions(['clipboard-read','clipboard-write']);await page.setViewportSize({width,height:900});
 const external:string[]=[];page.on('request',r=>{if(new URL(r.url()).origin!=='http://localhost:49331') external.push(r.url());});
 await page.goto('/demo/report.html');
 await expect(page.locator('[data-worktree]')).toHaveCount(4);
 await page.locator('#verdict').selectOption('cleanup_candidate');await expect(page.locator('[data-worktree]:visible')).toHaveCount(1);
 await page.locator('[data-worktree]:visible [data-copy-path]').click();expect(await page.evaluate(()=>navigator.clipboard.readText())).toMatch(/\/source branch$/);
 await page.locator('#search').fill('nothing-matches');await expect(page.locator('[data-worktree]:visible')).toHaveCount(0);await expect(page.locator('[data-worktree]')).toHaveCount(4);
 await page.locator('#search').fill('');await page.locator('#verdict').selectOption('');await expect(page.locator('[data-worktree]:visible')).toHaveCount(4);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);expect(external).toEqual([]);
});

test('mobile navigation without JavaScript reaches documentation and switches language',async({browser})=>{
 const context=await browser.newContext({javaScriptEnabled:false,viewport:{width:390,height:900}});const page=await context.newPage();await page.goto('http://localhost:49331/fr/');
 const navigation=page.locator('[data-fallback-navigation]');await expect(navigation).toBeVisible();await navigation.locator('a[hreflang="en"]').click();expect(new URL(page.url()).pathname).toBe('/');
 await page.locator('[data-fallback-navigation] a[data-doc-link]').click();expect(new URL(page.url()).pathname).toBe('/docs/');await context.close();
});
test('mobile action labels remain on one line while actions wrap into rows',async({page})=>{
 await page.setViewportSize({width:390,height:900});await page.goto('/fr/');await page.locator('#mobile-menu-toggle').click();
 for(const link of await page.locator('.header-actions a').all()) expect(await link.evaluate(el=>{ const text=Array.from(el.childNodes).find(node=>node.nodeType===Node.TEXT_NODE && node.textContent?.trim());if(!text)return 1;const range=document.createRange();range.selectNodeContents(text);return range.getClientRects().length; })).toBe(1);
});

for (const lang of ['en','fr']) test(`${lang}: documentation search reaches the disk guide`, async ({page}) => {
 const prefix = lang === 'fr' ? '/fr' : '';
 await page.goto(`${prefix}/docs/`);
 await page.getByRole('button', {name: lang === 'fr' ? /Rechercher/ : /Search/}).click();
 const dialog = page.getByRole('dialog');
 await dialog.getByRole('textbox').fill(lang === 'fr' ? 'disque' : 'disk');
 const result = dialog.locator(`a[href*="${prefix}/docs/disk/"]`).first();
 await expect(result).toBeVisible();
 await result.click();
 expect(new URL(page.url()).pathname).toBe(`${prefix}/docs/disk/`);
 await expect(page.locator('h1')).toBeVisible();
});

// Catches lost icon accessibility, a clipped Install action, misalignment or broken drawer focus.
for (const lang of ['en', 'fr']) for (const theme of ['light', 'dark']) {
 test(`Compact ${lang} ${theme}: aligned actions and mobile keyboard access`, async ({page}) => {
  const path = lang === 'fr' ? '/fr/' : '/';
  await page.setViewportSize({width:1440,height:1000});
  await page.addInitScript(value => localStorage.setItem('theme', value), theme);
  await page.goto(path);
  const header = page.locator('.site-header');
  const install = header.getByRole('link', {name: lang === 'fr' ? 'Installer' : 'Install', exact:true});
  const github = header.getByRole('link', {name:/GitHub/});
  await expect(github.locator('svg')).toBeVisible();
  await expect(header.locator('a[hreflang] svg')).toBeVisible();
  await expect(install).toBeVisible();
  const mark = await header.locator('.wordmark').boundingBox();
  const hero = await page.locator('.hero-grid').boundingBox();
  const action = await install.boundingBox();
  expect(Math.abs(mark!.x - hero!.x)).toBeLessThan(1);
  expect(Math.abs(action!.x + action!.width - hero!.x - hero!.width)).toBeLessThan(1);
  const themeControl = header.locator('[data-theme-toggle]');
  expect(await themeControl.evaluate(el => getComputedStyle(el).borderTopWidth)).toBe('0px');
  expect(await install.evaluate(el => getComputedStyle(el).backgroundColor)).not.toBe('rgba(0, 0, 0, 0)');
  await expect(themeControl.locator('svg:visible')).toHaveCount(1);
  await themeControl.focus(); await page.keyboard.press('Tab'); await expect(install).toBeFocused();
  await themeControl.click(); await expect(page.locator('html')).toHaveAttribute('data-theme', theme === 'light' ? 'dark' : 'light');
  await expect(themeControl.locator('svg:visible')).toHaveCount(1);
  await page.setViewportSize({width:390,height:900});
  await header.locator('#mobile-menu-toggle').click();
  await page.keyboard.press('Shift+Tab'); await expect(install).toBeFocused();
  await page.keyboard.press('Tab'); await expect(header.locator('[data-mobile-menu-close]')).toBeFocused();
  const mobileInstall = await install.boundingBox();
  const docs = await header.getByRole('link',{name:'Docs',exact:true}).boundingBox();
  expect(Math.abs(mobileInstall!.width - docs!.width)).toBeLessThan(1);
  expect(mobileInstall!.height).toBeGreaterThanOrEqual(44);
  await install.click(); await expect(header.locator('#mobile-menu-toggle')).toHaveAttribute('aria-expanded','false');
  expect(new URL(page.url()).hash).toBe('#install');
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
 });
}

// Catches a narrowed landing envelope or a missing / misleading updates entry.
for (const lang of ['en', 'fr']) for (const width of [390, 1440, 2048]) {
 test(`Reference layout ${lang} ${width}: shared edges and source updates`, async ({page}) => {
  const prefix = lang === 'fr' ? '/fr' : '';
  await page.setViewportSize({width, height:1000});
  await page.goto(`${prefix}/`);
  const hero = await page.locator('.hero-grid').boundingBox();
  const mark = await page.locator('.wordmark').boundingBox();
  // Literal content edges independently measured from the rendered Search Console site.
  const expected = width === 390 ? {left:16, right:374} : width === 1440 ? {left:72, right:1368} : {left:352, right:1696};
  expect(Math.abs(hero!.x - expected.left)).toBeLessThan(1);
  expect(Math.abs(hero!.x + hero!.width - expected.right)).toBeLessThan(1);
  expect(Math.abs(mark!.x - expected.left)).toBeLessThan(1);
  const banner = page.getByRole('complementary', {name:lang === 'fr' ? 'Nouveautés' : 'What’s new'});
  await expect(banner).toBeVisible();
  const update = banner.getByRole('link');
  await expect(update).toHaveAttribute('href', `${prefix}/updates/`);
  await expect(banner.locator('time')).toHaveAttribute('datetime', sourceUpdateDate!);
  expect(packageVersion).toBeTruthy();
  await expect(banner).toContainText(`v${packageVersion}`);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await update.focus(); await page.keyboard.press('Enter');
  await expect(page).toHaveURL(new RegExp(`${prefix}/updates/$`));
 });
}
