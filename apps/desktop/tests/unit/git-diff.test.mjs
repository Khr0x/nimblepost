import assert from 'node:assert/strict';
import { test } from 'node:test';
import { diffKind, indexGitDiff } from '../../src/lib/features/collections/git-diff.ts';

function indexed(lines) {
  const { lines: visible, before, after, kinds, digits } = indexGitDiff(lines.join('\n'));
  return { lines: visible, rows: Array.from(kinds, (kind, index) => [before[index], after[index], kind]), digits };
}

const { metadata, context, addition, deletion } = diffKind;

test('diff numbers follow each side independently and restart at the next hunk', () => {
  const patch = [
    'diff --git a/request.yml b/request.yml',
    'index 7365930..909b229 100644',
    '--- a/request.yml',
    '+++ b/request.yml',
    '@@ -8,3 +8,4 @@ request:',
    '   method: GET',
    '-  url: /users',
    '+  url: /users?active=true',
    '+  timeout: 2000',
    '   name: Users',
    '@@ -99 +100 @@',
    '-  auth: none',
    '+  auth: inherit',
  ];
  assert.deepEqual(indexed(patch), {
    lines: [
      '   method: GET', '-  url: /users', '+  url: /users?active=true', '+  timeout: 2000',
      '   name: Users', '-  auth: none', '+  auth: inherit',
    ],
    rows: [
      [8, 8, context], [9, 0, deletion], [0, 9, addition], [0, 10, addition], [10, 11, context],
      [99, 0, deletion], [0, 100, addition],
    ],
    digits: 3,
  });
});

test('new and deleted files number only the side with content', () => {
  const diff = indexed([
    '@@ -0,0 +1,2 @@',
    '+++inside a new file',
    '+',
    '@@ -1,2 +0,0 @@',
    '---inside a deleted file',
    '-',
    '',
  ]);
  assert.deepEqual(diff.lines, ['+++inside a new file', '+', '---inside a deleted file', '-']);
  assert.deepEqual(diff.rows, [
    [0, 1, addition], [0, 2, addition], [1, 0, deletion], [2, 0, deletion],
  ]);
});

test('missing-newline notes and mode metadata stay unnumbered', () => {
  const note = '\\ No newline at end of file';
  const diff = indexed([
    '@@ -3 +4 @@',
    '-old\r', note, '+new\r', note,
    'old mode 100644', 'new mode 100755',
  ]);
  assert.deepEqual(diff.lines, ['-old\r', note, '+new\r', note, 'old mode 100644', 'new mode 100755']);
  assert.deepEqual(diff.rows, [
    [3, 0, deletion], [0, 0, metadata], [0, 4, addition], [0, 0, metadata], [0, 0, metadata], [0, 0, metadata],
  ]);
});

test('blank context advances both numbers even when Git suppresses the space prefix', () => {
  assert.deepEqual(indexed(['@@ -2,2 +3,2 @@', '', '-before', '+after', '']).rows, [
    [2, 3, context], [3, 0, deletion], [0, 4, addition],
  ]);
});

test('a patch for a new empty file has no visible text', () => {
  assert.deepEqual(indexed([
    'diff --git a/empty.yml b/empty.yml',
    'new file mode 100644',
    'index 0000000..e69de29',
    '',
  ]).lines, []);
});
