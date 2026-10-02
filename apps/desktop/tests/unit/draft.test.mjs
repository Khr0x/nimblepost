import assert from 'node:assert/strict';
import { test } from 'node:test';
import { draftFrom, editDraft } from '../../src/lib/features/requests/draft.ts';

test('el borrador conserva campos ajenos y coalesce typing sin perder el orden de filas', () => {
  const view = { revision: 7, document: { http: { method: 'GET', headers: [{ name: 'X', value: 'old', description: 'keep' }] }, foreign: { keep: true } }, diagnostics: [] };
  const draft = draftFrom(view);
  editDraft(draft, ['http', 'headers', '0', 'value'], 'n');
  editDraft(draft, ['http', 'headers', '0', 'value'], 'new');
  assert.equal(draft.edits.length, 1);
  editDraft(draft, ['http', 'headers', '1'], { name: 'Y', value: 'v' });
  editDraft(draft, ['http', 'headers', '0'], null);
  assert.deepEqual(draft.value.http.headers, [{ name: 'Y', value: 'v' }]);
  assert.deepEqual(draft.value.foreign, view.document.foreign);
  assert.equal(view.document.http.headers[0].value, 'old');
  assert.deepEqual(draft.edits.map(edit => edit.path), [['http', 'headers', '0', 'value'], ['http', 'headers', '1'], ['http', 'headers', '0']]);
});

test('crear variables y declarar secretos elimina el valor persistido de la declaración', () => {
  const draft = draftFrom({ revision: 1, document: { name: 'local' }, diagnostics: [] });
  editDraft(draft, ['variables'], [{ name: 'token', value: 'old public value' }]);
  editDraft(draft, ['variables', '0', 'value'], null);
  editDraft(draft, ['variables', '0', 'secret'], true);
  editDraft(draft, ['variables', '0', 'type'], 'string');
  assert.deepEqual(draft.value.variables, [{ name: 'token', secret: true, type: 'string' }]);
});
