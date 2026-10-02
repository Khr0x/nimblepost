import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileTree } from '../../src/lib/features/collections/tree.ts';

test('el árbol incluye carpetas vacías, agrupa rutas y ordena carpetas antes de archivos', () => {
  const tree = fileTree(['z.yml', 'users/list.yml', 'users/admin/create.yaml'], ['empty', 'users', 'users/admin']);
  assert.deepEqual(tree.map(node => node.name), ['empty', 'users', 'z.yml']);
  assert.equal(tree[0].folderPath, 'empty');
  assert.deepEqual(tree[0].children, []);
  assert.deepEqual(tree[1].children.map(node => node.name), ['admin', 'list.yml']);
  assert.equal(tree[1].children[0].folderPath, 'users/admin');
  assert.equal(tree[1].children[0].children[0].path, 'users/admin/create.yaml');
  assert.equal(tree[1].children[1].path, 'users/list.yml');
  assert.deepEqual(fileTree([]), []);
});
