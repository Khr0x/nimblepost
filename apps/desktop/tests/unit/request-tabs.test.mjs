import assert from 'node:assert/strict';
import { test } from 'node:test';
import { untitledDraft, refreshTabDraft } from '../../src/lib/features/requests/request-tabs.ts';
import { draftFrom, editDraft } from '../../src/lib/features/requests/draft.ts';

test('Untitled starts in memory and independent tabs retain their own edits', () => {
  const first = untitledDraft();
  const second = untitledDraft();
  assert.equal(first.revision, 0);
  assert.equal(first.value.info.name, 'Untitled');
  assert.deepEqual(first.edits, []);
  editDraft(first, ['http', 'url'], 'https://example.com/one');
  editDraft(second, ['http', 'headers'], [{ name: 'Accept', value: 'application/json' }]);
  assert.equal(second.value.http.url, '');
  assert.equal(first.value.http.headers, undefined);
});

test('refreshing an expired tab revision preserves edits and refuses external changes', () => {
  const source = { name: 'List', method: 'GET', url: 'https://example.com', revision: 1, diagnostics: [], document: { info: { name: 'List' }, http: { method: 'GET', url: 'https://example.com' } } };
  const draft = draftFrom(source);
  editDraft(draft, ['http', 'params'], [{ name: 'limit', value: '25', type: 'query' }]);
  const tab = { source, draft };
  const refreshed = refreshTabDraft(tab, { ...source, revision: 15 });
  assert.equal(refreshed.revision, 15);
  assert.deepEqual(refreshed.edits, draft.edits);
  assert.equal(refreshed.value.http.params[0].name, 'limit');
  assert.throws(() => refreshTabDraft(tab, { ...source, document: { ...source.document, external: true } }), /changed on disk/);
  assert.equal(draft.revision, 1);
});
