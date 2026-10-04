import assert from 'node:assert/strict';
import { test } from 'node:test';
import { captureRecovery, restoreRecovery, recoverAsUntitled } from '../../src/lib/features/requests/recovery.ts';
import { untitledDraft, refreshTabDraft } from '../../src/lib/features/requests/request-tabs.ts';
import { draftFrom, editDraft } from '../../src/lib/features/requests/draft.ts';

const owner = { root: '/tmp/api', requests: ['saved.yml'], environments: ['local'] };
const source = { name: 'Saved', method: 'GET', url: 'https://example.com', revision: 42, diagnostics: [], document: { info: { name: 'Saved', type: 'http' }, http: { method: 'GET', url: 'https://example.com', headers: [{ name: 'X-Demo', value: '{{token}}' }] }, foreign: { keep: true } } };
function tab(id, path = '') {
  return { id, root: owner.root, path, source: path ? source : null, draft: path ? draftFrom(source) : untitledDraft(), environment: 'local', baseUrl: 'transient-override', response: { headers: ['sensitive-response'] }, body: 'sensitive-response', offset: 50, secrets: { token: 'execution-secret' } };
}

test('recovery keeps tab order, active draft, full configuration and environment without runtime data', () => {
  const saved = tab(1, 'saved.yml'), unsaved = tab(2);
  editDraft(saved.draft, ['http', 'url'], 'https://example.com/pending');
  editDraft(unsaved.draft, ['http', 'body'], { type: 'json', data: '{"hello":true}' });
  const activeDraft = structuredClone(unsaved.draft);
  editDraft(activeDraft, ['info', 'name'], 'Active draft');
  const snapshot = captureRecovery(1, [saved, unsaved], 2, activeDraft, 'local', false);
  assert.deepEqual(snapshot.tabs.map(tab => tab.id), [1, 2]);
  assert.equal(snapshot.activeTab, 2);
  assert.equal(snapshot.tabs[1].value.info.name, 'Active draft');
  assert.equal(snapshot.tabs[0].value.foreign.keep, true);
  const bytes = JSON.stringify(snapshot);
  for (const text of ['execution-secret', 'sensitive-response', 'transient-override', 'revision', 'diagnostics']) assert.equal(bytes.includes(text), false);
  activeDraft.value.info.name = 'Later';
  assert.equal(snapshot.tabs[1].value.info.name, 'Active draft', 'Queued backup is a detached copy');
  const restored = restoreRecovery(snapshot, [owner]);
  assert.deepEqual(restored.map(tab => tab.draft.revision), [0, 0]);
  assert.deepEqual(restored[1].draft.edits, snapshot.tabs[1].edits);
  assert.equal(restored[0].environment, 'local');
  const fresh = refreshTabDraft(restored[0], { ...source, revision: 90 });
  assert.equal(fresh.revision, 90);
  assert.equal(fresh.value.http.url, 'https://example.com/pending');
});

test('missing files and conflicts become unsaved copies without losing fields or overwriting the source', () => {
  const saved = tab(1, 'saved.yml');
  const snapshot = captureRecovery(1, [saved], 1, saved.draft, 'local', false);
  const [missing] = restoreRecovery(snapshot, []);
  assert.equal(missing.path, '');
  assert.equal(missing.source, null);
  assert.equal(missing.root, null);
  assert.equal(missing.draft.value.foreign.keep, true);
  assert.ok(missing.draft.edits.length, 'Recovered copy must be protected when closing its tab');
  editDraft(saved.draft, ['http', 'url'], 'https://example.com/edited');
  const changed = { ...source, document: { ...source.document, foreign: { keep: false } } };
  assert.throws(() => refreshTabDraft(saved, changed), /changed on disk/);
  recoverAsUntitled(saved, 'File changed.');
  assert.equal(saved.path, '');
  assert.equal(saved.draft.value.foreign.keep, true);
  assert.equal(saved.draft.value.http.url, 'https://example.com/edited');
  assert.equal(changed.document.foreign.keep, false);
  assert.deepEqual(captureRecovery(1, [], 0, null, '', false).tabs, []);
});
