(() => {
  'use strict';
  try {
    const saved = localStorage.getItem('wt-sweep.theme');
    if (saved === 'light' || saved === 'dark') document.documentElement.dataset.theme = saved;
  } catch (_) {
    // Local-file or privacy policies may deny storage; CSS follows the system.
  }
})();
