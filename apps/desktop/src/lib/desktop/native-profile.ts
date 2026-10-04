// Called only with VITE_NATIVE_VALIDATION=1; ordinary builds remove these probes.
let profiling = true;
export function recordStage(name: string, start: number, items = 0) {
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
