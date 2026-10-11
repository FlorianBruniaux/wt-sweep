// Exercise the report's actual filter/sort code with synthetic DOM nodes, no browser or scan.
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const script = fs.readFileSync(process.argv[2] || path.join(__dirname, '../src/presentation/html/report.js'), 'utf8');
const filtering = script.split('  const sections =')[0] + '\n  update();\n})();';
const control = value => ({value, hidden: false, textContent: '', listeners: {}, addEventListener(name, fn) { this.listeners[name] = fn; }});
const row = (name, state, allocated, verdict = 'not_reviewed') => ({
  dataset: {pathHex: Buffer.from('/fixture/' + name).toString('hex'), branch: name, state, allocated, verdict},
  hidden: false,
  cloneNode() {
    const content = {textContent: name + ' Copy path'};
    content.querySelectorAll = () => [{remove() { content.textContent = name; }}];
    return content;
  },
});
function fixture(flat, rows, splits = [rows]) {
  const groups = splits.map(children => ({children: [...children], querySelectorAll() { return this.children; }, append(child) { this.children = this.children.filter(r => r !== child); this.children.push(child); }}));
  const families = flat ? [] : groups.map(group => ({hidden: false, count: {}, querySelectorAll() { return group.children; }, querySelector() { return this.count; }}));
  const controls = {search: control(''), verdict: control(''), sort: control(flat ? 'state' : 'path'), 'visible-status': control(''), theme: control(''), 'no-matches': control('')};
  const document = {
    documentElement: {},
    getElementById: id => controls[id],
    querySelector: () => flat ? {} : null,
    querySelectorAll(selector) {
      if (selector === '.family') return families;
      if (selector === '[data-worktree]') return rows;
      if (selector.includes('tbody')) return groups;
      throw Error('Unexpected selector ' + selector);
    },
  };
  vm.runInNewContext(filtering, {document});
  const change = (id, value) => { controls[id].value = value; controls[id].listeners[id === 'search' ? 'input' : 'change'](); };
  return {groups, families, controls, change};
}
const rows = [row('z-present', 'present', '9007199254740993'), row('a-bare', 'bare', '9007199254740992'), row('b-absent', 'absent', ''), row('c-unknown', 'unknown', ''), row('y-zero', 'present', '0')];
const before = JSON.stringify(rows.map(r => r.dataset));
const flat = fixture(true, rows);
assert.deepEqual(flat.groups[0].children.map(r => r.dataset.state), ['present', 'present', 'bare', 'absent', 'unknown']);
flat.change('verdict', 'present');
assert.equal(rows.filter(r => !r.hidden).length, 2);
assert.match(flat.controls['visible-status'].textContent, /Showing 2 of 5 recorded entries/);
flat.change('search', 'z-present');
assert.deepEqual(rows.filter(r => !r.hidden), [rows[0]]);
flat.change('search', 'Copy path');
assert.equal(rows.filter(r => !r.hidden).length, 0);
assert.equal(flat.controls['no-matches'].hidden, false);
flat.change('search', '');
flat.change('verdict', '');
flat.change('sort', 'allocated');
assert.deepEqual(flat.groups[0].children, [rows[0], rows[1], rows[4], rows[2], rows[3]], 'sort exact integers globally, unknown sizes after measured zero');
flat.change('sort', 'branch');
assert.deepEqual(flat.groups[0].children.map(r => r.dataset.branch), ['a-bare', 'b-absent', 'c-unknown', 'y-zero', 'z-present']);
assert.equal(JSON.stringify(rows.map(r => r.dataset)), before, 'filtering and sorting preserve captured identities and values');
const reviewRows = [row('z-keep', 'present', '10', 'keep'), row('a-candidate', 'present', '20', 'cleanup_candidate')];
const review = fixture(false, reviewRows, [[reviewRows[0]], [reviewRows[1]]]);
review.change('verdict', 'keep');
assert.equal(review.families[0].hidden, false);
assert.equal(review.families[0].count.textContent, '1');
assert.equal(review.families[1].hidden, true);
review.change('verdict', '');
assert.equal(review.families[1].hidden, false);
const empty = fixture(true, []);
assert.equal(empty.controls['no-matches'].hidden, true);
assert.match(empty.controls['visible-status'].textContent, /Showing 0 of 0/);
console.log('PASS: flat list search/state filters/global exact-byte sorting, unchanged evidence, grouped review and empty inventory');
