import assert from 'node:assert/strict';
import { test } from 'node:test';
import { defaultsProvenance, editableDefaults } from '../../src/lib/features/collections/defaults.ts';

test('structurally invalid defaults stay retained and cannot receive structured field edits', () => {
  assert.equal(editableDefaults({}), true);
  assert.equal(editableDefaults({request:{headers:[],foreign:'keep'}}), true);
  for (const request of [null,[], 'invalid',15,true]) assert.equal(editableDefaults({request}), false);
});

test('defaults provenance distinguishes case-insensitive header suppression from disabled variables', () => {
  const ancestors = [{ path: 'opencollection.yml', headers: [{ name: 'X-Key', disabled: false }, { name: 'X-Repeat', disabled: false }, { name: 'x-repeat', disabled: true }], variables: [{ name: 'token', disabled: false }, { name: 'Token', disabled: false }], auth: 'bearer' },
    { path: 'parent/folder.yml', headers: [], variables: [{ name: 'token', disabled: false }], auth: 'inherit' }];
  const value = { request: { headers: [{ name: 'x-key', disabled: true, value: 'SECRET' }], variables: [{ name: 'token', disabled: true }, { name: 'Token', value: 'PRIVATE' }], auth: 'inherit' } };
  const rows = defaultsProvenance(ancestors, 'parent/child/folder.yml', value);
  assert.equal(rows.find(row => row.name === 'X-Key').state, 'overridden');
  assert.equal(rows.find(row => row.name === 'x-key').state, 'suppressed');
  assert.equal(rows.find(row => row.name === 'X-Repeat').state, 'inherited');
  assert.equal(rows.find(row => row.name === 'x-repeat').state, 'disabled');
  assert.equal(rows.find(row => row.name === 'token' && row.path === 'parent/folder.yml').state, 'inherited');
  assert.equal(rows.find(row => row.name === 'Token' && row.path === 'opencollection.yml').state, 'overridden');
  assert.equal(rows.find(row => row.section === 'Auth').state, 'inherited');
  assert.ok(!JSON.stringify(rows).includes('SECRET') && !JSON.stringify(rows).includes('PRIVATE'));
  assert.equal(ancestors[0].headers[0].name, 'X-Key');
});

test('defaults provenance keeps duplicate rows and reports explicit auth overrides without values', () => {
  const source = { path: 'opencollection.yml', headers: [], variables: [], auth: 'basic' };
  const rows = defaultsProvenance([source], 'folder/folder.yml', { request: { headers: [{name:'X',value:'1'}, {name:'X',value:'2'}], auth: {type:'apikey',value:'private'} } });
  assert.equal(rows.filter(row => row.name === 'X' && row.state === 'local').length, 2);
  assert.equal(rows.find(row => row.name === 'basic').state, 'overridden');
  assert.equal(rows.find(row => row.name === 'apikey').state, 'local');
  assert.deepEqual(defaultsProvenance([], 'opencollection.yml', {}), []);
  assert.doesNotThrow(() => defaultsProvenance([], 'folder.yml', {request:{headers:'retained invalid field',variables:[null]}}));
});
