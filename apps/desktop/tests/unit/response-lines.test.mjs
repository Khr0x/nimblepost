import assert from 'node:assert/strict';
import { test } from 'node:test';
import { hasManyLines, responseLineStarts, responseLine, responseLineTops, responseLineWindow, responseLineAt, RESPONSE_LINE_HEIGHT, RESPONSE_OVERSCAN } from '../../src/lib/features/responses/response-lines.ts';

test('Response line offsets preserve blank lines, mixed newlines, Unicode and partial final lines', () => {
  const text = 'α\r\n\r\n雪\r🙂\nlast';
  const starts = responseLineStarts(text);
  assert.deepEqual([...starts].map((_, index) => responseLine(text, starts, index)), ['α', '', '雪', '🙂', 'last']);
  for (const value of ['', 'one\n', 'one\r', 'one\r\n', '\n\n']) {
    const offsets = responseLineStarts(value);
    assert.deepEqual([...offsets].map((_, index) => responseLine(value, offsets, index)), value.split(/\r\n|\r|\n/));
  }
  assert.equal(hasManyLines('x\n'.repeat(499)), false);
  assert.equal(hasManyLines('x\r\n'.repeat(500)), true);
});

test('A 2 MiB response mounts a bounded window at the start, middle and end', () => {
  const body = 'Native memory validation response\n'.repeat(65536).slice(0, 2 ** 21);
  const starts = responseLineStarts(body);
  assert.equal(starts.length, 61681);
  const tops = responseLineTops(starts.length, new Map());
  for (const position of [0, tops.at(-1) / 2, tops.at(-1) - 200, tops.at(-1) + 1000]) {
    const { start, end } = responseLineWindow(tops, position, 200);
    assert.ok(end - start <= Math.ceil(200 / RESPONSE_LINE_HEIGHT) + 2 * RESPONSE_OVERSCAN + 1);
    assert.ok(start >= 0 && end <= starts.length && end > start);
  }
  assert.equal(responseLineWindow(tops, 0, 200).start, 0);
  assert.equal(responseLineWindow(tops, tops.at(-1) - 200, 200).end, starts.length);
  assert.equal(responseLine(body, starts, starts.length - 1), body.slice(starts.at(-1)));
});

test('Measured wrapped lines participate in viewport lookup without losing later lines', () => {
  const tops = responseLineTops(1000, new Map([[0, 300], [5, 100]]));
  assert.equal(responseLineAt(tops, 299), 0);
  assert.equal(responseLineAt(tops, 300), 1);
  assert.equal(responseLineAt(tops, tops[500]), 500);
  const range = responseLineWindow(tops, tops[500], 100);
  assert.ok(range.start <= 500 && range.end > 500);
  assert.equal(responseLineAt(tops, -100), 0);
  assert.equal(responseLineAt(tops, Infinity), 999);
});
