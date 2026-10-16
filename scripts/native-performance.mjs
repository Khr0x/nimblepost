import { readFile, writeFile, rename, mkdir, readdir, rm, mkdtemp, realpath, stat } from 'node:fs/promises';
import { tmpdir, hostname, cpus, totalmem, release } from 'node:os';
import { resolve, join } from 'node:path';
import { spawn, execFileSync } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import assert from 'node:assert/strict';
import { fileURLToPath } from 'node:url';
import { counts, protocol, hash, datasetHash, summarize, validateReport, compareReports } from './performance-report.mjs';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const json = async path => JSON.parse(await readFile(path, 'utf8'));
const save = async (path, value) => {
  await writeFile(`${path}.tmp`, JSON.stringify(value, null, 2) + '\n', { mode: 0o600 });
  await rename(`${path}.tmp`, path);
};
const command = (name, args) => execFileSync(name, args, { encoding: 'utf8' }).trim();

export async function fixtures(directory, count) {
  assert(counts.includes(count));
  const root = join(directory, 'collection'), profile = join(directory, 'profile');
  await mkdir(root, { recursive: true }); await mkdir(profile, { recursive: true });
  await writeFile(join(root, 'opencollection.yml'), 'opencollection: "1.0.0"\ninfo:\n  name: Synthetic benchmark\n');
  const source = { info: { name: 'Request 00000', type: 'http' }, http: { method: 'GET', url: 'http://127.0.0.1:3000/users' } };
  for (let n = 0; n < count; n++) await writeFile(join(root, `request-${String(n).padStart(5, '0')}.yml`), JSON.stringify({ ...source,
    info: { ...source.info, name: `Request ${String(n).padStart(5, '0')}` } }));
  await save(join(profile, 'workspaces.json'), { version: 2, activeWorkspace: 1, workspaces: [{ id: 1, name: 'Benchmark',
    collections: count ? [{ root, name: 'Synthetic benchmark', readOnly: false }] : [], activeCollection: count ? root : null }] });
  const value = structuredClone(source); value.http.url += '?draft=1';
  await save(join(profile, 'recovery.json'), { version: 1, workspaceId: 1, activeTab: count ? 1 : null, home: !count,
    tabs: count ? [{ id: 1, root, path: 'request-00000.yml', environment: '', value,
      edits: [{ path: ['http', 'url'], value: value.http.url }], source }] : [] });
}

export function parseStat(text, pageSize, hz) {
  const end = text.lastIndexOf(')'), fields = text.slice(end + 2).trim().split(/\s+/);
  assert(end > 0 && fields.length >= 22, 'Invalid /proc stat');
  return { pid: Number(text.slice(0, text.indexOf(' '))), name: text.slice(text.indexOf('(') + 1, end), ppid: Number(fields[1]),
    startTicks: Number(fields[19]), rssBytes: Number(fields[21]) * pageSize, cpuMs: (Number(fields[11]) + Number(fields[12])) * 1000 / hz };
}

export async function family(pid, startTicks, pageSize, hz) {
  const table = [];
  for (const entry of await readdir('/proc')) if (/^\d+$/.test(entry)) {
    try { table.push(parseStat(await readFile(`/proc/${entry}/stat`, 'utf8'), pageSize, hz)); } catch (error) {
      if (!['ENOENT', 'ESRCH', 'EACCES'].includes(error.code)) throw error;
    }
  }
  assert(table.some(p => p.pid === pid && (!startTicks || p.startTicks === startTicks)), 'Measured process exited or PID was reused');
  const ids = new Set([pid]);
  for (let grew = true; grew;) { grew = false; for (const p of table) if (ids.has(p.ppid) && !ids.has(p.pid)) { ids.add(p.pid); grew = true; } }
  return table.filter(p => ids.has(p.pid));
}

async function launch(binary, directory, count, sha) {
  const pageSize = Number(command('getconf', ['PAGESIZE'])), hz = Number(command('getconf', ['CLK_TCK']));
  const started = performance.now(), child = spawn(binary, [], { env: { ...process.env, NIMBLEPOST_VALIDATION_DIR: directory }, stdio: ['ignore', 'ignore', 'pipe'] });
  let stderr = '', spawnError; child.stderr.on('data', chunk => { stderr = (stderr + chunk).slice(-16384); }); child.on('error', error => { spawnError = error; });
  let known = [];
  const deadline = started + 90000;
  const marker = async name => {
    while (performance.now() < deadline) {
      if (spawnError) throw spawnError;
      assert(child.exitCode === null && child.signalCode === null, `Application exited (${child.exitCode})`);
      try { return { data: await json(join(directory, `${name}.json`)), observed: performance.now() }; }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
      await delay(10);
    }
    throw new Error(`Timed out waiting for ${name}`);
  };
  try {
    const ready = await marker('ready'), profile = await marker('profile');
    assert.equal(ready.data.buildSha, sha, 'Binary was built from a different/unrecorded source revision');
    assert.equal(ready.data.debugAssertions, false, 'Debug builds cannot be benchmarked as release');
    assert.equal(ready.data.inspector, false, 'Inspector builds cannot be used for acceptance');
    const p = ready.data.payload, tree = profile.data.payload;
    assert.equal(p.error, ''); assert.equal(p.rendered, true); assert.equal(p.requests, count); assert.equal(p.tabs, count ? 1 : 0);
    if (count) { assert.equal(p.url, 'http://127.0.0.1:3000/users?draft=1'); assert.equal(p.urlEnabled, true); assert.equal(p.untitled, false); }
    assert.equal(tree.error, ''); assert.equal(tree.labelsReady, true);
    assert(tree.treeRows <= tree.rowLimit, 'Tree rows exceed viewport bound');
    if (count) { assert.equal(tree.activeRequestRendered, true); assert(tree.requestRows > 0); assert.equal(tree.labeledRequestRows, tree.requestRows); }
    known = await family(child.pid, null, pageSize, hz); const startTicks = known.find(p => p.pid === child.pid).startTicks;
    const idle = []; await delay(3000);
    for (let i = 0; i < 6; i++) {
      if (i) await delay(1000);
      known = await family(child.pid, startTicks, pageSize, hz);
      assert(known.some(p => /WebKit/.test(p.name)), 'No attributed WebKit helper');
      idle.push({ rootPid: child.pid, afterProfileMs: performance.now() - profile.observed, rssBytes: known.reduce((n, p) => n + p.rssBytes, 0), processes: known });
    }
    const smoke = (await marker('smoke')).data;
    assert.equal(smoke.payload.error, '');
    for (const check of ['lazy-panel-chunks', 'bundled-example', 'request-ipc', 'collection-refresh', ...(count ? ['recovery-read-write'] : [])]) assert(smoke.payload.checks.includes(check), `Missing ${check}`);
    return { launchToReadyMs: ready.observed - started, launchToProfileMs: profile.observed - started,
      checksPassed: true, idle, probes: { ready: ready.data, profile: profile.data, smoke } };
  } catch (error) {
    error.message += stderr ? `\nApplication stderr: ${stderr}` : '';
    throw error;
  } finally {
    if (child.pid) {
      try { known = await family(child.pid, null, pageSize, hz); } catch { /* Last attributed PIDs remain identity-checked below. */ }
      for (const p of known.reverse()) {
        try { if (parseStat(await readFile(`/proc/${p.pid}/stat`, 'utf8'), pageSize, hz).startTicks === p.startTicks) process.kill(p.pid, 'SIGTERM'); }
        catch (error) { if (!['ENOENT', 'ESRCH'].includes(error.code)) throw error; }
      }
      if (child.exitCode === null && child.signalCode === null) { child.kill('SIGTERM'); await Promise.race([new Promise(resolve => child.once('exit', resolve)), delay(2000)]); }
      if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    }
    await writeFile(join(directory, 'stderr.log'), stderr, { mode: 0o600 });
  }
}

async function run(options) {
  assert.equal(process.platform, 'linux', 'Only Linux process-family attribution is supported');
  const binary = await realpath(options.binary), output = resolve(options.out), launches = Number(options.launches ?? 5);
  assert(Number.isInteger(launches) && launches >= 5 && launches <= 30);
  assert(options.conditions?.length, 'Provide --conditions (display, power and workload)');
  const source = { sha: command('git', ['rev-parse', 'HEAD']), dirty: !!command('git', ['status', '--porcelain', '--untracked-files=no']) };
  assert.equal(source.dirty, false, 'Commit measured source changes first');
  const bytes = await readFile(binary); assert(bytes.includes(Buffer.from('native-validation requires NIMBLEPOST_VALIDATION_DIR')), 'Binary lacks native validation hooks');
  await mkdir(resolve(output, '..'), { recursive: true });
  const report = { schemaVersion: 1, protocol, seriesId: randomUUID(), status: 'running', mode: 'instrumented-release-ephemeral',
    absoluteTargetsEvaluated: false, launches, source, binary: { sha256: hash(bytes), bytes: (await stat(binary)).size },
    environment: { hostHash: hash(hostname()), platform: process.platform, arch: process.arch, cpu: cpus()[0].model, cpuCount: cpus().length,
      ramBytes: totalmem(), osRelease: release(), webview: command('dpkg-query', ['-W', '-f=${Version}', 'libwebkit2gtk-4.1-0']), conditions: options.conditions }, scenarios: [] };
  await save(output, report);
  try {
    for (const count of counts) {
      const scenario = { requests: count, datasetHash: datasetHash(count), samples: [] }; report.scenarios.push(scenario);
      for (let iteration = 1; iteration <= launches; iteration++) {
        const directory = await mkdtemp(join(tmpdir(), 'nimblepost-performance-'));
        try { await fixtures(directory, count); scenario.samples.push({ iteration, ...await launch(binary, directory, count, source.sha) }); }
        finally { await rm(directory, { recursive: true, force: true }); }
        await save(output, report); console.log(`requests=${count} launch=${iteration}/${launches}`);
      }
      scenario.summary = summarize(scenario);
    }
    report.status = 'passed'; validateReport(report); await save(output, report);
  } catch (error) { report.status = 'failed'; report.error = error.message; await save(output, report); throw error; }
}

export async function main(args) {
  const [action, ...rest] = args, options = {};
  for (let i = 0; i < rest.length; i += 2) { assert(rest[i].startsWith('--') && rest[i + 1], 'Expected --option value'); options[rest[i].slice(2)] = rest[i + 1]; }
  if (action === 'run') { await run(options); return 0; }
  assert.equal(action, 'compare', 'Use run or compare');
  assert(options['tooling-check'] === 'true' || options['accept-reference'] === 'true', 'Explicitly accept the reference or select --tooling-check true');
  const result = compareReports(await json(options.reference), await json(options.candidate), options.confirmation ? await json(options.confirmation) : null, options['tooling-check'] === 'true');
  await save(options.out, result); console.log(result.status);
  return { passed: 0, warning: 0, 'needs-confirmation': 3, blocked: 2 }[result.status];
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { process.exitCode = await main(process.argv.slice(2)); } catch (error) { console.error(error.message); process.exitCode = 1; }
}
