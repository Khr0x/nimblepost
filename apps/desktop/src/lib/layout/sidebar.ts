export const SIDEBAR_MIN_WIDTH = 245;

export function sidebarMaxWidth(viewportWidth: number): number {
  return Math.max(SIDEBAR_MIN_WIDTH, Math.min(480, viewportWidth - 480));
}

export function clampSidebarWidth(width: number, viewportWidth = Infinity): number {
  return Math.round(Math.min(sidebarMaxWidth(viewportWidth), Math.max(SIDEBAR_MIN_WIDTH, Number.isFinite(width) ? width : SIDEBAR_MIN_WIDTH)));
}

export function readSidebarWidth(): number {
  try { return clampSidebarWidth(Number(localStorage.getItem('nimblepost.sidebarWidth'))); }
  catch { return SIDEBAR_MIN_WIDTH; }
}

export function saveSidebarWidth(width: number) {
  try { localStorage.setItem('nimblepost.sidebarWidth', String(clampSidebarWidth(width))); }
  catch (error) { console.warn('Sidebar width could not be saved.', error); }
}
