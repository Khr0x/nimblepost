import assert from 'node:assert/strict';
import { test } from 'node:test';
import { formatJSON, tokenizeJSON, foldJSON } from '../../src/lib/features/responses/response-body.ts';

test('Pretty JSON preserves values, duplicate keys and exact numeric/string tokens', () => {
  const source = '{"id":9007199254740993123,"precise":0.123456789012345678901,"escaped":"\\u0061\\n\\\"\\\\","items":[1,{"empty":{},"array":[]}],"duplicate":1,"duplicate":2}';
  const { pretty, issue } = formatJSON(source);
  assert.equal(issue, null);
  assert.match(pretty, /\n  "id": 9007199254740993123,/);
  assert.match(pretty, /0\.123456789012345678901/);
  assert.ok(pretty.includes('"escaped": "\\u0061\\n\\\"\\\\"'));
  assert.equal(pretty.match(/"duplicate"/g).length, 2);
  assert.deepEqual(JSON.parse(pretty), JSON.parse(source));
  assert.ok(pretty.includes('"empty": {}'));
  assert.ok(pretty.includes('"array": []'));
  assert.equal(source.includes('\n'), false);
});

test('JSON highlighting classifies types while preserving escaped strings and exact text', () => {
  const { pretty } = formatJSON('{"true":false,"null":null,"integer":9007199254740993123,"decimal":-1.2345e+67,"quoted\\\"key":"value: \\\"hello\\\"","unsafe":"<script>alert(1)</script>","list":[true,{},[]]}');
  const tokens = tokenizeJSON(pretty);
  assert.equal(tokens.map(token => token.text).join(''), pretty);
  assert.ok(tokens.some(token => token.kind === 'key' && token.text === '"true"'));
  assert.ok(tokens.some(token => token.kind === 'key' && token.text === '"null"'));
  assert.ok(tokens.some(token => token.kind === 'string' && token.text.includes('value:')));
  assert.ok(tokens.some(token => token.kind === 'string' && token.text.includes('<script>')));
  assert.ok(tokens.some(token => token.kind === 'number' && token.text === '9007199254740993123'));
  assert.ok(tokens.some(token => token.kind === 'number' && token.text === '-1.2345e+67'));
  assert.ok(tokens.some(token => token.kind === 'boolean' && token.text === 'false'));
  assert.ok(tokens.some(token => token.kind === 'boolean' && token.text === 'true'));
  assert.ok(tokens.some(token => token.kind === 'null' && token.text === 'null'));
  assert.equal(tokenizeJSON('[' + '0,'.repeat(12000) + '0]'), null);
});

test('Pretty handles JSON scalars, BOM and whitespace, and leaves invalid or oversized bodies available as raw text', () => {
  for (const source of ['null', 'false', '0', '"hello"', '[]', '{}']) assert.equal(formatJSON(source).pretty, source);
  assert.equal(formatJSON('\uFEFF  { "ok" : true } ').pretty, '{\n  "ok": true\n}');
  for (const source of ['', '<html>text</html>', '{"partial":', '{"bad":NaN}']) assert.deepEqual(formatJSON(source), { pretty: null, issue: 'invalid' });
  assert.deepEqual(formatJSON('"' + 'a'.repeat(2 * 1024 * 1024) + '"'), { pretty: null, issue: 'large' });
  const deep = '['.repeat(2000) + '0' + ']'.repeat(2000);
  assert.deepEqual(formatJSON(deep), { pretty: null, issue: 'large' });
});

test('JSON folding pairs nested objects and arrays without changing values, closing commas or empty containers', () => {
  const source = '{"user":{"id":9007199254740993123,"details":{"name":"María"}},"roles":["admin",{"literal":"brackets } [ and \\\"quotes\\\""}],"emptyObject":{},"emptyArray":[],"user":{"active":true}}';
  const { pretty } = formatJSON(source);
  const lines = foldJSON(tokenizeJSON(pretty));
  const expanded = nodes => nodes.flatMap(node => [node.tokens.map(token => token.text).join(''), ...(node.children ? [...expanded(node.children), node.closing.map(token => token.text).join('')] : [])]);
  assert.equal(expanded(lines).join('\n'), pretty);
  const root = lines[0];
  assert.equal(root.children.length, 5);
  assert.ok(root.children[0].children[1].children);
  assert.ok(root.children[1].children[1].children);
  assert.equal(root.children[0].closing.map(token => token.text).join(''), '  },');
  assert.equal(root.children[2].children, undefined);
  assert.equal(root.children[3].children, undefined);
  assert.notEqual(root.children[0].id, root.children[4].id);
  for (const value of ['null', 'true', '123', '"hello"', '{}', '[]']) {
    const nodes = foldJSON(tokenizeJSON(formatJSON(value).pretty));
    assert.equal(nodes[0].children, undefined);
    assert.equal(expanded(nodes).join('\n'), value);
  }
  const deep = '['.repeat(129) + '0' + ']'.repeat(129);
  assert.equal(foldJSON(tokenizeJSON(formatJSON(deep).pretty)), null);
});
