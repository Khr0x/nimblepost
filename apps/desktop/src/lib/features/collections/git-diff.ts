export const diffKind = { metadata: 0, context: 1, addition: 2, deletion: 3 } as const;

const hunkHeader = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@(?: |$)/;

export function indexGitDiff(patch: string) {
  const lines = patch.split('\n');
  const visible: string[] = [];
  const before = new Float64Array(lines.length);
  const after = new Float64Array(lines.length);
  const kinds = new Uint8Array(lines.length);
  let oldLine = 0, newLine = 0;
  let oldRemaining = 0, newRemaining = 0;
  let maxLine = 0;

  for (const line of lines) {
    const index = visible.length;
    const hunk = line.startsWith('@@ -') ? hunkHeader.exec(line) : null;
    if (hunk) {
      oldLine = Number(hunk[1]);
      oldRemaining = Number(hunk[2] ?? 1);
      newLine = Number(hunk[3]);
      newRemaining = Number(hunk[4] ?? 1);
      continue;
    }
    if (oldRemaining > 0 && newRemaining > 0 && (line === '' || line.startsWith(' '))) {
      before[index] = oldLine++;
      after[index] = newLine++;
      kinds[index] = diffKind.context;
      oldRemaining--;
      newRemaining--;
    } else if (newRemaining > 0 && line.startsWith('+')) {
      after[index] = newLine++;
      kinds[index] = diffKind.addition;
      newRemaining--;
    } else if (oldRemaining > 0 && line.startsWith('-')) {
      before[index] = oldLine++;
      kinds[index] = diffKind.deletion;
      oldRemaining--;
    } else if (!line.startsWith('\\ ')) {
      oldRemaining = newRemaining = 0;
      if (!line.startsWith('old mode ') && !line.startsWith('new mode ')) continue;
    }
    visible.push(line);
    maxLine = Math.max(maxLine, before[index], after[index]);
  }
  const length = visible.length;
  return {
    lines: visible,
    before: before.subarray(0, length),
    after: after.subarray(0, length),
    kinds: kinds.subarray(0, length),
    digits: String(maxLine).length,
  };
}
