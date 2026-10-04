import assert from 'node:assert/strict';
import { test } from 'node:test';
import { fileTree, treeRows, treeKey, treeWindow, TREE_ROW_HEIGHT, TREE_OVERSCAN } from '../../src/lib/features/collections/tree.ts';

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

test('solo las ramas abiertas forman filas; la ventana cubre el viewport y su margen al recortar el árbol', () => {
  const owner = { name: 'API', root: '/api', requests: ['users/admin/create.yml', 'root.yml'], folders: ['empty'], readOnly: false };
  const collections = [{ owner, nodes: fileTree(owner.requests, owner.folders) }];
  const open = treeRows(collections, new Set());
  assert.deepEqual(open.map(row => row.kind), ['collection', 'folder', 'empty', 'folder', 'folder', 'request', 'request']);
  const closed = treeRows(collections, new Set([treeKey(owner.root, 'folder', 'users')]));
  assert.deepEqual(closed.map(row => row.path), ['', 'empty', 'empty', 'users', 'root.yml']);
  assert.deepEqual(treeRows(collections, new Set([treeKey(owner.root, 'collection')])).map(row => row.kind), ['collection']);
  assert.equal(open[5].parent, open[4].key);
  assert.equal(open[5].depth, 3);
  assert.equal(open[6].position, 3);
  const middle = treeWindow(5000, 2000 * TREE_ROW_HEIGHT, 20 * TREE_ROW_HEIGHT);
  assert.deepEqual(middle, { start: 2000 - TREE_OVERSCAN, end: 2020 + TREE_OVERSCAN });
  assert.deepEqual(treeWindow(3, 5000 * TREE_ROW_HEIGHT, 20 * TREE_ROW_HEIGHT), { start: 0, end: 3 });
  assert.deepEqual(treeWindow(0, 0, 100), { start: 0, end: 0 });
});

test('el índice distingue prefijos y nombres especiales, sin duplicar carpetas ni requests', () => {
  const tree = fileTree(['a/b.yml', 'ab/b.yml', 'constructor/list.yml', '__proto__/list.yml', 'a/b.yml'], ['a', 'a/empty', 'a']);
  assert.deepEqual(tree.map(node => node.name), ['__proto__', 'a', 'ab', 'constructor']);
  assert.deepEqual(tree[1].children.map(node => node.name), ['empty', 'b.yml']);
  assert.equal(tree[1].children[1].path, 'a/b.yml');
  assert.equal(tree[2].children[0].path, 'ab/b.yml');
  assert.equal(tree[0].children[0].path, '__proto__/list.yml');
  assert.equal(tree[3].folderPath, 'constructor');
});
