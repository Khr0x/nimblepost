import { readFile, writeFile, mkdir, rename, mkdtemp, rm, readdir, stat, realpath } from 'node:fs/promises';
import { spawn, execFileSync } from 'node:child_process';
import { tmpdir, hostname, cpus, totalmem, release } from 'node:os';
import { join, resolve } from 'node:path';
import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
import { family, fixtures, parseStat } from './native-performance.mjs';
import { hash } from './performance-report.mjs';
import { serveResponses } from './response-fixtures.mjs';
import { responseProtocol, summarizeResponses, compareResponses } from './response-report.mjs';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const command = (name, args) => execFileSync(name, args, { encoding: 'utf8', maxBuffer: 32 * 2 ** 20 }).trim();
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const save = async (path, value) => { await writeFile(`${path}.tmp`, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 }); await rename(`${path}.tmp`, path); };

export function parsePss(text) {
  const match = /^Pss:\s+(\d+) kB$/m.exec(text);
  assert(match, 'PSS unavailable'); return Number(match[1]) * 1024;
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
async function runLaunch(binary, sha, server, iteration) {
  const directory = await mkdtemp(join(tmpdir(), 'nimblepost-responses-'));
  const pageSize = Number(command('getconf', ['PAGESIZE'])), hz = Number(command('getconf', ['CLK_TCK']));
  let child, known = [], startTicks, sampling, stopped = false, sampleError, stderr = '';
  const points = [], phases = [], trafficStart = server.traffic.length;
  try {
    await fixtures(directory, 100);
    const workspace = await json(join(directory, 'profile/workspaces.json'));
    workspace.workspaces.push({ id: 2, name: 'Empty validation workspace', collections: [], activeCollection: null });
    await save(join(directory, 'profile/workspaces.json'), workspace);
    for (const path of await files(join(directory, 'collection'))) if (path.endsWith('.yml') && !path.endsWith('opencollection.yml')) {
      const source = await json(path);
      source.runtime = { variables: [{ name: 'np013-token', secret: true, type: 'string' }] };
      source.http.headers = [{ name: 'X-Probe-Secret', value: '{{np013-token}}' }];
      await save(path, source);
    }
    // Recovery refers to the original request; begin with clean tabs for this protocol.
    await save(join(directory, 'profile/recovery.json'), { version: 1, workspaceId: 1, activeTab: null, home: true, tabs: [] });
    await save(join(directory, 'plan.json'), { responses: true, url: server.url });
    const before = await collectionHash(join(directory, 'collection'));
    child = spawn(binary, [], { env: { ...process.env, NIMBLEPOST_VALIDATION_DIR: directory }, stdio: ['ignore', 'ignore', 'pipe'] });
    child.stderr.on('data', data => { stderr = (stderr + data).slice(-8192); });
    let spawnError; child.on('error', error => { spawnError = error; });
    const deadline = performance.now() + 360000;
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
            const pssBytes = parsePss(await readFile(`/proc/${p.pid}/smaps_rollup`, 'utf8'));
            assert.equal(parseStat(await readFile(`/proc/${p.pid}/stat`, 'utf8'), pageSize, hz).startTicks, p.startTicks);
            processes.push({ ...p, pssBytes });
          }
          points.push({ observed: performance.now(), rootPid: child.pid, pssBytes: processes.reduce((n, p) => n + p.pssBytes, 0), rssBytes: processes.reduce((n, p) => n + p.rssBytes, 0), processes });
        } catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) { sampleError = error; return; } }
        await delay(100);
      }
    })();
    for (let index = 0; index < 28; index++) {
      const probe = await marker(`responses-${index}`), observed = performance.now(), phase = { probe, samples: [] };
      phases.push(phase);
      if (probe.payload.name === 'copy') {
        const point = probe.payload.point;
        const windows = command('xdotool', ['search', '--onlyvisible', '--pid', String(child.pid)]).split('\n');
        assert(windows.length, 'No native copy target');
        const window = windows.at(-1);
        const geometry = command('xdotool', ['getwindowgeometry', '--shell', window]);
        const height = Number(/^HEIGHT=(\d+)$/m.exec(geometry)?.[1]); assert(height >= point.innerHeight);
        command('xdotool', ['windowfocus', '--sync', window]);
        command('xdotool', ['mousemove', '--window', window, String(Math.round(point.x)), String(Math.round(point.y + height - point.innerHeight)), 'click', '1']);
        // Do not retain clipboard payloads. Hash the real X11 selection, including trailing bytes.
        await delay(150);
        const clipboard = execFileSync('xclip', ['-selection', 'clipboard', '-out'], { maxBuffer: 32 * 2 ** 20, timeout: 5000 });
        phase.clipboardSha256 = hash(clipboard);
        assert.equal(phase.clipboardSha256, probe.payload.content.sha256, 'Clipboard mismatch');
      } else {
        for (let sample = 0; sample < 6; sample++) {
          const target = observed + 3000 + sample * 210;
          while (!points.some(p => p.observed >= target)) { alive(); await delay(20); }
          const point = points.find(p => p.observed >= target);
          phase.samples.push({ ...point, afterMarkerMs: point.observed - observed });
        }
      }
    }
    const smoke = await marker('smoke'); assert.equal(smoke.payload.error, '');
    stopped = true; await sampling;
    assert.equal(await collectionHash(join(directory, 'collection')), before, 'Scenario modified YAML');
    for (const path of await files(join(directory, 'profile'))) {
      const value = await readFile(path, 'utf8');
      assert(!/NP013_(JSON|LINE|TEXT|RUNTIME_SECRET_CANARY)/.test(value), 'Body or runtime secret reached persistent profile');
    }
    return { iteration, ready, smoke, phases, privacyPassed: true, traffic: structuredClone(server.traffic.slice(trafficStart)),
      peakObserved: { pssBytes: Math.max(...points.map(p => p.pssBytes)), rssBytes: Math.max(...points.map(p => p.rssBytes)), intervalMs: 100, samples: points.length } };
  } catch (error) {
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
    await rm(directory, { recursive: true, force: true });
  }
}
export async function main(args) {
  const [action, ...rest] = args, options = {};
  for (let i = 0; i < rest.length; i += 2) { assert(rest[i].startsWith('--') && rest[i + 1]); options[rest[i].slice(2)] = rest[i + 1]; }
  if (action === 'compare') {
    const result = compareResponses(await json(options.reference), await json(options.candidate));
    await save(options.out, result); console.log(result.status); return result.status === 'passed' ? 0 : 2;
  }
  assert.equal(action, 'run'); assert.equal(process.platform, 'linux');
  const binary = await realpath(options.binary), output = resolve(options.out), launchCount = Number(options.launches ?? 5);
  assert(Number.isInteger(launchCount) && launchCount >= 1 && launchCount <= 30);
  assert(options.conditions?.length, 'Provide host/display/power/load conditions');
  const source = { sha: command('git', ['rev-parse', 'HEAD']), dirty: !!command('git', ['status', '--porcelain', '--untracked-files=no']) };
  assert.equal(source.dirty, false, 'Commit measured sources first');
  const bytes = await readFile(binary); assert(bytes.includes(Buffer.from('native-validation requires NIMBLEPOST_VALIDATION_DIR')));
  const server = await serveResponses();
  const report = { schemaVersion: 1, protocol: responseProtocol, seriesId: randomUUID(), status: 'running', source, launchCount, launches: [],
    binary: { sha256: hash(bytes), bytes: (await stat(binary)).size }, fixtures: server.fixtures,
    environment: { platform: process.platform, arch: process.arch, hostHash: hash(hostname()), cpu: cpus()[0].model, cpuCount: cpus().length,
      ramBytes: totalmem(), osRelease: release(), webview: command('dpkg-query', ['-W', '-f=${Version}', 'libwebkit2gtk-4.1-0']), conditions: options.conditions } };
  await mkdir(resolve(output, '..'), { recursive: true }); await save(output, report);
  try {
    for (let iteration = 1; iteration <= launchCount; iteration++) {
      report.launches.push(await runLaunch(binary, source.sha, server, iteration)); await save(output, report);
      console.log(`responses launch=${iteration}/${launchCount}`);
    }
    report.status = launchCount >= 5 ? 'passed' : 'diagnostic';
    if (launchCount >= 5) report.summary = summarizeResponses(report);
    await save(output, report); return report.summary?.budgetStatus === 'failed' ? 2 : 0;
  } catch (error) {
    report.status = 'failed'; report.error = error.message; report.partialPhases = error.partialPhases; await save(output, report); throw error;
  } finally { await server.close(); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
