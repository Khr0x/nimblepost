export const RESPONSE_DEFAULT_HEIGHT = 320;
export const RESPONSE_MIN_HEIGHT = 180;

export function responseMinHeight(availableHeight = Infinity): number {
  return Math.min(RESPONSE_MIN_HEIGHT, Math.max(0, Math.floor(availableHeight / 2)));
}

export function responseMaxHeight(availableHeight: number): number {
  return Math.max(responseMinHeight(availableHeight), Math.round(Math.min(availableHeight * 0.7, availableHeight - 200)));
}

export function clampResponseHeight(height: number, availableHeight = Infinity): number {
  return Math.round(Math.min(responseMaxHeight(availableHeight), Math.max(responseMinHeight(availableHeight), Number.isFinite(height) ? height : RESPONSE_DEFAULT_HEIGHT)));
}

export function readResponseHeight(): number {
  try {
    const saved = localStorage.getItem('nimblepost.responseHeight');
    return saved === null ? RESPONSE_DEFAULT_HEIGHT : clampResponseHeight(Number(saved));
  } catch { return RESPONSE_DEFAULT_HEIGHT; }
}

export function saveResponseHeight(height: number) {
  try { localStorage.setItem('nimblepost.responseHeight', String(clampResponseHeight(height))); }
  catch (error) { console.warn('Response height could not be saved.', error); }
}
