import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { counts, protocol, datasetHash, stats, summarize, validateReport, compareReports } from './performance-report.mjs';
import { fixtures, parseStat, main } from './native-performance.mjs';

// Synthetic measurements exercise the contract; these are not benchmark evidence.
function report(startup = 100, rss = 1000) {
  return { schemaVersion: 1, protocol, seriesId: randomUUID(), status: 'passed', mode: 'instrumented-release-ephemeral', absoluteTargetsEvaluated: false,
    launches: 5, source: { sha: 'a'.repeat(40), dirty: false }, binary: { sha256: 'b'.repeat(64) },
    environment: { hostHash: 'c'.repeat(64), platform: 'linux', arch: 'x64', cpu: 'fixture CPU', cpuCount: 2, ramBytes: 4096,
      osRelease: 'fixture OS', webview: 'fixture WebKit', conditions: 'fixture display/power/load' },
    scenarios: counts.map(count => ({ requests: count, datasetHash: datasetHash(count), samples: Array.from({ length: 5 }, (_, i) => ({
      iteration: i + 1, launchToReadyMs: 10, launchToProfileMs: startup, checksPassed: true,
      idle: Array.from({ length: 6 }, (_, n) => ({ rootPid: 1, afterProfileMs: 3000 + n * 1000, rssBytes: rss, processes: [
        { pid: 1, ppid: 0, startTicks: 1, name: 'NimblePost', rssBytes: rss / 2 },
        { pid: 2, ppid: 1, startTicks: 2, name: 'WebKitWebProces', rssBytes: rss / 2 },
      ] })),
    })) })) };
}

test('statistics retain first sample/outliers and use nearest-rank p95', () => {
  assert.deepEqual(stats([1000, 1, 2, 3, 4]), { min: 1, max: 1000, median: 3, p95: 1000 });
  assert.equal(stats([1, 2, 3, 4]).median, 2.5);
  assert.throws(() => stats([NaN]));
  const r = report(); r.scenarios[0].samples[0].launchToProfileMs = 500;
  assert.equal(summarize(r.scenarios[0]).launchToProfileMs.p95, 500);
});

test('partial, failed, malformed and uninstrumented series are rejected', () => {
  validateReport(report());
  const mutations = [r => r.status = 'running', r => r.status = 'failed', r => r.mode = 'inspector', r => r.launches = 4,
    r => r.source.dirty = true, r => r.scenarios.pop(), r => r.scenarios[0].samples.pop(), r => r.scenarios[0].samples[0].checksPassed = false,
    r => r.scenarios[0].samples[0].idle.pop(), r => r.scenarios[0].samples[0].idle[0].processes.pop(),
    r => r.scenarios[0].samples[0].idle[0].rssBytes++, r => r.scenarios[0].samples[0].idle[1].afterProfileMs = 3500,
    r => r.scenarios[0].samples[0].launchToProfileMs = NaN, r => r.scenarios[0].datasetHash = 'other',
    r => r.scenarios[0].samples[0].idle[0].rootPid = 99, r => r.scenarios[0].samples[0].idle[0].processes[1].ppid = 99];
  for (const mutate of mutations) { const r = report(); mutate(r); assert.throws(() => validateReport(r), mutate.toString()); }
});

test('comparison recalculates summaries, rejects incompatible environments and reused series', () => {
  const base = report(), candidate = report(); candidate.scenarios[0].summary = { launchToProfileMs: { median: 9999 } };
  assert.equal(compareReports(base, candidate).status, 'passed');
  assert.throws(() => compareReports(base, base));
  for (const key of Object.keys(base.environment)) {
    const r = report(); r.environment[key] = typeof r.environment[key] === 'number' ? r.environment[key] + 1 : r.environment[key] + 'different';
    assert.throws(() => compareReports(base, r), key);
  }
  const r = report(); r.protocol = 'historical-v1'; assert.throws(() => compareReports(base, r));
});

test('inclusive 10/15 percent thresholds and independent confirmations follow RFC exit states', () => {
  const base = report();
  assert.equal(compareReports(base, report(109.99)).status, 'passed');
  assert.equal(compareReports(base, report(110)).status, 'warning');
  assert.equal(compareReports(base, report(115)).status, 'needs-confirmation');
  assert.equal(compareReports(base, report(115), report(115)).status, 'blocked');
  assert.equal(compareReports(base, report(115), report(114)).status, 'warning');
  assert.equal(compareReports(base, report(100), report(115)).status, 'warning', 'A divergent repetition must not be hidden');
  assert.equal(compareReports(base, report(100, 1150), report(100, 1150)).status, 'blocked');
  const candidate = report(115), confirmation = report(115);
  confirmation.binary.sha256 = 'd'.repeat(64); assert.throws(() => compareReports(base, candidate, confirmation));
  assert.throws(() => compareReports(base, candidate, candidate));
  const comparison = compareReports(base, report(), null, true);
  assert.equal(comparison.releaseGateEvaluated, false); assert.equal(comparison.absoluteTargetsEvaluated, false);
});

test('fixture profiles are deterministic, synthetic, isolated and recover a draft', async () => {
  const root = await mkdtemp(join(tmpdir(), 'nimblepost-fixtures-'));
  try {
    for (const count of counts) {
      const directory = join(root, String(count)); await fixtures(directory, count);
      assert.equal((await readdir(join(directory, 'collection'))).length, count + 1);
      const registry = JSON.parse(await readFile(join(directory, 'profile/workspaces.json')));
      const recovery = JSON.parse(await readFile(join(directory, 'profile/recovery.json')));
      assert.equal(registry.workspaces[0].collections.length, count ? 1 : 0);
      assert.equal(recovery.tabs.length, count ? 1 : 0);
      if (count) {
        assert.equal(recovery.tabs[0].root, join(directory, 'collection'));
        assert.notEqual(recovery.tabs[0].value.http.url, recovery.tabs[0].source.http.url);
      }
    }
  } finally { await rm(root, { recursive: true, force: true }); }
});

test('/proc parser handles parenthesized command names and exact RSS units', () => {
  const fields = Array(22).fill('0'); fields[0] = 'S'; fields[1] = '5'; fields[11] = '30'; fields[12] = '20'; fields[19] = '999'; fields[21] = '12';
  assert.deepEqual(parseStat(`42 (WebKit (worker)) ${fields.join(' ')}`, 4096, 100), {
    pid: 42, name: 'WebKit (worker)', ppid: 5, startTicks: 999, rssBytes: 49152, cpuMs: 500,
  });
  assert.throws(() => parseStat('bad', 4096, 100));
});

test('CLI requires explicit reference selection and returns confirmation/blocking codes', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'nimblepost-compare-'));
  const { writeFile } = await import('node:fs/promises');
  try {
    for (const [name, r] of [['base', report()], ['candidate', report(115)], ['confirmation', report(115)]]) await writeFile(join(directory, name), JSON.stringify(r));
    const args = ['compare', '--reference', join(directory, 'base'), '--candidate', join(directory, 'candidate'), '--out', join(directory, 'result')];
    await assert.rejects(main(args));
    assert.equal(await main([...args, '--tooling-check', 'true']), 3);
    assert.equal(await main([...args, '--accept-reference', 'true', '--confirmation', join(directory, 'confirmation')]), 2);
  } finally { await rm(directory, { recursive: true, force: true }); }
});
