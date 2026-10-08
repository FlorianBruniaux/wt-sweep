(() => {
  'use strict';
  const root = document.documentElement;
  const families = Array.from(document.querySelectorAll('.family'));
  const rows = Array.from(document.querySelectorAll('[data-worktree]'));
  const search = document.getElementById('search');
  const verdict = document.getElementById('verdict');
  const sort = document.getElementById('sort');
  const status = document.getElementById('visible-status');
  const theme = document.getElementById('theme');
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  const update = () => {
    const query = search.value.trim().toLowerCase();
    let visible = 0;
    const counts = new Map();
    for (const row of rows) {
      const matches = (!query || row.textContent.toLowerCase().includes(query)) &&
        (!verdict.value || row.dataset.verdict === verdict.value);
      row.hidden = !matches;
      if (matches) {
        visible += 1;
        counts.set(row.dataset.verdict, (counts.get(row.dataset.verdict) || 0) + 1);
      }
    }
    for (const family of families) {
      const tbody = family.querySelector('tbody');
      const children = Array.from(tbody.querySelectorAll('[data-worktree]'));
      children.sort((a, b) => {
        let result = 0;
        if (sort.value === 'allocated') {
          const av = a.dataset.allocated, bv = b.dataset.allocated;
          result = !av && !bv ? 0 : !av ? 1 : !bv ? -1 : compare(BigInt(bv), BigInt(av));
        } else if (sort.value === 'verdict') {
          result = compare(a.dataset.verdict, b.dataset.verdict);
        } else if (sort.value === 'branch') {
          result = compare(a.dataset.branch, b.dataset.branch);
        }
        return result || compare(a.dataset.pathHex, b.dataset.pathHex);
      });
      for (const row of children) tbody.append(row);
      const count = children.filter(row => !row.hidden).length;
      family.querySelector('.visible-count').textContent = String(count);
      family.hidden = count === 0 && children.length > 0;
    }
    const totals = Array.from(counts, ([name, count]) => `${count} ${name}`).join(', ');
    status.textContent = `Visible ${visible} of ${rows.length} selected worktrees${totals ? ': ' + totals : ''}. Source totals and disk measurements are unchanged.`;
    document.getElementById('no-matches').hidden = visible !== 0 || rows.length === 0;
  };
  search.addEventListener('input', update);
  verdict.addEventListener('change', update);
  sort.addEventListener('change', update);
  theme.addEventListener('click', () => {
    const dark = root.dataset.theme ? root.dataset.theme === 'dark' : matchMedia('(prefers-color-scheme: dark)').matches;
    root.dataset.theme = dark ? 'light' : 'dark';
    theme.textContent = dark ? 'Switch to dark' : 'Switch to light';
  });
  theme.textContent = matchMedia('(prefers-color-scheme: dark)').matches ? 'Switch to light' : 'Switch to dark';
  root.classList.add('js');
  update();
})();
