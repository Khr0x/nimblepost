import assert from 'node:assert/strict';
import { test } from 'node:test';
import { completeUrlVariable, isVariablePlaceholder, urlHighlightParts, urlVariableMatch } from '../../src/lib/features/requests/url-completion.ts';

test('only complete variable references can be displayed in masked auth fields', () => {
  for (const value of ['{{token}}', '{{ api.token }}']) assert.equal(isVariablePlaceholder(value), true);
  for (const value of ['secret', '{{token', 'secret{{token}}', '{{token}}secret', '{{token}}{{other}}']) {
    assert.equal(isVariablePlaceholder(value), false);
  }
});

test('URL highlighting tracks partial selections across variables and plain text', () => {
  const value = '{{host}}/users/{{id}}';
  const parts = urlHighlightParts(value, 4, 17);
  assert.equal(parts.map(part => part.text).join(''), value);
  assert.equal(parts.filter(part => part.selected).map(part => part.text).join(''), value.slice(4, 17));
  assert.deepEqual(parts.filter(part => part.variable && !part.selected).map(part => part.text), ['{{ho', 'id}}']);
  assert.ok(parts.some(part => part.selected && !part.variable && part.text === '/users/'));
  for (const [start, end] of [[0, 0], [7, 7], [value.length, value.length]]) {
    assert.ok(urlHighlightParts(value, start, end).every(part => !part.selected));
  }
  assert.ok(urlHighlightParts(value, 0, value.length).every(part => part.selected));
  assert.deepEqual(urlHighlightParts('', 0, 0), []);
  const collapsed = urlHighlightParts(value, 8, 8);
  assert.ok(collapsed.every(part => !part.selected));
  assert.deepEqual(collapsed.filter(part => part.variable).map(part => part.text), ['{{host}}', '{{id}}']);
  assert.deepEqual(parts.filter(part => part.variable).map(part => part.name), ['host', 'host', 'id', 'id']);
});

test('URL completion recognizes an unfinished variable at the cursor', () => {
  assert.deepEqual(urlVariableMatch('{{', 2), { from: 2, prefix: '' });
  assert.deepEqual(urlVariableMatch('https://{{ api.host', 19), { from: 11, prefix: 'api.host' });
  assert.equal(urlVariableMatch('{{host}}/users', 13), null);
  assert.equal(urlVariableMatch('https://example.com', 19), null);
  assert.equal(urlVariableMatch('{host', 5), null);
});

test('URL completion inserts closing braces and preserves surrounding URL text', () => {
  for (const suffix of ['', '}', '}}', 'seUrl}}', 'seUrl }}']) {
    const value = `{{ba${suffix}/users?token={{token}}`;
    assert.deepEqual(completeUrlVariable(value, 4, 'baseUrl'), {
      value: '{{baseUrl}}/users?token={{token}}', cursor: 11,
    });
  }
  assert.deepEqual(completeUrlVariable('https://{{ api.h}}/users', 15, 'api.host'), {
    value: 'https://{{ api.host}}/users', cursor: 21,
  });
  assert.equal(completeUrlVariable('https://example.com', 19, 'host'), null);
});
