// Called only with VITE_NATIVE_VALIDATION=1; ordinary builds remove these probes.
let profiling = true;
let responseProfiling = false;
type ResponseStage = { name: string; count: number; totalMs: number; maxMs: number; items: number; samples: number[] };
const responseStages = new Map<string, ResponseStage>();
let dropped = 0;
export function profileResponses() { responseProfiling = true; }
export function takeResponseStages() {
  const result = { stages: [...responseStages.values()], dropped };
  responseStages.clear(); dropped = 0;
  return result;
}
export function recordStage(name: string, start: number, items = 0) {
  if (responseProfiling) {
    let stage = responseStages.get(name);
    if (!stage) {
      if (responseStages.size >= 32) { dropped++; return; }
      stage = { name, count: 0, totalMs: 0, maxMs: 0, items: 0, samples: [] };
      responseStages.set(name, stage);
    }
    const duration = performance.now() - start;
    stage.count++; stage.totalMs += duration; stage.maxMs = Math.max(stage.maxMs, duration); stage.items += items;
    if (stage.samples.length < 16) stage.samples.push(duration);
    return;
  }
  if (!profiling) return;
  performance.measure(`nimblepost:${name}`, { start, end: performance.now(), detail: { items } });
}

export function stopProfiling() {
  profiling = false;
  for (const entry of performance.getEntriesByType('measure')) if (entry.name.startsWith('nimblepost:')) performance.clearMeasures(entry.name);
}

export function frontendStages() {
  return performance.getEntriesByType('measure')
    .filter(entry => entry.name.startsWith('nimblepost:'))
    .map(entry => ({ name: entry.name.slice('nimblepost:'.length), startMs: entry.startTime,
      durationMs: entry.duration, items: (entry as PerformanceMeasure).detail.items as number }));
}
