import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';

export const protocol = 'startup-rss-linux-v2';
export const counts = [0, 100, 5000];
export const hash = value => createHash('sha256').update(value).digest('hex');
export const datasetHash = count => hash(`nimblepost-fixtures-v1:${count}:recovered-url-draft:ephemeral-webview`);
const canonical = value => JSON.stringify(value, (_, v) => v && typeof v === 'object' && !Array.isArray(v)
  ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => a.localeCompare(b))) : v);
const positive = n => Number.isFinite(n) && n > 0;

export function stats(values) {
  assert(values.length && values.every(n => Number.isFinite(n) && n >= 0), 'Invalid measurements');
  const sorted = [...values].sort((a, b) => a - b), n = sorted.length;
  return { min: sorted[0], max: sorted[n - 1], median: n % 2 ? sorted[(n - 1) / 2] : (sorted[n / 2 - 1] + sorted[n / 2]) / 2,
    p95: sorted[Math.ceil(n * .95) - 1] };
}

export function summarize(scenario) {
  return { launchToReadyMs: stats(scenario.samples.map(s => s.launchToReadyMs)),
    launchToProfileMs: stats(scenario.samples.map(s => s.launchToProfileMs)),
    familyIdleRssBytes: stats(scenario.samples.map(s => stats(s.idle.map(i => i.rssBytes)).median)) };
}

export function validateReport(report) {
  assert.equal(report.schemaVersion, 1, 'Unsupported report schema');
  assert.equal(report.protocol, protocol, 'Incompatible protocol');
  assert.equal(report.status, 'passed', 'Incomplete or failed series');
  assert.equal(report.mode, 'instrumented-release-ephemeral', 'Profiler or incompatible mode');
  assert.equal(report.absoluteTargetsEvaluated, false);
  assert.match(report.seriesId, /^[0-9a-f-]{36}$/);
  assert.match(report.binary.sha256, /^[0-9a-f]{64}$/);
  assert.match(report.source.sha, /^[0-9a-f]{40}$/);
  assert.equal(report.source.dirty, false, 'Build provenance must be committed');
  assert(Number.isInteger(report.launches) && report.launches >= 5 && report.launches <= 30);
  const env = report.environment;
  for (const key of ['hostHash', 'platform', 'arch', 'cpu', 'osRelease', 'webview', 'conditions']) assert(typeof env[key] === 'string' && env[key].length, `Missing ${key}`);
  assert.equal(env.platform, 'linux');
  assert.match(env.hostHash, /^[0-9a-f]{64}$/);
  assert(positive(env.ramBytes) && positive(env.cpuCount));
  assert.deepEqual(report.scenarios.map(s => s.requests), counts, 'Missing or duplicate scenario');
  for (const scenario of report.scenarios) {
    assert.equal(scenario.datasetHash, datasetHash(scenario.requests));
    assert.equal(scenario.samples.length, report.launches, 'Partial scenario');
    assert.deepEqual(scenario.samples.map(s => s.iteration), Array.from({ length: report.launches }, (_, i) => i + 1));
    for (const sample of scenario.samples) {
      assert(positive(sample.launchToReadyMs) && sample.launchToProfileMs >= sample.launchToReadyMs, 'Invalid startup boundary');
      assert.equal(sample.checksPassed, true, 'Failed functional checks');
      assert.equal(sample.idle.length, 6, 'Partial idle window');
      assert(sample.idle[0].afterProfileMs >= 3000, 'Idle window started too early');
      for (let i = 0; i < sample.idle.length; i++) {
        const reading = sample.idle[i];
        if (i) assert(reading.afterProfileMs - sample.idle[i - 1].afterProfileMs >= 1000, 'Idle samples too close');
        assert(reading.processes.length >= 2 && reading.processes.some(p => /WebKit/.test(p.name)), 'WebView family missing');
        assert.equal(new Set(reading.processes.map(p => p.pid)).size, reading.processes.length, 'Duplicate PID');
        const attributed = new Set([reading.rootPid]);
        assert(reading.processes.some(p => p.pid === reading.rootPid), 'Missing root process');
        for (let grew = true; grew;) { grew = false; for (const p of reading.processes) if (attributed.has(p.ppid) && !attributed.has(p.pid)) { attributed.add(p.pid); grew = true; } }
        assert.equal(attributed.size, reading.processes.length, 'Unattributed process in family');
        assert(reading.processes.every(p => Number.isInteger(p.pid) && p.pid > 0 && positive(p.startTicks) && Number.isFinite(p.rssBytes) && p.rssBytes >= 0));
        assert.equal(reading.rssBytes, reading.processes.reduce((n, p) => n + p.rssBytes, 0), 'RSS sum differs from process readings');
        assert(positive(reading.rssBytes));
      }
    }
  }
  return report.scenarios.map(summarize);
}

export function compareReports(reference, candidate, confirmation = null, toolingCheck = false) {
  const base = validateReport(reference), next = validateReport(candidate);
  assert.notEqual(reference.seriesId, candidate.seriesId, 'Reused series');
  assert.equal(canonical(reference.environment), canonical(candidate.environment), 'Incompatible environment');
  assert.equal(reference.launches, candidate.launches, 'Different launch count');
  let confirmed;
  if (confirmation) {
    confirmed = validateReport(confirmation);
    assert(![reference.seriesId, candidate.seriesId].includes(confirmation.seriesId), 'Confirmation reuses a series');
    assert.equal(canonical(candidate.environment), canonical(confirmation.environment), 'Incompatible confirmation');
    assert.equal(candidate.binary.sha256, confirmation.binary.sha256, 'Confirmation is a different binary');
    assert.equal(candidate.source.sha, confirmation.source.sha, 'Confirmation is a different source revision');
    assert.equal(candidate.launches, confirmation.launches);
  }
  const variations = base.flatMap((scenario, index) => ['launchToProfileMs', 'familyIdleRssBytes'].map(metric => {
    const delta = (value) => (value / scenario[metric].median - 1) * 100;
    return { requests: counts[index], metric, percent: delta(next[index][metric].median),
      confirmationPercent: confirmed ? delta(confirmed[index][metric].median) : null };
  }));
  // Roundoff at inclusive percentage boundaries must not turn 15% into 14.999999%.
  const reaches = (value, threshold) => value >= threshold - 1e-9;
  const status = variations.some(v => reaches(v.percent, 15) && v.confirmationPercent !== null && reaches(v.confirmationPercent, 15)) ? 'blocked'
    : variations.some(v => reaches(v.percent, 15)) && !confirmation ? 'needs-confirmation'
    : variations.some(v => reaches(v.percent, 10)) ? 'warning' : 'passed';
  return { schemaVersion: 1, protocol, status, referenceSeries: reference.seriesId, candidateSeries: candidate.seriesId,
    confirmationSeries: confirmation?.seriesId ?? null, toolingCheck, absoluteTargetsEvaluated: false,
    releaseGateEvaluated: !toolingCheck, variations, referenceSummary: base, candidateSummary: next };
}
