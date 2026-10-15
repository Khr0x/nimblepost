import assert from 'node:assert/strict';
import { test } from 'node:test';
import { frontendStages, recordStage, stopProfiling, profileResponses, takeResponseStages } from '../../src/lib/desktop/native-profile.ts';

test('frontend profiling exports durations and counts without unrelated performance entries', () => {
  try {
    performance.measure('unrelated', { start: 0, end: 1 });
    const started = performance.now() - 5;
    recordStage('tree-build', started, 5000);
    const stages = frontendStages();
    assert.equal(stages.length, 1);
    assert.equal(stages[0].name, 'tree-build');
    assert.equal(stages[0].startMs, started);
    assert(stages[0].durationMs >= 5);
    assert.equal(stages[0].items, 5000);
    assert(!JSON.stringify(stages).includes('unrelated'));
    stopProfiling();
    recordStage('summaries-load', performance.now(), 5000);
    assert.deepEqual(frontendStages(), [], 'Sustained memory runs do not retain diagnostic timings');
    assert.equal(performance.getEntriesByName('unrelated').length, 1);
  } finally {
    performance.clearMeasures('nimblepost:tree-build');
    performance.clearMeasures('unrelated');
  }
});

test('Response timings remain bounded and drain between phases', () => {
  stopProfiling(); profileResponses();
  for (let i=0;i<100;i++) recordStage('layout',performance.now(),64);
  for (let i=0;i<100;i++) recordStage(`other-${i}`,performance.now());
  const batch=takeResponseStages();
  assert.equal(batch.stages.length,32); assert.equal(batch.dropped,69);
  const layout=batch.stages.find(s=>s.name==='layout');
  assert.equal(layout.count,100); assert.equal(layout.items,6400); assert.equal(layout.samples.length,16);
  assert(layout.totalMs>=layout.maxMs);
  assert.deepEqual(takeResponseStages(),{stages:[],dropped:0});
});
