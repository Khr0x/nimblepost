import assert from 'node:assert/strict';

export const diagnosticProtocol = 'responses-linux-diagnostic-v2';

// Read only glibc's aggregate footer; summing heap children would double count it.
export function parseAllocator(xml) {
  const header = /^<np013 pid="(\d+)" monotonicNs="(\d+)"\/>\n/.exec(xml);
  assert(header && xml.endsWith('</malloc>\n'), 'Incomplete allocator sample');
  assert(xml.includes('<malloc version="1">'), 'Unknown allocator format');
  const arenas = [...xml.matchAll(/<heap nr="\d+">/g)].length;
  assert(arenas > 0);
  const footer = xml.slice(xml.lastIndexOf('</heap>') + '</heap>'.length);
  const field = pattern => {
    const matches = [...footer.matchAll(pattern)];
    assert.equal(matches.length, 1, 'Missing/duplicate allocator total');
    const value = Number(matches[0][1]); assert(Number.isSafeInteger(value) && value >= 0); return value;
  };
  const arenaBytes = field(/<system type="current" size="(\d+)"\/>/g);
  const freeBytes = field(/<total type="fast" count="\d+" size="(\d+)"\/>/g) + field(/<total type="rest" count="\d+" size="(\d+)"\/>/g);
  const mmapBytes = field(/<total type="mmap" count="\d+" size="(\d+)"\/>/g);
  assert(freeBytes <= arenaBytes, 'Allocator totals inconsistent');
  return { pid: Number(header[1]), monotonicMs: Number(BigInt(header[2]) / 1000n) / 1000,
    arenas, arenaBytes, freeBytes, occupiedArenaBytes: arenaBytes - freeBytes, mmapBytes };
}

export function smapsCounters(text) {
  return Object.fromEntries(['Anonymous', 'Private_Dirty', 'Private_Clean', 'Swap'].map(name => {
    const match = new RegExp(`^${name}:\\s+(\\d+) kB$`, 'm').exec(text);
    assert(match, `Missing smaps counter ${name}`); return [`${name}Bytes`, Number(match[1]) * 1024];
  }));
}
