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
  // Control labels and transient copy feedback must not become search terms.
  const searchable = new Map(rows.map(row => {
    const content = row.cloneNode(true);
    content.querySelectorAll('[data-copy-path]').forEach(button => button.remove());
    return [row, content.textContent.toLowerCase()];
  }));
  const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
  const update = () => {
    const query = search.value.trim().toLowerCase();
    let visible = 0;
    const counts = new Map();
    for (const row of rows) {
      const matches = (!query || searchable.get(row).includes(query)) &&
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
  const sections = Array.from(document.querySelectorAll('details.report-section'));
  document.getElementById('collapse-sections').addEventListener('click', () => {
    for (const section of sections) section.open = false;
  });
  document.getElementById('expand-sections').addEventListener('click', () => {
    for (const section of sections) section.open = true;
  });
  const revealFragment = hash => {
    let id;
    try { id = decodeURIComponent(hash.slice(1)); } catch (_) { return; }
    const target = document.getElementById(id);
    if (!target) return;
    for (let ancestor = target; ancestor; ancestor = ancestor.parentElement) {
      if (ancestor.matches('details')) ancestor.open = true;
    }
  };
  // Also handle repeated links to the current fragment, which do not fire hashchange.
  document.addEventListener('click', event => {
    const link = event.target.closest('a[href^="#"]');
    if (link) revealFragment(link.getAttribute('href'));
  });
  window.addEventListener('hashchange', () => revealFragment(location.hash));
  revealFragment(location.hash);
  // Native popovers keep help outside the scrolling sidebar and work on tap without JS.
  for (const item of document.querySelectorAll('.nav-item')) {
    const link = item.querySelector('a'), button = item.querySelector('button');
    const tip = item.querySelector('[popover]');
    if (typeof tip.showPopover !== 'function') continue;
    let pinned = false, timer;
    const isOpen = () => tip.matches(':popover-open');
    const position = () => {
      if (!isOpen()) return;
      const anchor = link.getBoundingClientRect(), box = tip.getBoundingClientRect();
      const left = innerWidth > 800 ? item.getBoundingClientRect().right + 12 : anchor.left;
      const top = innerWidth > 800 ? anchor.top : anchor.bottom + 8;
      tip.style.margin = '0';
      tip.style.left = `${Math.max(16, Math.min(left, innerWidth - box.width - 16))}px`;
      tip.style.top = `${Math.max(16, Math.min(top, innerHeight - box.height - 16))}px`;
    };
    const show = () => { clearTimeout(timer); if (!isOpen()) tip.showPopover(); position(); };
    const hide = () => { clearTimeout(timer); pinned = false; if (isOpen()) tip.hidePopover(); };
    const leave = () => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        if (!pinned && document.activeElement !== link && document.activeElement !== button && !tip.matches(':hover')) hide();
      }, 200);
    };
    link.addEventListener('mouseenter', show);
    link.addEventListener('mouseleave', leave);
    link.addEventListener('focus', show);
    link.addEventListener('blur', leave);
    link.addEventListener('click', hide);
    button.addEventListener('focus', show);
    button.addEventListener('blur', leave);
    button.addEventListener('click', event => {
      event.preventDefault();
      if (pinned && isOpen()) hide(); else { pinned = true; show(); }
    });
    tip.addEventListener('mouseenter', () => clearTimeout(timer));
    tip.addEventListener('mouseleave', leave);
    tip.addEventListener('toggle', () => { if (!isOpen()) pinned = false; });
    window.addEventListener('resize', position);
  }
  const systemTheme = matchMedia('(prefers-color-scheme: dark)');
  const isDark = () => root.dataset.theme ? root.dataset.theme === 'dark' : systemTheme.matches;
  const updateThemeLabel = () => {
    theme.textContent = isDark() ? 'Switch to light' : 'Switch to dark';
  };
  theme.addEventListener('click', () => {
    root.dataset.theme = isDark() ? 'light' : 'dark';
    try {
      localStorage.setItem('wt-sweep.theme', root.dataset.theme);
    } catch (_) {
      // Switching still works when local-file storage is unavailable.
    }
    updateThemeLabel();
  });
  systemTheme.addEventListener('change', updateThemeLabel);
  updateThemeLabel();
  const copyStatus = document.getElementById('copy-status');
  const copyDialog = document.getElementById('copy-dialog');
  const copyValue = document.getElementById('copy-value');
  let copySequence = 0, copyTimer;
  const announceCopy = message => {
    clearTimeout(copyTimer);
    copyStatus.textContent = message;
    copyTimer = setTimeout(() => { copyStatus.textContent = ''; }, 6000);
  };
  document.getElementById('copy-close').addEventListener('click', () => copyDialog.close());
  document.addEventListener('click', async event => {
    const button = event.target.closest('[data-copy-path]');
    if (!button) return;
    const sequence = ++copySequence;
    let path;
    try {
      const hex = button.dataset.copyPath;
      if (!/^(?:[0-9a-f]{2})+$/.test(hex)) throw Error('Invalid path bytes');
      const bytes = Uint8Array.from(hex.match(/../g), pair => parseInt(pair, 16));
      // Fatal decoding refuses substitutions; ignoreBOM preserves the BOM byte sequence.
      path = new TextDecoder('utf-8', {fatal: true, ignoreBOM: true}).decode(bytes);
    } catch (_) {
      announceCopy('Cannot copy this path as UTF-8 text. Its exact hex bytes are in the recorded evidence.');
      return;
    }
    if (/[\u0000-\u001f\u007f-\u009f\u2028-\u202e\u2066-\u2069]/u.test(path)) {
      announceCopy('Cannot copy this path: it contains control characters. Its exact hex bytes are in the recorded evidence.');
      return;
    }
    announceCopy('Copying path…');
    try {
      await navigator.clipboard.writeText(path);
      if (sequence === copySequence) announceCopy('Path copied.');
    } catch (_) {
      if (sequence !== copySequence) return;
      announceCopy('Automatic copy unavailable. Copy the selected path manually.');
      copyValue.value = path;
      copyDialog.showModal();
      copyValue.focus();
      copyValue.select();
    }
  });
  root.classList.add('js');
  update();
})();
