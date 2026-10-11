'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const source = fs.readFileSync('src/presentation/html/report.js', 'utf8');
const script = source.split('  // A local index')[0] + '\n  update();\n})();';
const control = value => ({value, hidden: false, textContent: '', listeners: {}, attrs: {}, addEventListener(type, fn) { (this.listeners[type] ??= []).push(fn); }, setAttribute(name, value) { this.attrs[name] = value; }});
const row = (name, classification, allocation) => ({
  dataset: {pathHex: Buffer.from('/fixture/' + name).toString('hex'), branch: name, state: 'present', allocated: allocation, verdict: classification}, hidden: false,
  cloneNode() { return {textContent: name, querySelectorAll() { return []; }}; },
});
function fixture(flat) {
  const rows = [row('small', 'keep', '10'), row('large', 'cleanup_candidate', '90')];
  const before = JSON.stringify(rows.map(row => row.dataset));
  const tbody = {children: [...rows], querySelectorAll() { return this.children; }, append(child) { this.children = this.children.filter(row => row !== child); this.children.push(child); }};
  const controls = Object.fromEntries(['search', 'verdict', 'sort', 'visible-status', 'theme', 'no-matches', 'collapse-sections', 'expand-sections'].map(name => [name, control(name === 'sort' ? (flat ? 'state' : 'path') : '')]));
  const filter = control(''); filter.dataset = {listFilter: flat ? 'present' : 'cleanup_candidate'};
  const sort = control(''); sort.dataset = {listSort: 'allocated'};
  const reset = control(''); reset.dataset = {listReset: ''};
  const list = {matches() { return false; }, parentElement: null, scrollIntoView() { this.scrolled = true; }, focus() { this.focused = true; }};
  const document = {
    documentElement: {},
    getElementById(id) { return id === 'worktree-families' ? list : controls[id]; },
    addEventListener() {},
    querySelector() { return flat ? {} : null; },
    querySelectorAll(selector) {
      if (selector === '.family' || selector === 'details.report-section') return [];
      if (selector === '[data-worktree]') return rows;
      if (selector.includes('tbody')) return [tbody];
      if (selector.includes('data-list-filter')) return [filter];
      if (selector.includes('data-list-sort')) return [sort];
      if (selector.includes('data-list-reset')) return [reset];
      throw Error('Unknown selector: ' + selector);
    },
  };
  vm.runInNewContext(script, {document, window: {addEventListener() {}}, location: {hash: ''}});
  const click = button => {
    assert.ok(button.listeners.click?.length, 'shortcut click behavior must be registered');
    button.listeners.click.forEach(handler => handler());
  };
  controls.search.value = 'unmatched';
  click(filter);
  assert.equal(controls.search.value, '', 'filter shortcut clears incompatible query');
  assert.equal(controls.verdict.value, filter.dataset.listFilter);
  assert.equal(rows.filter(row => !row.hidden).length, flat ? 2 : 1);
  assert.equal(filter.attrs['aria-pressed'], 'true');
  assert.equal(list.scrolled, true, 'shortcut reveals the list');
  assert.equal(list.focused, true, 'keyboard reading continues in the revealed list');
  click(sort);
  assert.equal(controls.verdict.value, '', 'size shortcut searches all saved rows');
  assert.equal(controls.sort.value, 'allocated');
  assert.equal(tbody.children[0].dataset.branch, 'large');
  assert.equal(filter.attrs['aria-pressed'], 'false');
  controls.search.value = 'unmatched'; click(reset);
  assert.equal(controls.search.value, '');
  assert.equal(controls.verdict.value, '');
  assert.equal(controls.sort.value, flat ? 'state' : 'path');
  assert.equal(rows.filter(row => !row.hidden).length, 2);
  assert.equal(JSON.stringify(rows.map(row => row.dataset)), before, 'source data stays unchanged');
}
fixture(true); fixture(false);
console.log('PASS: inventory and review shortcut filters, size sort, reset and unchanged source data');
