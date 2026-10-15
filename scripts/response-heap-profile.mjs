import assert from 'node:assert/strict';
import { spawn, execFileSync } from 'node:child_process';
import { readFile, readdir, stat } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { hash } from './performance-report.mjs';
import { parseStat } from './native-performance.mjs';

const delay = ms => new Promise(resolve=>setTimeout(resolve,ms));

export function smapsGroups(text) {
  const groups = new Map(); let current;
  for (const line of text.split('\n')) {
    const header = /^[a-f0-9]+-[a-f0-9]+\s+(\S+)\s+[a-f0-9]+\s+\S+\s+\d+\s*(.*)$/.exec(line);
    if (header) {
      const path = header[2], label = !path ? 'anonymous' : path.startsWith('[') ? path : basename(path);
      const key = `${header[1]} ${label}`;
      current = groups.get(key);
      if (!current) { current = { label, permissions:header[1], mappings:0, sizeBytes:0, rssBytes:0, pssBytes:0, anonymousBytes:0 }; groups.set(key,current); }
      current.mappings++;
    } else if (current) {
      const metric = /^(Size|Rss|Pss|Anonymous):\s+(\d+) kB$/.exec(line);
      if (metric) current[`${metric[1]==='Anonymous'?'anonymous':metric[1].toLowerCase()}Bytes`] += Number(metric[2])*1024;
    }
  }
  assert(groups.size, 'No smaps mappings');
  return [...groups.values()].sort((a,b)=>b.pssBytes-a.pssBytes);
}

export async function attachHeapProfile(proc, directory, pageSize, hz) {
  assert(/^WebKitWebProces/.test(proc.name), 'Only the synthetic WebKit child may be profiled');
  const identity = async () => assert.equal(parseStat(await readFile(`/proc/${proc.pid}/stat`,'utf8'),pageSize,hz).startTicks,proc.startTicks,'Profiler PID reused');
  await identity();
  const version = execFileSync('heaptrack',['--version'],{encoding:'utf8'}).trim();
  const prefix = join(directory,'webkit-heap');
  const child = spawn('heaptrack',['--record-only','--output',prefix,'--pid',String(proc.pid)],{env:{...process.env,DEBUGINFOD_URLS:''},stdio:['ignore','pipe','pipe'],detached:true});
  let log='', error;
  for (const stream of [child.stdout,child.stderr]) stream.on('data',bytes=>{log=(log+bytes).slice(-32000);});
  child.on('error',e=>{error=e;});
  const exited = new Promise(resolve=>child.once('exit',resolve));
  const terminate = async () => {
    // The owned process group includes GDB and the interpreter if attach stalls.
    if (child.pid && child.exitCode===null && child.signalCode===null) {
      process.kill(-child.pid,'SIGTERM'); await Promise.race([exited,delay(3000)]);
      if (child.exitCode===null && child.signalCode===null) { process.kill(-child.pid,'SIGKILL'); await exited; }
    }
  };
  try {
    const deadline=performance.now()+60000;
    while (true) {
      if (error) throw error;
      assert(child.exitCode===null && child.signalCode===null,`Heap profiler exited: ${log}`);
      await identity();
      if ((await readFile(`/proc/${proc.pid}/maps`,'utf8')).includes('libheaptrack_inject.so') && log.includes('injection finished')) break;
      assert(performance.now()<deadline,`Heap profiler attach timeout: ${log}`); await delay(200);
    }
  } catch (error) { await terminate(); throw error; }
  return {
    pid:proc.pid, startTicks:proc.startTicks, version, attachedAtMonotonicMs:Number(process.hrtime.bigint()/1000n)/1000,
    async finish() {
      // Called after the identity-checked app family exits; EOF lets the compressor finish.
      await Promise.race([exited,delay(20000)]);
      if (child.exitCode===null && child.signalCode===null) await terminate();
      assert.equal(child.exitCode,0,`Heap profiler failed: ${log}`);
      const traces=(await readdir(directory)).filter(name=>/^webkit-heap\.(gz|zst)$/.test(name));
      assert.equal(traces.length,1,'Missing heap trace');
      const trace=join(directory,traces[0]), size=(await stat(trace)).size; assert(size>1000);
      const massif=join(directory,'webkit.massif');
      const summary=execFileSync('heaptrack_print',['--file',trace,'--print-leaks','1','--print-peaks','1','--print-allocators','0','--print-temporary','0','--peak-limit','20','--sub-peak-limit','5','--print-massif',massif,'--massif-detailed-freq','0'],{encoding:'utf8',maxBuffer:8*2**20,timeout:60000});
      assert(summary.includes('peak heap memory consumption'), 'Incomplete heap summary');
      assert(!/NP013_(JSON|LINE|TEXT|RUNTIME_SECRET_CANARY)|a{512}|x{512}/.test(summary+log), 'Payload reached heap profile output');
      return { pid:proc.pid,startTicks:proc.startTicks,version,traceBytes:size,traceSha256:hash(await readFile(trace)),log,summary,massif:await readFile(massif,'utf8'),
        limitation:'Allocations after attach only; custom allocators and missing symbols limit coverage. Not freed at process termination is not proof of a leak. Raw trace discarded; no payload/heap-object dump.' };
    },
  };
}
