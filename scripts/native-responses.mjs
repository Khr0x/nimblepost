import { readFile, writeFile, mkdir, rename, mkdtemp, rm, readdir, stat, realpath } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir, hostname, cpus, totalmem, release, loadavg } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { family, fixtures, parseStat } from './native-performance.mjs';
import { hash } from './performance-report.mjs';
import { serveResponses } from './response-fixtures.mjs';
import { responseProtocol, responseLaunchCount, summarizeResponses } from './response-report.mjs';
import { diagnosticProtocol, parseAllocator, smapsCounters } from './response-diagnostics.mjs';
import { attachHeapProfile, smapsGroups } from './response-heap-profile.mjs';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const command = (name, args) => execFileSync(name, args, { encoding: 'utf8', maxBuffer: 32 * 2 ** 20 }).trim();
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const save = async (path, value) => { await writeFile(`${path}.tmp`, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 }); await rename(`${path}.tmp`, path); };

export function parsePss(text) {
  const match = /^Pss:\s+(\d+) kB$/m.exec(text);
  assert(match, 'PSS unavailable'); return Number(match[1]) * 1024;
}
export function copyWindow(candidates, viewport) {
  const matches = candidates.filter(c => c.width >= viewport.innerWidth && c.height >= viewport.innerHeight);
  assert(matches.length, `No application-sized X11 window: ${JSON.stringify({ candidates, viewport })}`);
  return matches.sort((a, b) => b.width * b.height - a.width * a.height)[0];
}
async function files(directory) {
  const result = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) result.push(...await files(path)); else if (entry.isFile()) result.push(path);
  }
  return result;
}
async function collectionHash(directory) {
  const paths = (await files(directory)).sort();
  return hash((await Promise.all(paths.map(async p => `${p.slice(directory.length)}:${hash(await readFile(p))}`))).join('\n'));
}
export async function responseWorkspace(directory, url, diagnostics = false, fineProfile = false, heapProfile = false) {
  await fixtures(directory, 100);
  const workspace = await json(join(directory, 'profile/workspaces.json'));
  workspace.workspaces.push({ id: 2, name: 'Empty validation workspace', collections: [], activeCollection: null });
  await save(join(directory, 'profile/workspaces.json'), workspace);
  for (const path of await files(join(directory, 'collection'))) if (path.endsWith('.yml') && !path.endsWith('opencollection.yml')) {
    const source = await json(path);
    source.http.headers = [{ name: 'X-Probe-Secret', value: '{{np013-token}}' }];
    await save(path, source);
  }
  await mkdir(join(directory, 'collection/environments'));
  await save(join(directory, 'collection/environments/validation.yml'), { name: 'validation', variables: [{ name: 'np013-token', secret: true, type: 'string' }] });
  // Recovery refers to the original request; begin with clean tabs for this protocol.
  await save(join(directory, 'profile/recovery.json'), { version: 1, workspaceId: 1, activeTab: null, home: true, tabs: [] });
  await save(join(directory, 'plan.json'), { responses: true, url, ...(diagnostics ? { responseDiagnostics: true } : {}), ...(fineProfile ? { fineProfile:true } : {}), ...(heapProfile ? { heapProfile:true } : {}) });
}
async function runLaunch(binary, sha, server, iteration, checkpoint, allocatorLibrary, fineProfile, heapRequested) {
  const directory = await mkdtemp(join(tmpdir(), 'nimblepost-responses-'));
  const pageSize = Number(command('getconf', ['PAGESIZE'])), hz = Number(command('getconf', ['CLK_TCK']));
  let child, known = [], startTicks, sampling, stopped = false, sampleError, stderr = '', heapSession, outcome, failure;
  const points = [], phases = [], trafficStart = server.traffic.length;
  try {
    await responseWorkspace(directory, server.url, !!allocatorLibrary, fineProfile, heapRequested);
    const before = await collectionHash(join(directory, 'collection'));
    child = spawn(binary, [], { env: { ...process.env, NIMBLEPOST_VALIDATION_DIR: directory,
      ...(allocatorLibrary && !heapRequested ? { LD_PRELOAD: allocatorLibrary, NP013_ALLOCATOR_DIAGNOSTIC_DIR: directory } : {}),
      ...(heapRequested ? { HEAPTRACK_UNWIND_DEPTH:'16' } : {}) }, stdio: ['ignore', 'ignore', 'pipe'] });
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
    let spawnError; child.on('error', error => { spawnError = error; });
    const deadline = performance.now() + (heapRequested ? 480000 : allocatorLibrary ? 600000 : 360000);
    const alive = () => { if (spawnError) throw spawnError; assert(child.exitCode === null && child.signalCode === null, 'Application exited'); if (sampleError) throw sampleError; assert(performance.now() < deadline, 'Response launch timed out'); };
    const marker = async name => {
      while (true) {
        alive();
        try { return await json(join(directory, `${name}.json`)); } catch (error) { if (error.code !== 'ENOENT') throw error; }
        try { const smoke = await json(join(directory, 'smoke.json')); assert.equal(smoke.payload.error, '', `Scenario failed: ${smoke.payload.error}`); }
        catch (error) { if (error.code !== 'ENOENT') throw error; }
        await delay(25);
      }
    };
    const ready = await marker('ready');
    assert.equal(ready.buildSha, sha); assert.equal(ready.debugAssertions, false); assert.equal(ready.inspector, false);
    assert.equal(ready.payload.error, ''); assert.equal(ready.payload.requests, 100);
    sampling = (async () => {
      while (!stopped) {
        try {
          known = await family(child.pid, startTicks, pageSize, hz);
          startTicks ??= known.find(p => p.pid === child.pid).startTicks;
          assert(known.some(p => /WebKit/.test(p.name)), 'Missing WebKit helper');
          const processes = [];
          for (const p of known) {
            const smaps = await readFile(`/proc/${p.pid}/smaps_rollup`, 'utf8'), pssBytes = parsePss(smaps);
            assert.equal(parseStat(await readFile(`/proc/${p.pid}/stat`, 'utf8'), pageSize, hz).startTicks, p.startTicks);
            processes.push({ ...p, pssBytes, ...(allocatorLibrary ? smapsCounters(smaps) : {}) });
          }
          points.push({ observed: performance.now(), previousMarkerIndex: phases.length - 1, rootPid: child.pid, pssBytes: processes.reduce((n, p) => n + p.pssBytes, 0), rssBytes: processes.reduce((n, p) => n + p.rssBytes, 0), processes });
        } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) { sampleError = error; return; } }
        await delay(100);
      }
    })();
    for (let index = 0; index < (fineProfile ? 11 : allocatorLibrary ? 29 : 28); index++) {
      const probe = await marker(`responses-${index}`), observed = performance.now(), phase = { probe, samples: [] };
      if (allocatorLibrary) assert.equal(probe.backendTimingDropped, 0, 'Diagnostic timing buffer overflowed');
      phases.push(phase);
      if (probe.payload.name === 'copy') {
        const point = probe.payload.point;
        const windows = command('xdotool', ['search', '--onlyvisible', '--pid', String(child.pid)]).split('\n');
        assert(windows.length, 'No native copy target');
        const candidates = windows.map(window => {
          const geometry = command('xdotool', ['getwindowgeometry', '--shell', window]);
          return { window, width: Number(/^WIDTH=(\d+)$/m.exec(geometry)?.[1]), height: Number(/^HEIGHT=(\d+)$/m.exec(geometry)?.[1]) };
        });
        // GTK also exposes a tiny child window; it is not the WebView click target.
        const { window, height } = copyWindow(candidates, point);
        command('xdotool', ['windowfocus', '--sync', window]);
        command('xdotool', ['mousemove', '--window', window, String(Math.round(point.x)), String(Math.round(point.y + height - point.innerHeight)), 'click', '1']);
        // Do not retain clipboard payloads. Hash the real X11 selection, including trailing bytes.
        await delay(150);
        const clipboard = execFileSync('xclip', ['-selection', 'clipboard', '-out'], { maxBuffer: 32 * 2 ** 20, timeout: 5000 });
        phase.clipboardSha256 = hash(clipboard);
        assert.equal(phase.clipboardSha256, probe.payload.content.sha256, 'Clipboard mismatch');
      } else {
        for (let sample = 0; sample < 6; sample++) {
          const target = sample ? phase.samples.at(-1).observed + 200 : observed + 3000;
          while (!points.some(p => p.observed >= target)) { alive(); await delay(20); }
          const point = points.find(p => p.observed >= target);
          phase.samples.push({ ...point, afterMarkerMs: point.observed - observed });
        }
      }
      if (allocatorLibrary && probe.payload.name !== 'copy') {
        phase.allocatorSamples = [];
        for (const proc of heapRequested ? [] : phase.samples.at(-1).processes) {
          let xml;
          try { xml = await readFile(join(directory, `allocator-${proc.pid}.xml`), 'utf8'); }
          catch (error) { if (error.code === 'ENOENT' && proc.pid !== child.pid) continue; throw error; }
          const allocator = parseAllocator(xml), ageMs = Number(process.hrtime.bigint() / 1000n) / 1000 - allocator.monotonicMs;
          assert.equal(allocator.pid, proc.pid); assert(ageMs >= 0 && ageMs < 5000, 'Stale allocator sample');
          assert.equal(parseStat(await readFile(`/proc/${proc.pid}/stat`, 'utf8'), pageSize, hz).startTicks, proc.startTicks);
          phase.allocatorSamples.push({ ...allocator, startTicks: proc.startTicks, ageMs, xml });
        }
        if (!heapRequested) assert(phase.allocatorSamples.some(sample => sample.pid === child.pid), 'Missing root allocator sample');
        phase.host = { loadAverage: loadavg(), cpuPressure: await readFile('/proc/pressure/cpu', 'utf8') };
        if (fineProfile) {
          phase.mappings=[];
          for (const proc of phase.samples.at(-1).processes) phase.mappings.push({pid:proc.pid,startTicks:proc.startTicks,groups:smapsGroups(await readFile(`/proc/${proc.pid}/smaps`,'utf8'))});
        }
      }
      if (heapRequested && probe.payload.name === 'four-responses') {
        const webkit=phase.samples.at(-1).processes.filter(p=>/^WebKitWebProces/.test(p.name)); assert.equal(webkit.length,1);
        heapSession=await attachHeapProfile(webkit[0],directory,pageSize,hz);
        await writeFile(join(directory,'heaptrack-ready'),'ready\n');
      }
      await checkpoint(phases, ready);
    }
    const smoke = await marker('smoke'); assert.equal(smoke.payload.error, '');
    stopped = true; await sampling;
    assert.equal(await collectionHash(join(directory, 'collection')), before, 'Scenario modified YAML');
    const checkedProfile = [];
    for (const path of await files(join(directory, 'profile'))) {
      const value = await readFile(path, 'utf8');
      assert(!/NP013_(JSON|LINE|TEXT|RUNTIME_SECRET_CANARY)|a{512}|x{512}/.test(value), 'Body or runtime secret reached persistent profile');
      checkedProfile.push({ path: path.slice(join(directory, 'profile').length + 1), sha256: hash(value) });
    }
    assert(checkedProfile.some(p => p.path === 'history.json') && checkedProfile.some(p => p.path === 'recovery.json'), 'Privacy check requires actual history and recovery files');
    outcome = { iteration, ready, smoke, phases, privacyPassed: true, privacy: { collectionSha256: before, checkedProfile }, traffic: structuredClone(server.traffic.slice(trafficStart)),
      peakObserved: { pssBytes: Math.max(...points.map(p => p.pssBytes)), rssBytes: Math.max(...points.map(p => p.rssBytes)),
        pss: points.reduce((a, b) => a.pssBytes >= b.pssBytes ? a : b), rss: points.reduce((a, b) => a.rssBytes >= b.rssBytes ? a : b), intervalMs: 100, samples: points.length } };
    return outcome;
  } catch (error) {
    failure=error;
    error.partialPhases = phases;
    error.message += stderr ? `\nApplication stderr: ${stderr}` : '';
    throw error;
  } finally {
    stopped = true; if (sampling) await sampling;
    if (child?.pid) {
      try { known = await family(child.pid, startTicks, pageSize, hz); } catch { /* Identity-check the last attributed set. */ }
      for (const p of known.reverse()) {
        try { if (parseStat(await readFile(`/proc/${p.pid}/stat`, 'utf8'), pageSize, hz).startTicks === p.startTicks) process.kill(p.pid, 'SIGTERM'); }
        catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
      }
      if (child.exitCode === null && child.signalCode === null) { child.kill('SIGTERM'); await Promise.race([new Promise(resolve => child.once('exit', resolve)), delay(2000)]); }
      if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    }
    try {
      if (heapSession) {
        try { const heapProfile=await heapSession.finish(); if(outcome) outcome.heapProfile=heapProfile; else if(failure) failure.heapProfile=heapProfile; }
        catch (error) { if(failure) failure.heapProfileError=error.message; else throw error; }
      }
    }
    finally { await rm(directory, { recursive: true, force: true }); }
  }
}
export async function main(args) {
  const [action, ...rest] = args, options = {};
  for (let i = 0; i < rest.length; i += 2) { assert(rest[i].startsWith('--') && rest[i + 1]); options[rest[i].slice(2)] = rest[i + 1]; }
  if (action === 'evaluate') {
    const report = await json(options.report), summary = summarizeResponses(report);
    const result = { protocol: responseProtocol, status: summary.budgetStatus, seriesId: report.seriesId,
      source: report.source, binary: report.binary, environment: report.environment, summary, absoluteTargetsEvaluated: false };
    await save(options.out, result); console.log(result.status); return result.status === 'passed' ? 0 : 2;
  }
  assert.equal(action, 'run'); assert.equal(process.platform, 'linux');
  const binary = await realpath(options.binary), output = resolve(options.out), launchCount = Number(options.launches ?? responseLaunchCount);
  assert([1, responseLaunchCount].includes(launchCount), 'Use three launches for ALPHA acceptance or one for diagnostics');
  assert(options.conditions?.length, 'Provide host/display/power/load conditions');
  const source = { sha: command('git', ['rev-parse', 'HEAD']), dirty: !!command('git', ['status', '--porcelain', '--untracked-files=no']) };
  assert.equal(source.dirty, false, 'Commit measured sources first');
  const bytes = await readFile(binary); assert(bytes.includes(Buffer.from('native-validation requires NIMBLEPOST_VALIDATION_DIR')));
  assert.equal(process.arch, 'x64', 'NP-013 acceptance currently supports Linux x64');
  assert(!process.env.LD_PRELOAD && !process.env.NP013_ALLOCATOR_DIAGNOSTIC_DIR, 'Do not contaminate acceptance with a preload');
  const allocatorLibrary = options['allocator-library'] ? await realpath(options['allocator-library']) : null;
  const heapRequested=options['heap-profile']==='true', uiProfile=options['ui-profile']==='true';
  assert(!(heapRequested && uiProfile),'Choose one fine profiling mode');
  const fineProfile=heapRequested || uiProfile; assert(!fineProfile || allocatorLibrary,'Fine profiling requires the diagnostic mode');
  if (allocatorLibrary) assert.equal(launchCount, 1, 'Allocator diagnostics are not acceptance series');
  const server = await serveResponses();
  const report = { schemaVersion: 1, protocol: heapRequested ? 'responses-linux-profile-v3' : uiProfile ? 'responses-linux-ui-profile-v2' : allocatorLibrary ? diagnosticProtocol : responseProtocol, seriesId: randomUUID(), status: 'running', source, launchCount, launches: [],
    mode: 'instrumented-release-ephemeral', absoluteTargetsEvaluated: false,
    binary: { sha256: hash(bytes), bytes: (await stat(binary)).size }, fixtures: server.fixtures,
    environment: { platform: process.platform, arch: process.arch, hostHash: hash(hostname()), cpu: cpus()[0].model, cpuCount: cpus().length,
      ramBytes: totalmem(), osRelease: release(), webview: command('dpkg-query', ['-W', '-f=${Version}', 'libwebkit2gtk-4.1-0']), conditions: options.conditions },
    ...(allocatorLibrary ? { diagnostics: { allocatorLibrarySha256: hash(await readFile(allocatorLibrary)), allocatorSamplesEnabled:!heapRequested, libc: command('getconf', ['GNU_LIBC_VERSION']), intervalMs: 1000, acceptanceEligible: false,
      ...(heapRequested ? { heapAttachPhase:'four-responses', requestedUnwindDepth:16 } : {}) } } : {}) };
  await mkdir(resolve(output, '..'), { recursive: true }); await save(output, report);
  try {
    for (let iteration = 1; iteration <= launchCount; iteration++) {
      report.launches.push(await runLaunch(binary, source.sha, server, iteration, async (phases, ready) => {
        report.partialLaunch = { iteration, ready, phases }; await save(output, report);
      }, allocatorLibrary, fineProfile, heapRequested));
      delete report.partialLaunch; await save(output, report);
      console.log(`responses launch=${iteration}/${launchCount}`);
    }
    report.status = launchCount === responseLaunchCount ? 'passed' : 'diagnostic';
    if (launchCount === responseLaunchCount) report.summary = summarizeResponses(report);
    await save(output, report); return report.summary?.budgetStatus === 'failed' ? 2 : 0;
  } catch (error) {
    report.status = 'failed'; report.error = error.message; report.partialPhases = error.partialPhases;
    if(error.heapProfile) report.heapProfile=error.heapProfile;
    if(error.heapProfileError) report.heapProfileError=error.heapProfileError;
    await save(output, report); throw error;
  } finally { await server.close(); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
