import assert from 'node:assert/strict';
import { test } from 'node:test';
import { frontendStages, recordStage, stopProfiling } from '../../src/lib/desktop/native-profile.ts';

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
