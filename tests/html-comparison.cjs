// A real --compare-local report must retain navigation, filters and prompt controls.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs'), os = require('node:os'), path = require('node:path');
const {spawnSync} = require('node:child_process');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
function run(command, args, cwd) {
 const result = spawnSync(command,args,{cwd,encoding:'utf8',timeout:120000,
  env:{...process.env,GIT_CONFIG_NOSYSTEM:'1',GIT_CONFIG_GLOBAL:'/dev/null',GIT_TERMINAL_PROMPT:'0'}});
 assert.equal(result.status,0,result.stderr||String(result.error));
}
(async()=>{
 let owned, report=process.argv[2];
 if(!report) {
  owned=fs.mkdtempSync(path.join(fs.realpathSync(os.tmpdir()),'wt-sweep-browser-comparison-'));
  const repo=path.join(owned,'repo'), wt=path.join(owned,'worktree'), output=path.join(owned,'reports');
  fs.mkdirSync(repo);fs.mkdirSync(output);
  run('git',['init','-b','main'],repo);
  fs.writeFileSync(path.join(repo,'tracked.txt'),'preserved\n');
  run('git',['add','tracked.txt'],repo);
  run('git',['-c','user.name=Fixture','-c','user.email=fixture@example.invalid','commit','-m','base'],repo);
  run('git',['worktree','add','-b','note',wt],repo);
  fs.writeFileSync(path.join(wt,'notes.txt'),'unique local note\n');
  run(path.resolve(process.env.WT_SWEEP_BIN||'target/debug/wt-sweep'),['review','--repo',repo,'--target','refs/heads/main','--compare-local','--html','--summary','--run-dir',output],repo);
  report=path.join(output,fs.readdirSync(output).find(name=>name.startsWith('run-')),'report.html');
 }
 const browser=await chromium.launch({headless:true});
 try {
  const page=await browser.newPage(), errors=[];
  page.on('pageerror',error=>errors.push(error.message));
  await page.goto(pathToFileURL(path.resolve(report)).href);
  assert.deepEqual(errors,[],'comparison navigation must not abort report initialization');
  assert.equal(await page.locator('#claude-open').isVisible(),true);
  await page.locator('nav a[href="#local-comparison"]').click();
  assert.equal(await page.locator('#local-comparison').isVisible(),true);
  await page.locator('#search').fill('no-matching-worktree');
  assert.equal(await page.locator('[data-worktree]:visible').count(),0);
  await page.locator('#claude-open').click();
  assert.equal(await page.locator('#claude-dialog').isVisible(),true);
  const prompt=await page.locator('#claude-prompt').inputValue();
  assert.match(prompt,/Ne supprime rien/);
  assert.match(prompt,/notes\.txt|local_untracked/);
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#claude-dialog').isVisible(),false);
  assert.deepEqual(errors,[]);
  console.log('PASS: real local-comparison HTML navigation, filters and Claude prompt');
 } finally {
  await browser.close();
  if(owned)fs.rmSync(owned,{recursive:true,force:true});
 }
})().catch(error=>{console.error(error);process.exitCode=1;});
