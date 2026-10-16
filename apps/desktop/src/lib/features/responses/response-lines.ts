export const RESPONSE_LINE_HEIGHT = 21.6;
export const RESPONSE_LINE_LIMIT = 500;
export const RESPONSE_OVERSCAN = 6;
export const RESPONSE_SEGMENT_LENGTH = 2048;

export function hasManyLines(text: string): boolean {
  let count = 1, start = 0;
  for (const match of text.matchAll(/\r\n|\r|\n/g)) {
    if (match.index - start > RESPONSE_SEGMENT_LENGTH || ++count > RESPONSE_LINE_LIMIT) return true;
    start = match.index + match[0].length;
  }
  return text.length - start > RESPONSE_SEGMENT_LENGTH;
}

// Keep offsets into the original body, rather than a second array of strings.
export function responseLineStarts(text: string): Uint32Array {
  return responseLineIndex(text).starts;
}

export function responseLineIndex(text: string): { starts: Uint32Array; segmented: boolean } {
  const starts = [0];
  let segmented = false;
  const segments = (end: number) => {
    while (end - starts[starts.length - 1] > RESPONSE_SEGMENT_LENGTH) {
      let next = starts[starts.length - 1] + RESPONSE_SEGMENT_LENGTH;
      // Keep UTF-16 surrogate pairs together at visual segment boundaries.
      if (text.charCodeAt(next - 1) >= 0xd800 && text.charCodeAt(next - 1) <= 0xdbff && text.charCodeAt(next) >= 0xdc00 && text.charCodeAt(next) <= 0xdfff) next--;
      starts.push(next);
      segmented = true;
    }
  };
  for (const match of text.matchAll(/\r\n|\r|\n/g)) {
    segments(match.index);
    starts.push(match.index + match[0].length);
  }
  segments(text.length);
  return { starts: Uint32Array.from(starts), segmented };
}

export function appendResponseLineIndex(text: string, previous: { text: string; starts: Uint32Array; segmented: boolean }) {
  if (!text.startsWith(previous.text)) return { ...responseLineIndex(text), text };
  // Rescan the tail to join a split CRLF or UTF-16 pair without indexing the full body again.
  const stable = Math.max(0, previous.starts.length - 2), offset = previous.starts[stable] ?? 0;
  const tail = responseLineIndex(text.slice(offset));
  const starts = new Uint32Array(stable + tail.starts.length);
  starts.set(previous.starts.subarray(0, stable));
  for (let i = 0; i < tail.starts.length; i++) starts[stable + i] = offset + tail.starts[i];
  return { text, starts, segmented: previous.segmented || tail.segmented };
}

export function responseLine(text: string, starts: Uint32Array, index: number): string {
  const start = starts[index];
  let end = starts[index + 1] ?? text.length;
  if (end > start && text[end - 1] === '\n') end--;
  if (end > start && text[end - 1] === '\r') end--;
  return text.slice(start, end);
}

export function responseLineTops(count: number, heights: Map<number, number>): Float64Array {
  const tops = new Float64Array(count + 1);
  for (let index = 0; index < count; index++) tops[index + 1] = tops[index] + (heights.get(index) ?? RESPONSE_LINE_HEIGHT);
  return tops;
}

export function responseLineAt(tops: Float64Array, position: number): number {
  let low = 0, high = tops.length - 2;
  while (low < high) {
    const middle = Math.ceil((low + high) / 2);
    if (tops[middle] <= position) low = middle; else high = middle - 1;
  }
  return low;
}

export function responseLineWindow(tops: Float64Array, scrollTop: number, height: number) {
  const start = Math.max(0, responseLineAt(tops, scrollTop) - RESPONSE_OVERSCAN);
  const end = Math.min(tops.length - 1, responseLineAt(tops, scrollTop + height) + RESPONSE_OVERSCAN + 1);
  return { start, end };
}
