import assert from 'node:assert/strict';
import { stats } from './performance-report.mjs';
import { responseFixtures, fixtureIdentity } from './response-fixtures.mjs';

export const responseProtocol = 'responses-linux-v2';
export const responseAcceptancePolicy = 'alpha-responses-v3';
export const responseLaunchCount = 3;
export const responsePhases = ['four-responses', 'json', 'copy', 'line', 'copy', 'excess', 'after-cancel', 'closed', 'reset'];
const actionBudgets = {
  chunk: { p95: 150, max: 250 }, scroll: { p95: 150, max: 250 },
  wrap: { p95: 100, max: 250 },
  'tab-empty-loading': { p95: 250, max: 250 }, 'tab-partial': { p95: 250, max: 250 },
  'tab-empty': { p95: 250, max: 250 }, 'tab-response': { p95: 250, max: 250 },
  cancel: { max: 1000 },
};
const MiB = 2 ** 20;
const memoryBudget = { minimumBytes: 16 * MiB, relativeGrowth: .10 };
const median = values => stats(values).median;
const signedStats = values => { const min = Math.min(...values); return Object.fromEntries(Object.entries(stats(values.map(v => v - min))).map(([key, value]) => [key, value + min])); };
const numeric = value => Number.isFinite(value) && value >= 0;
function validateFamily(sample) {
  assert(sample.processes.some(p => /WebKit/.test(p.name)), 'Missing attributed WebKit helper');
  const ids = new Set(sample.processes.map(p => p.pid)); assert.equal(ids.size, sample.processes.length);
  assert(ids.has(sample.rootPid));
  for (const proc of sample.processes) {
    assert(numeric(proc.pssBytes) && numeric(proc.rssBytes) && proc.startTicks > 0);
    const chain = new Set(); let current = proc;
    while (current.pid !== sample.rootPid) {
      assert(!chain.has(current.pid)); chain.add(current.pid);
      current = sample.processes.find(p => p.pid === current.ppid); assert(current, 'Unattributed process');
    }
  }
  assert.equal(sample.pssBytes, sample.processes.reduce((sum, p) => sum + p.pssBytes, 0));
  assert.equal(sample.rssBytes, sample.processes.reduce((sum, p) => sum + p.rssBytes, 0));
}
export function summarizeResponses(report) {
  assert.equal(report.schemaVersion, 1); assert.equal(report.protocol, responseProtocol); assert.equal(report.status, 'passed');
  assert.equal(report.mode, 'instrumented-release-ephemeral'); assert.equal(report.absoluteTargetsEvaluated, false);
  assert(report.seriesId && /^[a-f0-9]{40}$/.test(report.source.sha) && !report.source.dirty);
  assert(/^[a-f0-9]{64}$/.test(report.binary.sha256));
  assert.equal(report.environment.platform, 'linux'); assert.equal(report.environment.arch, 'x64');
  assert.deepEqual(report.fixtures, fixtureIdentity(responseFixtures()), 'Unknown fixture recipe');
  assert(report.launches.length === responseLaunchCount && report.launchCount === responseLaunchCount, 'Three complete launches required');
  const interactions = [], cancellations = [], deltas = [], resets = [], byAction = {};
  const failures = [];
  for (const [i, launch] of report.launches.entries()) {
    assert.equal(launch.iteration, i + 1); assert.equal(launch.smoke.payload.error, '');
    assert(launch.smoke.payload.checks.includes('large-responses'));
    assert.equal(launch.ready.buildSha, report.source.sha); assert.equal(launch.ready.debugAssertions, false); assert.equal(launch.ready.inspector, false);
    assert.equal(launch.phases.length, 1 + 3 * responsePhases.length);
    assert.equal(launch.phases[0].probe.payload.name, 'baseline');
    const launchResets = [];
    const actionNames = [];
    for (const [index, phase] of launch.phases.entries()) {
      const { probe } = phase, p = probe.payload;
      assert.equal(p.index, index); assert.equal(probe.buildSha, report.source.sha);
      assert.equal(probe.debugAssertions, false); assert.equal(probe.inspector, false);
      assert.equal(p.recoveryWarning, '');
      const capacity = probe.backendMemory.responseCapacity;
      assert(Number.isInteger(capacity) && capacity >= probe.backendMemory.responseBytes && capacity <= 16 * MiB, 'Receive capacity exceeded the body budget');
      if (p.name === 'closed' || p.name === 'reset') assert.equal(capacity, 0);
      if (index) { assert.equal(p.cycle, Math.floor((index - 1) / responsePhases.length) + 1); assert.equal(p.name, responsePhases[(index - 1) % responsePhases.length]); }
      for (const action of p.actions) {
        assert(Object.hasOwn(actionBudgets, action.name), 'Unknown response action');
        assert(numeric(action.durationMs) && action.durationMs > 0);
        actionNames.push(action.name);
        (byAction[action.name] ??= []).push(action.durationMs);
        (action.name === 'cancel' ? cancellations : interactions).push(action.durationMs);
      }
      if (p.name === 'copy') {
        assert.deepEqual(p.content, report.fixtures[p.kind]);
        assert.equal(phase.clipboardSha256, p.content.sha256, 'Real clipboard content differs');
        continue;
      }
      assert.equal(phase.samples.length, 6);
      for (const [j, sample] of phase.samples.entries()) {
        assert(sample.afterMarkerMs >= 3000 && (!j || sample.afterMarkerMs - phase.samples[j - 1].afterMarkerMs >= 199));
        validateFamily(sample);
      }
      if (p.name === 'json' || p.name === 'line' || p.name === 'four-responses' || p.name === 'after-cancel') {
        assert.deepEqual(p.content, report.fixtures[p.name === 'json' ? 'json' : p.name === 'line' ? 'line' : 'text']);
        assert.equal(p.activeOffset, p.bodyBytes); assert.equal(p.bodyBytes, p.content.bytes); assert.equal(p.error, '');
      }
      if (p.name === 'four-responses') {
        assert.equal(p.tabs.length, 64); assert.equal(p.cachedBodyChars > 0, true);
        assert.deepEqual(p.loaded, Array(4).fill(report.fixtures.text));
      }
      if (p.name === 'excess') { assert.match(p.error, /16777216/); assert.equal(p.bodyBytes, 0); assert.equal(probe.backendMemory.responseBytes, 0); }
      if (p.name === 'closed' || p.name === 'reset') {
        for (const value of [p.tabs.length, p.cachedBodyChars, p.activeBodyChars, p.bodyBytes, probe.backendMemory.responseBytes]) assert.equal(value, 0);
        assert.equal(probe.backendMemory.activeRequest, false);
      }
      if (p.name === 'reset') {
        for (const value of [p.labels, p.attempts, p.treeCaches, probe.backendMemory.snapshots]) assert.equal(value, 0);
        launchResets.push(median(phase.samples.map(s => s.pssBytes)));
      }
    }
    const slow = launch.traffic.filter(t => t.name === 'slow');
    assert.equal(slow.length, 3); assert(slow.every(t => t.bytesWritten > 0 && t.bytesWritten < 10 * MiB && t.closed && !t.finished), 'Cancellation was not during transfer');
    assert.equal(launch.traffic.filter(t => t.name === 'excess' && t.bytesWritten > 16 * MiB).length, 3, 'Real streamed excess not observed');
    assert.equal(launch.privacyPassed, true);
    assert(launch.traffic.length > 0 && launch.traffic.every(t => t.secretReceived), 'Runtime secret did not reach the server');
    assert(launch.peakObserved.pssBytes > 0 && launch.peakObserved.rssBytes > 0);
    for (const metric of ['pss', 'rss']) {
      validateFamily(launch.peakObserved[metric]);
      assert.equal(launch.peakObserved[`${metric}Bytes`], launch.peakObserved[metric][`${metric}Bytes`]);
      assert(launch.phases.flatMap(p => p.samples ?? []).every(s => s[`${metric}Bytes`] <= launch.peakObserved[`${metric}Bytes`]));
    }
    assert.equal(launchResets.length, 3);
    for (const name of ['chunk', 'scroll', 'wrap', 'tab-empty', 'tab-response', 'tab-empty-loading', 'tab-partial', 'cancel']) assert(actionNames.filter(action => action === name).length >= 3, `Missing repeated action: ${name}`);
    const growth = launchResets[2] - launchResets[1];
    deltas.push(growth); resets.push(...launchResets);
    if (growth > Math.max(memoryBudget.minimumBytes, launchResets[1] * memoryBudget.relativeGrowth)) failures.push(`pss-stability-launch-${i + 1}`);
    assert(interactions.length && cancellations.length);
  }
  const local = stats(interactions), cancel = stats(cancellations);
  const actionMs = Object.fromEntries(Object.entries(byAction).map(([name, values]) => [name, stats(values)]));
  for (const [name, value] of Object.entries(actionMs)) {
    const limit = actionBudgets[name];
    if ((limit.p95 !== undefined && value.p95 > limit.p95) || value.max > limit.max) failures.push(`${name}-budget`);
  }
  return { acceptanceProfile: responseAcceptancePolicy, memoryBudget, actionBudgetsMs: actionBudgets, aspirationalLocalP95Ms: 100,
    localActionMs: local, actionMs, cancelMs: cancel, resetPssBytes: stats(resets), cycleGrowthBytes: signedStats(deltas), budgetStatus: failures.length ? 'failed' : 'passed', failures };
}
