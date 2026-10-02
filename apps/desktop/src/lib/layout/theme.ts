export type Theme = 'dark' | 'light';

export function readTheme(): Theme {
  try { return localStorage.getItem('nimblepost.theme') === 'light' ? 'light' : 'dark'; }
  catch { return 'dark'; }
}

export function applyTheme(theme: Theme) {
  document.documentElement.dataset.theme = theme;
}

export function saveTheme(theme: Theme) {
  applyTheme(theme);
  try { localStorage.setItem('nimblepost.theme', theme); }
  catch (error) { console.warn('Theme preference could not be saved.', error); }
}
