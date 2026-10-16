import assert from 'node:assert/strict';
import { test } from 'node:test';
import { responseFixtures, fixtureIdentity, serveResponses } from './response-fixtures.mjs';
import { responseProtocol, responsePhases, summarizeResponses } from './response-report.mjs';
import { parsePss, responseWorkspace, copyWindow } from './native-responses.mjs';
import { hash } from './performance-report.mjs';
import { mkdtempSync, writeFileSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import Ajv from 'ajv';
import { parse } from 'yaml';
const MiB = 2 ** 20;
const payloads = responseFixtures(), identity = fixtureIdentity(payloads);
function report(id = 'reference') {
  const source = { sha: 'a'.repeat(40), dirty: false };
  const ready = { buildSha: source.sha, debugAssertions: false, inspector: false };
  const makePhase = (name, cycle, index) => {
    const content = identity[name === 'json' ? 'json' : name === 'line' ? 'line' : 'text'];
    const p = { index, cycle, name, actions: [], recoveryWarning: '', error: '', tabs: [], activeOffset: 0, bodyBytes: 0, activeBodyChars: 0, cachedBodyChars: 0, labels: 0, attempts: 0, treeCaches: 0 };
    if (['json','line','four-responses','after-cancel'].includes(name)) Object.assign(p, { content, activeOffset: content.bytes, bodyBytes: content.bytes });
    if (name === 'four-responses') Object.assign(p, { tabs: Array.from({length:64}, (_, i) => i), cachedBodyChars: 2 * MiB, loaded:Array(4).fill(identity.text) });
    if (name === 'excess') p.error = 'La respuesta supera el límite en memoria de 16777216 bytes';
    if (name === 'after-cancel') p.actions = [{ name: 'cancel', durationMs: 50 }];
    if (name === 'json') p.actions = [{ name: 'chunk', durationMs: 40 }];
    if (name === 'line') p.actions = ['scroll','wrap','tab-empty','tab-response','tab-empty-loading','tab-partial'].map(name => ({ name, durationMs: 40 }));
    if (name === 'copy') Object.assign(p, { kind: index % responsePhases.length === 3 ? 'json' : 'line', content: index % responsePhases.length === 3 ? identity.json : identity.line });
    return { probe: { ...ready, payload: p, backendMemory: { responseBytes: 0, responseCapacity: 0, snapshots: 0, activeRequest: false } },
      ...(name === 'copy' ? { clipboardSha256: p.content.sha256 } : { samples: Array.from({length:6}, (_, i) => ({ afterMarkerMs: 3000 + i * 210, rootPid: 1, pssBytes: 200 * MiB, rssBytes: 300 * MiB,
        processes: [{ pid: 1, ppid: 0, startTicks: 10, name: 'NimblePost', pssBytes: 100 * MiB, rssBytes: 150 * MiB }, { pid: 2, ppid: 1, startTicks: 11, name: 'WebKitWebProcess', pssBytes: 100 * MiB, rssBytes: 150 * MiB }] })) }) };
  };
  const peak = {rootPid:1,pssBytes:300*MiB,rssBytes:400*MiB,processes:[{pid:1,ppid:0,startTicks:10,name:'NimblePost',pssBytes:150*MiB,rssBytes:200*MiB},{pid:2,ppid:1,startTicks:11,name:'WebKitWebProcess',pssBytes:150*MiB,rssBytes:200*MiB}]};
  return { schemaVersion:1, protocol:responseProtocol, seriesId:id, status:'passed', mode:'instrumented-release-ephemeral',absoluteTargetsEvaluated:false,source, binary:{sha256:'b'.repeat(64)}, environment:{platform:'linux',arch:'x64',hostHash:'h',webview:'v'}, fixtures:identity, launchCount:3,
    launches:Array.from({length:3}, (_, i) => ({ iteration:i+1, ready, smoke:{payload:{error:'',checks:['large-responses']}}, privacyPassed:true,
      traffic:Array.from({length:3}, () => [{name:'slow',bytesWritten:65536,closed:true,finished:false,secretReceived:true},{name:'excess',bytesWritten:17*MiB,closed:true,finished:false,secretReceived:true}]).flat(),
      peakObserved:{pssBytes:300*MiB,rssBytes:400*MiB,pss:peak,rss:peak}, phases:[makePhase('baseline',0,0),...Array.from({length:3},(_, cycle)=>responsePhases.map((name,index)=>makePhase(name,cycle+1,1+cycle*responsePhases.length+index))).flat()] })) };
}

test('Large fixtures are exact UTF-8 payloads, valid JSON and an unbroken Unicode line', () => {
  assert.equal(payloads.json.length, 10 * MiB); assert.doesNotThrow(() => JSON.parse(payloads.json));
  for (const key of ['text','line']) assert.equal(payloads[key].length, 2 * MiB);
  for (const bytes of Object.values(payloads)) assert.equal(Buffer.from(bytes.toString('utf8')).equals(bytes), true);
  assert(!/[\r\n]/.test(payloads.line.toString())); assert(payloads.line.includes(Buffer.from('雪🙂')));
  assert.equal(payloads.line[65535],0xf0);
  const decoder=new TextDecoder();
  const partial=decoder.decode(payloads.line.subarray(0,65536),{stream:true});
  assert.equal(partial+decoder.decode(payloads.line.subarray(65536)),payloads.line.toString());
  assert.notEqual(partial+new TextDecoder().decode(payloads.line.subarray(65536)),payloads.line.toString());
  assert.equal(identity.json.sha256, hash(payloads.json));
  assert.equal(parsePss('Rss: 4096 kB\nPss:                2048 kB\n'), 2 * MiB);
  assert.throws(() => parsePss('Pss_Dirty: 100 kB'));
  assert.equal(copyWindow([{window:'main',width:1180,height:800},{window:'child',width:1,height:1}],{innerWidth:1180,innerHeight:765}).window,'main');
  assert.throws(()=>copyWindow([{window:'child',width:1,height:1}],{innerWidth:1180,innerHeight:765}));
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
    r => r.status='failed', r => r.status='diagnostic', r => r.protocol='responses-linux-v1',
    r => r.launchCount=5, r => r.launches.pop(), r => r.launches[0].phases.pop(),
    r => r.launches[0].phases[2].probe.payload.content={bytes:1,sha256:'wrong'},
    r => r.launches[0].phases[3].clipboardSha256='wrong',
    r => r.launches[0].phases[8].probe.backendMemory.responseBytes=1,
    r => r.launches[0].phases[2].probe.backendMemory.responseCapacity=32*MiB,
    r => delete r.launches[0].phases[2].probe.backendMemory.responseCapacity,
    r => r.launches[0].phases[2].probe.payload.actions[0].name='unknown',
    r => r.launches[0].phases[9].probe.backendMemory.snapshots=1,
    r => r.launches[0].phases[9].probe.payload.cachedBodyChars=1,
    r => r.launches[0].traffic[0].finished=true, r => r.launches[0].traffic[1].bytesWritten=16*MiB,
    r => r.launches[0].traffic[0].secretReceived=false, r => r.launches[0].privacyPassed=false,
    r => r.launches[0].peakObserved.pss.processes[1].ppid=3,
    r => r.launches[0].phases[0].samples[0].processes[1].ppid=3,
    r => r.launches[0].phases[0].samples[0].processes[1].name='Other',
    r => r.launches[0].phases[0].samples[0].pssBytes=1,
    r => r.launches[0].phases[0].samples[1].afterMarkerMs=3001,
    r => r.launches[0].ready.inspector=true,
  ]) { const broken=structuredClone(r); mutate(broken); assert.throws(()=>summarizeResponses(broken)); }
});

test('ALPHA per-action boundaries and the original memory budget remain enforced', () => {
  for (const [name,limit] of Object.entries({chunk:150,scroll:150,wrap:100,'tab-empty':250,'tab-response':250,'tab-empty-loading':250,'tab-partial':250,cancel:1000})) {
    const r=report();
    const actions=r.launches.flatMap(l=>l.phases.flatMap(p=>p.probe.payload.actions)).filter(a=>a.name===name);
    for(const a of actions)a.durationMs=limit;
    assert.equal(summarizeResponses(r).budgetStatus,'passed',name);
    for(const a of actions)a.durationMs=limit+.01;
    assert(summarizeResponses(r).failures.includes(`${name}-budget`),name);
  }
  const outlier=report();outlier.launches[0].phases[2].probe.payload.actions.push(...Array.from({length:100},()=>({name:'chunk',durationMs:40})),{name:'chunk',durationMs:251});
  const summary=summarizeResponses(outlier);assert.equal(summary.actionMs.chunk.p95,40);assert(summary.failures.includes('chunk-budget'));
  const leak=report('leak'); for(const sample of leak.launches[0].phases.at(-1).samples) { sample.pssBytes+=20*MiB; sample.processes[0].pssBytes+=20*MiB; }
  assert(summarizeResponses(leak).failures.includes('pss-stability-launch-1'));
  const slowTab=report('slow-tab'); slowTab.launches[0].phases[4].probe.payload.actions.find(a=>a.name==='tab-response').durationMs=251;
  assert.equal(summarizeResponses(slowTab).localActionMs.p95,40);
  assert(summarizeResponses(slowTab).failures.includes('tab-response-budget'));
});

test('Single-series evaluate recalculates raw budgets and rejects legacy or partial reports', () => {
  const directory=mkdtempSync(join(tmpdir(),'np013-evaluate-'));
  try {
    const path=join(directory,'report.json'),out=join(directory,'evaluation.json');
    const evaluate=r=>{writeFileSync(path,JSON.stringify(r));return spawnSync(process.execPath,[fileURLToPath(new URL('./native-responses.mjs',import.meta.url)),'evaluate','--report',path,'--out',out],{encoding:'utf8'});};
    assert.equal(evaluate(report()).status,0);
    assert.equal(JSON.parse(readFileSync(out)).status,'passed');
    const slow=report();slow.summary={budgetStatus:'passed'};slow.launches[0].phases[4].probe.payload.actions.find(a=>a.name==='tab-response').durationMs=251;
    assert.equal(evaluate(slow).status,2);assert.equal(JSON.parse(readFileSync(out)).status,'failed');
    const legacy=report();legacy.protocol='responses-linux-v1';assert.equal(evaluate(legacy).status,1);
    const partial=report();partial.launches.pop();assert.equal(evaluate(partial).status,1);
  } finally {rmSync(directory,{recursive:true,force:true});}
});

test('Native response collection and secret environment conform to the pinned schema', async () => {
  const directory=mkdtempSync(join(tmpdir(),'np013-fixture-test-'));
  try {
    await responseWorkspace(directory,'http://127.0.0.1:3000');
    const schema=JSON.parse(readFileSync(new URL('../schemas/opencollection/1.0.0/opencollection.schema.json',import.meta.url)));
    const ajv=new Ajv({strict:false});ajv.addSchema(schema);
    for(const [file,definition] of [['request-00000.yml','HttpRequest'],['environments/validation.yml','Environment'],['opencollection.yml',null]]) {
      const validate=ajv.getSchema(definition?`${schema.$id}#/$defs/${definition}`:schema.$id);
      assert(validate,definition);const document=parse(readFileSync(join(directory,'collection',file),'utf8'));
      assert(validate(document),JSON.stringify(validate.errors));
      assert(!JSON.stringify(document).includes('NP013_RUNTIME_SECRET_CANARY'));
    }
  } finally { rmSync(directory,{recursive:true,force:true}); }
});

test('The manual workflow propagates a runner failure through its actual logging pipeline', () => {
  const workflow=parse(readFileSync(new URL('../.github/workflows/native-performance.yml',import.meta.url),'utf8'));
  assert.deepEqual(Object.keys(workflow.on),['workflow_dispatch']);
  assert.equal(workflow.jobs.measure.defaults.run.shell,'bash');
  assert.equal(workflow.on.workflow_dispatch.inputs.scenario.default,'responses');
  assert.equal(workflow.on.workflow_dispatch.inputs.launches.default,'3');
  const step=workflow.jobs.measure.steps.find(s=>s.name==='Measure selected scenario');
  const selection=workflow.jobs.measure.steps.find(s=>s.name==='Validate measurement selection');
  const directory=mkdtempSync(join(tmpdir(),'np013-workflow-test-'));
  try {
    writeFileSync(join(directory,'dbus-run-session'),'#!/bin/sh\nexit "$NP013_EXIT_CODE"\n',{mode:0o755});
    for(const [scenario,launches,code] of [['responses','3',0],['responses','1',0],['startup','5',0],['responses','5',1],['startup','3',1]]) {
      assert.equal(spawnSync('bash',['-e','-c',selection.run],{env:{...process.env,SCENARIO:scenario,LAUNCHES:launches}}).status,code);
    }
    for(const code of [0,1,2]) {
      const result=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'responses',LAUNCHES:'1',NP013_EXIT_CODE:String(code)}});
      assert.equal(result.status,code);
    }
    const acceptanceFailure=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'responses',LAUNCHES:'3',NP013_EXIT_CODE:'2'}});
    assert.equal(acceptanceFailure.status,2);
    const diagnosticFailure=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'response-diagnostics',LAUNCHES:'5',NP013_EXIT_CODE:'1'}});
    assert.equal(diagnosticFailure.status,1);
    const profileFailure=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'response-profile',LAUNCHES:'5',NP013_EXIT_CODE:'1'}});
    assert.equal(profileFailure.status,1);
    const argsFile=join(directory,'ui-args');
    writeFileSync(join(directory,'dbus-run-session'),'#!/bin/sh\nprintf "%s\\n" "$@" >> "$NP013_ARGS_FILE"\nexit 0\n',{mode:0o755});
    const ui=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'response-ui-profile',LAUNCHES:'5',NP013_ARGS_FILE:argsFile}});
    assert.equal(ui.status,0,ui.stderr);
    const actualArgs=readFileSync(argsFile,'utf8');
    assert(actualArgs.includes('--launches\n1\n')); assert(actualArgs.includes('--ui-profile\ntrue\n'));
    assert.equal(actualArgs.split('--out\n').length-1,1,'UI profiling runs one diagnostic series');
    rmSync(argsFile);
    const acceptance=spawnSync('bash',['--noprofile','--norc','-e','-o','pipefail','-c',step.run],{cwd:directory,encoding:'utf8',env:{...process.env,PATH:`${directory}:${process.env.PATH}`,SCENARIO:'responses',LAUNCHES:'3',NP013_ARGS_FILE:argsFile}});
    assert.equal(acceptance.status,0,acceptance.stderr);
    const measured=readFileSync(argsFile,'utf8');assert(measured.includes('--launches\n3\n'));assert.equal(measured.split('--out\n').length-1,1);
    assert(measured.includes('acceptance.json'));assert(!measured.includes('--allocator-library'));
    const evaluation=workflow.jobs.measure.steps.find(s=>s.name==='Evaluate ALPHA response budgets from the single series');
    assert(evaluation.if.includes("env.LAUNCHES == '3'"));
    writeFileSync(join(directory,'node'),'#!/bin/sh\nexit 2\n',{mode:0o755});
    assert.equal(spawnSync('bash',['-e','-o','pipefail','-c',evaluation.run],{cwd:directory,env:{...process.env,PATH:`${directory}:${process.env.PATH}`}}).status,2);
  } finally { rmSync(directory,{recursive:true,force:true}); }
});
