import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { parseAllocator, smapsCounters, diagnosticProtocol } from './response-diagnostics.mjs';
import { summarizeResponses } from './response-report.mjs';

const xml = `<np013 pid="42" monotonicNs="123456789000"/>
<malloc version="1">
<heap nr="0"><system type="current" size="100"/></heap>
<heap nr="1"><system type="current" size="200"/></heap>
<total type="fast" count="1" size="10"/>
<total type="rest" count="2" size="90"/>
<total type="mmap" count="1" size="400"/>
<system type="current" size="300"/>
</malloc>
`;

test('Allocator footer counts all arenas once and rejects incomplete or inconsistent samples', () => {
  assert.deepEqual(parseAllocator(xml), { pid:42, monotonicMs:123456.789, arenas:2, arenaBytes:300, freeBytes:100, occupiedArenaBytes:200, mmapBytes:400 });
  for (const invalid of [xml.slice(0,-10), xml.replace('size="400"','size="-1"'), xml.replace('size="90"','size="900"'), xml.replace('</malloc>','<total type="mmap" count="1" size="1"/></malloc>'),xml.replace('version="1"','version="2"')]) assert.throws(()=>parseAllocator(invalid));
  assert.deepEqual(smapsCounters('Anonymous: 5 kB\nPrivate_Dirty: 4 kB\nPrivate_Clean: 3 kB\nSwap: 0 kB\n'), { AnonymousBytes:5120, Private_DirtyBytes:4096, Private_CleanBytes:3072, SwapBytes:0 });
  assert.throws(()=>smapsCounters('Anonymous: 5 kB'));
  assert.throws(()=>summarizeResponses({ schemaVersion:1, protocol:diagnosticProtocol, status:'passed' }), /responses-linux-v1/);
});

test('Linux preload exports real glibc XML only when explicitly enabled', { skip:process.platform !== 'linux' }, () => {
  const directory=mkdtempSync(join(tmpdir(),'np013-allocator-test-'));
  try {
    const library=join(directory,'allocator.so'), binary=join(directory,'NimblePost');
    const compile=(args)=> { const r=spawnSync('cc',args,{encoding:'utf8'}); assert.equal(r.status,0,r.stderr); };
    compile(['-shared','-fPIC','-pthread','-Wall','-Wextra','-Werror',new URL('./response-allocator.c',import.meta.url).pathname,'-o',library]);
    writeFileSync(join(directory,'main.c'),'#include <unistd.h>\nint main(void) { sleep(2); return 0; }\n');
    compile([join(directory,'main.c'),'-o',binary]);
    let r=spawnSync(binary,[],{encoding:'utf8',env:{...process.env,LD_PRELOAD:library}});
    assert.equal(r.status,0,r.stderr);
    assert.throws(()=>readFileSync(join(directory,`allocator-${r.pid}.xml`)),{code:'ENOENT'});
    r=spawnSync(binary,[],{encoding:'utf8',env:{...process.env,LD_PRELOAD:library,NP013_ALLOCATOR_DIAGNOSTIC_DIR:directory}});
    assert.equal(r.status,0,r.stderr);
    const counters=parseAllocator(readFileSync(join(directory,`allocator-${r.pid}.xml`),'utf8'));
    assert.equal(counters.pid,r.pid); assert(counters.arenaBytes>0);
  } finally { rmSync(directory,{recursive:true,force:true}); }
});
