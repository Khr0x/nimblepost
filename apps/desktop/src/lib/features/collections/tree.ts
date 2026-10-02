export interface TreeNode { name: string; path?: string; folderPath?: string; children: TreeNode[] }

export function fileTree(paths: string[], folders: string[] = []): TreeNode[] {
  const roots: TreeNode[] = [];
  for (const path of [...folders.map(path => `${path}/`), ...paths]) {
    let nodes = roots;
    const parts = path.split('/').filter(Boolean);
    for (let i = 0; i < parts.length; i++) {
      const name = parts[i];
      // ponytail: linear sibling lookup; use a per-folder Map if large collections make indexing slow.
      let node = nodes.find(node => node.name === name);
      if (!node) { node = { name, children: [] }; nodes.push(node); }
      if (i === parts.length - 1 && !path.endsWith('/')) node.path = path;
      else node.folderPath = parts.slice(0, i + 1).join('/');
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
