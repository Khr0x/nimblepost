import assert from 'node:assert/strict';
import { test } from 'node:test';
import { responseFixtures, fixtureIdentity, serveResponses } from './response-fixtures.mjs';
import { responseProtocol, responsePhases, summarizeResponses, compareResponses } from './response-report.mjs';
import { parsePss } from './native-responses.mjs';
import { hash } from './performance-report.mjs';
const MiB = 2 ** 20;
const payloads = responseFixtures(), identity = fixtureIdentity(payloads);
function report(id = 'reference') {
  const source = { sha: 'a'.repeat(40), dirty: false };
  const ready = { buildSha: source.sha, debugAssertions: false, inspector: false };
  const makePhase = (name, cycle, index) => {
    const content = identity[name === 'json' ? 'json' : name === 'line' ? 'line' : 'text'];
    const p = { index, cycle, name, actions: [], recoveryWarning: '', error: '', tabs: [], activeOffset: 0, bodyBytes: 0, activeBodyChars: 0, cachedBodyChars: 0, labels: 0, attempts: 0, treeCaches: 0 };
    if (['json','line','four-responses','after-cancel'].includes(name)) Object.assign(p, { content, activeOffset: content.bytes, bodyBytes: content.bytes });
    if (name === 'four-responses') Object.assign(p, { tabs: Array.from({length:64}, (_, i) => i), cachedBodyChars: 2 * MiB });
    if (name === 'excess') p.error = 'La respuesta supera el límite en memoria de 16777216 bytes';
    if (name === 'after-cancel') p.actions = [{ name: 'cancel', durationMs: 50 }];
    if (name === 'json') p.actions = [{ name: 'chunk', durationMs: 40 }];
    if (name === 'line') p.actions = ['scroll','wrap','tab-empty','tab-response'].map(name => ({ name, durationMs: 40 }));
    if (name === 'copy') Object.assign(p, { kind: index % responsePhases.length === 3 ? 'json' : 'line', content: index % responsePhases.length === 3 ? identity.json : identity.line });
    return { probe: { ...ready, payload: p, backendMemory: { responseBytes: 0, snapshots: 0, activeRequest: false } },
      ...(name === 'copy' ? { clipboardSha256: p.content.sha256 } : { samples: Array.from({length:6}, (_, i) => ({ afterMarkerMs: 3000 + i * 210, rootPid: 1, pssBytes: 200 * MiB, rssBytes: 300 * MiB,
        processes: [{ pid: 1, ppid: 0, startTicks: 10, name: 'NimblePost', pssBytes: 100 * MiB, rssBytes: 150 * MiB }, { pid: 2, ppid: 1, startTicks: 11, name: 'WebKitWebProcess', pssBytes: 100 * MiB, rssBytes: 150 * MiB }] })) }) };
  };
  return { schemaVersion:1, protocol:responseProtocol, seriesId:id, status:'passed', source, binary:{sha256:'b'.repeat(64)}, environment:{platform:'linux',hostHash:'h',webview:'v'}, fixtures:identity, launchCount:5,
    launches:Array.from({length:5}, (_, i) => ({ iteration:i+1, ready, smoke:{payload:{error:'',checks:['large-responses']}}, privacyPassed:true,
      traffic:Array.from({length:3}, () => [{name:'slow',bytesWritten:65536,closed:true,finished:false,secretReceived:true},{name:'excess',bytesWritten:17*MiB,closed:true,finished:false,secretReceived:true}]).flat(),
      peakObserved:{pssBytes:300*MiB,rssBytes:400*MiB}, phases:[makePhase('baseline',0,0),...Array.from({length:3},(_, cycle)=>responsePhases.map((name,index)=>makePhase(name,cycle+1,1+cycle*responsePhases.length+index))).flat()] })) };
}

test('Large fixtures are exact UTF-8 payloads, valid JSON and an unbroken Unicode line', () => {
  assert.equal(payloads.json.length, 10 * MiB); assert.doesNotThrow(() => JSON.parse(payloads.json));
  for (const key of ['text','line']) assert.equal(payloads[key].length, 2 * MiB);
  for (const bytes of Object.values(payloads)) assert.equal(Buffer.from(bytes.toString('utf8')).equals(bytes), true);
  assert(!/[\r\n]/.test(payloads.line.toString())); assert(payloads.line.includes(Buffer.from('雪🙂')));
  assert.equal(identity.json.sha256, hash(payloads.json));
  assert.equal(parsePss('Rss: 4096 kB\nPss:                2048 kB\n'), 2 * MiB);
  assert.throws(() => parsePss('Pss_Dirty: 100 kB'));
});

test('HTTP fixtures serve real chunked excess and a transfer can be aborted', async () => {
  const server = await serveResponses();
  try {
    const response = await fetch(`${server.url}/json`, { headers:{'x-probe-secret':'NP013_RUNTIME_SECRET_CANARY'} });
    assert.equal(hash(Buffer.from(await response.arrayBuffer())), identity.json.sha256);
    const excess = await fetch(`${server.url}/excess`);
    assert.equal(excess.headers.get('content-length'), null); assert.equal(excess.headers.get('transfer-encoding'), 'chunked');
    assert.equal((await excess.arrayBuffer()).byteLength, 17 * MiB);
    const abort = new AbortController(); const slow = await fetch(`${server.url}/slow`,{signal:abort.signal});
    const reader = slow.body.getReader(); assert((await reader.read()).value.byteLength > 0); abort.abort();
  } finally { await server.close(); }
});

test('Complete reports recompute summaries; clipboard, release and HTTP checks are mandatory', () => {
  const r = report(); assert.equal(summarizeResponses(r).budgetStatus,'passed');
  for (const mutate of [
    r => r.status='failed', r => r.launches.pop(), r => r.launches[0].phases.pop(),
    r => r.launches[0].phases[2].probe.payload.content={bytes:1,sha256:'wrong'},
    r => r.launches[0].phases[3].clipboardSha256='wrong',
    r => r.launches[0].phases[8].probe.backendMemory.responseBytes=1,
    r => r.launches[0].phases[9].probe.backendMemory.snapshots=1,
    r => r.launches[0].phases[9].probe.payload.cachedBodyChars=1,
    r => r.launches[0].traffic[0].finished=true, r => r.launches[0].traffic[1].bytesWritten=16*MiB,
    r => r.launches[0].traffic[0].secretReceived=false, r => r.launches[0].privacyPassed=false,
    r => r.launches[0].phases[0].samples[0].processes[1].ppid=3,
    r => r.launches[0].phases[0].samples[0].processes[1].name='Other',
    r => r.launches[0].phases[0].samples[0].pssBytes=1,
    r => r.launches[0].phases[0].samples[1].afterMarkerMs=3001,
    r => r.launches[0].ready.inspector=true,
  ]) { const broken=structuredClone(r); mutate(broken); assert.throws(()=>summarizeResponses(broken)); }
});

test('Operational interaction and memory budgets cannot be hidden by a comparison', () => {
  const reference=report(), candidate=report('candidate');
  assert.equal(compareResponses(reference,candidate).status,'passed');
  candidate.launches[0].phases[2].probe.payload.actions[0].durationMs=251;
  assert.equal(summarizeResponses(candidate).budgetStatus,'failed'); assert.equal(compareResponses(reference,candidate).status,'failed');
  assert.throws(()=>compareResponses(candidate,reference));
  assert.throws(()=>compareResponses(reference,reference));
  const incompatible=report('other');incompatible.environment.webview='other'; assert.throws(()=>compareResponses(reference,incompatible));
  const leak=report('leak'); for(const sample of leak.launches[0].phases.at(-1).samples) { sample.pssBytes+=20*MiB; sample.processes[0].pssBytes+=20*MiB; }
  assert.throws(()=>summarizeResponses(leak),/stabilize/);
});
