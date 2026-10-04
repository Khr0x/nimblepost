import type { Collection } from '../../desktop/api';

export interface TreeNode { name: string; path?: string; folderPath?: string; children: TreeNode[] }
export type TreeKind = 'collection' | 'folder' | 'request' | 'empty' | 'warning';
export interface TreeRow { key: string; owner: Collection; kind: TreeKind; path: string; name: string; depth: number; position: number; siblings: number; parent?: string }
export const TREE_ROW_HEIGHT = 28;
export const TREE_OVERSCAN = 6;
export const treeKey = (root: string, kind: TreeKind, path = '') => JSON.stringify([root, kind, path]);

export function treeRows(collections: { owner: Collection; nodes: TreeNode[] }[], closed: ReadonlySet<string>): TreeRow[] {
  const rows: TreeRow[] = [];
  function branch(owner: Collection, nodes: TreeNode[], depth: number, parent: string, folder = '') {
    if (!nodes.length) rows.push({ key: treeKey(owner.root, 'empty', folder), owner, kind: 'empty', path: folder, name: 'New request', depth, position: 1, siblings: 1, parent });
    nodes.forEach((node, index) => {
      const kind = node.path ? 'request' : 'folder';
      const path = node.path ?? node.folderPath!;
      const key = treeKey(owner.root, kind, path);
      rows.push({ key, owner, kind, path, name: node.name, depth, position: index + 1, siblings: nodes.length, parent });
      if (kind === 'folder' && !closed.has(key)) branch(owner, node.children, depth + 1, key, path);
    });
  }
  collections.forEach(({ owner, nodes }, index) => {
    const key = treeKey(owner.root, 'collection');
    rows.push({ key, owner, kind: 'collection', path: '', name: owner.name, depth: 0, position: index + 1, siblings: collections.length });
    if (closed.has(key)) return;
    if (owner.warning) rows.push({ key: treeKey(owner.root, 'warning'), owner, kind: 'warning', path: '', name: owner.warning, depth: 1, position: 1, siblings: 1, parent: key });
    else branch(owner, nodes, 1, key);
  });
  return rows;
}

export function treeWindow(length: number, scrollTop: number, height: number) {
  const top = Math.min(Math.max(0, scrollTop), Math.max(0, length * TREE_ROW_HEIGHT - height));
  return { start: Math.max(0, Math.floor(top / TREE_ROW_HEIGHT) - TREE_OVERSCAN), end: Math.min(length, Math.ceil((top + height) / TREE_ROW_HEIGHT) + TREE_OVERSCAN) };
}

export function fileTree(paths: string[], folders: string[] = []): TreeNode[] {
  const roots: TreeNode[] = [];
  const index = new Map<string, TreeNode>();
  for (const path of [...folders.map(path => `${path}/`), ...paths]) {
    let nodes = roots;
    let prefix = '';
    const parts = path.split('/').filter(Boolean);
    for (let i = 0; i < parts.length; i++) {
      const name = parts[i];
      prefix = prefix ? `${prefix}/${name}` : name;
      let node = index.get(prefix);
      if (!node) { node = { name, children: [] }; nodes.push(node); index.set(prefix, node); }
      if (i === parts.length - 1 && !path.endsWith('/')) node.path = path;
      else node.folderPath = prefix;
      nodes = node.children;
    }
  }
  function sort(nodes: TreeNode[]) {
    nodes.sort((a, b) => Number(!!a.path) - Number(!!b.path) || a.name.localeCompare(b.name));
    nodes.forEach(node => sort(node.children));
  }
  sort(roots);
  return roots;
}
