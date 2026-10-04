<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { ChevronRight, Ellipsis, Folder, Plus } from '@lucide/svelte';
  import { Button } from '$lib/components/ui/button';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import type { Collection, RequestSummary } from '$lib/desktop/api';
  import { treeRows, treeWindow, treeKey, TREE_ROW_HEIGHT, type TreeNode, type TreeRow, type TreeKind } from './tree';

  let { collections, summaries, summariesPending, onvisible, selectedRoot, selectedPath, activeSummary, locked, onchoose, oncreate, oncontextmenu, onactions }: {
    collections: { owner: Collection; nodes: TreeNode[] }[];
    summaries: Record<string, Record<string, RequestSummary>>;
    summariesPending: boolean;
    onvisible: (owners: { root: string; paths: string[] }[]) => void;
    selectedRoot: string; selectedPath: string; activeSummary: RequestSummary | null; locked: boolean;
    onchoose: (root: string, path?: string) => unknown;
    oncreate: (root: string, kind: 'request', folder: string) => unknown;
    oncontextmenu: (event: MouseEvent | PointerEvent) => void;
    onactions: (event: MouseEvent) => void;
  } = $props();
  const id = $props.id();
  let viewport = $state<HTMLElement | null>(null);
  let height = $state(0);
  let scrollTop = $state(0);
  let closed = $state(new Set<string>());
  let focused = $state('');
  const rows = $derived(treeRows(collections, closed));
  const range = $derived(treeWindow(rows.length, scrollTop, height));
  const visible = $derived(rows.slice(range.start, range.end));
  const focusIndex = $derived(rows.findIndex(row => row.key === focused));
  const activeDescendant = $derived(focusIndex >= range.start && focusIndex < range.end ? `${id}-${focusIndex}` : undefined);

  $effect(() => {
    const owners = new Map<string, string[]>();
    for (const row of visible) if (row.kind === 'request' && !row.owner.warning) {
      const paths = owners.get(row.owner.root) ?? [];
      paths.push(row.path); owners.set(row.owner.root, paths);
    }
    untrack(() => onvisible([...owners].map(([root, paths]) => ({ root, paths }))));
  });

  $effect(() => {
    if (!viewport) return;
    const element = viewport;
    const resize = new ResizeObserver(() => height = element.clientHeight);
    height = element.clientHeight;
    resize.observe(element);
    return () => resize.disconnect();
  });
  $effect(() => {
    const roots = new Set(collections.map(({ owner }) => owner.root));
    untrack(() => {
      const retained = new Set([...closed].filter(key => roots.has(JSON.parse(key)[0])));
      if (retained.size !== closed.size) closed = retained;
    });
  });
  $effect(() => {
    const root = selectedRoot, path = selectedPath;
    if (root && path) untrack(() => { void reveal(root, path); });
  });
  function scroll() {
    if (!viewport) return;
    // A focused action must not silently become an action for a different request.
    if (treeWindow(rows.length, viewport.scrollTop, height).start !== range.start &&
        document.activeElement !== viewport && viewport.contains(document.activeElement)) viewport.focus({ preventScroll: true });
    scrollTop = viewport.scrollTop;
  }
  function toggle(row: TreeRow) {
    if (locked) return;
    const next = new Set(closed);
    if (next.has(row.key)) next.delete(row.key); else next.add(row.key);
    closed = next; focused = row.key;
  }
  export function collapse(root: string) { closed = new Set([...closed, treeKey(root, 'collection')]); }
  async function show(index: number, focus = false) {
    if (!viewport?.isConnected || index < 0 || !rows[index]) return null;
    const key = rows[index].key;
    const top = index * TREE_ROW_HEIGHT;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + TREE_ROW_HEIGHT > viewport.scrollTop + height) viewport.scrollTop = Math.max(0, top + TREE_ROW_HEIGHT - height);
    scrollTop = viewport.scrollTop;
    if (focus) { focused = rows[index].key; viewport.focus({ preventScroll: true }); }
    await tick();
    return viewport?.isConnected ? Array.from(viewport.querySelectorAll<HTMLElement>('[data-tree-key]')).find(row => row.dataset.treeKey === key) ?? null : null;
  }
  export async function reveal(root: string, path: string, kind: TreeKind = 'request') {
    const next = new Set(closed);
    if (kind !== 'collection') next.delete(treeKey(root, 'collection'));
    const parts = path.split('/');
    for (let count = 1; count < parts.length; count++) next.delete(treeKey(root, 'folder', parts.slice(0, count).join('/')));
    if (next.size !== closed.size) closed = next;
    if (kind === 'request' && root === selectedRoot && path === selectedPath) focused = treeKey(root, kind, path);
    await tick();
    if (!viewport?.isConnected) return null;
    return show(rows.findIndex(row => row.key === treeKey(root, kind, path)));
  }
  function activate(row: TreeRow) {
    if (locked) return;
    if (row.kind === 'request') return onchoose(row.owner.root, row.path);
    else if (row.kind === 'collection' && !row.owner.warning) return onchoose(row.owner.root);
    else if (row.kind === 'folder') toggle(row);
    else if (row.kind === 'empty' && !row.owner.readOnly) oncreate(row.owner.root, 'request', row.path);
  }
  function keydown(event: KeyboardEvent) {
    if (locked || event.altKey || event.ctrlKey || event.metaKey) return;
    if (event.target !== viewport && (event.key === 'Enter' || event.key === ' ')) return;
    const index = focusIndex < 0 ? range.start : focusIndex;
    const row = rows[index];
    if (!row) return;
    let next = index;
    if (event.key === 'ArrowDown') next++;
    else if (event.key === 'ArrowUp') next--;
    else if (event.key === 'Home') next = 0;
    else if (event.key === 'End') next = rows.length - 1;
    else if (event.key === 'PageDown') next += Math.max(1, Math.floor(height / TREE_ROW_HEIGHT));
    else if (event.key === 'PageUp') next -= Math.max(1, Math.floor(height / TREE_ROW_HEIGHT));
    else if (event.key === 'ArrowRight') {
      if (row.kind === 'folder' || row.kind === 'collection') { if (closed.has(row.key)) toggle(row); else if (rows[index + 1]?.parent === row.key) next++; }
    } else if (event.key === 'ArrowLeft') {
      if ((row.kind === 'folder' || row.kind === 'collection') && !closed.has(row.key)) toggle(row);
      else if (row.parent) next = rows.findIndex(item => item.key === row.parent);
    } else if (event.key === 'Enter') {
      const result = activate(row);
      if (row.kind === 'request' || row.kind === 'collection') {
        event.preventDefault(); event.stopPropagation();
        void Promise.resolve(result).then(() => { if (viewport?.isConnected) return show(rows.findIndex(item => item.key === row.key), true); });
        return;
      }
    }
    else if (event.key === ' ') { if (row.kind === 'folder' || row.kind === 'collection') toggle(row); else activate(row); }
    else if (event.key === 'F10' && event.shiftKey || event.key === 'ContextMenu') {
      const button = viewport?.querySelector<HTMLElement>(`[id="${id}-${index}"] .tree-action`);
      button?.click();
      event.preventDefault(); event.stopPropagation(); return;
    } else return;
    event.preventDefault(); event.stopPropagation();
    void show(Math.max(0, Math.min(rows.length - 1, next)), true);
  }
</script>

<!-- One scroll viewport and a bounded set of fixed-height rows, including headers. -->
<ContextMenu.Trigger class="tree-scroll" bind:ref={viewport} role="tree" aria-label="Collections" aria-describedby={`${id}-help`} tabindex={0} aria-activedescendant={activeDescendant} inert={locked} aria-busy={locked} disabled={locked} {oncontextmenu} onkeydown={keydown} onscroll={scroll} onfocus={() => { if (focusIndex < 0 && rows.length) focused = rows[range.start].key; }} onfocusin={(event) => { const key = (event.target as Element).closest<HTMLElement>('[data-tree-key]')?.dataset.treeKey; if (key) focused = key; }} data-tree-total={rows.length} data-tree-row-height={TREE_ROW_HEIGHT} data-tree-labels-pending={summariesPending}>
  <span id={`${id}-help`} class="sr-only">Use arrow keys to navigate, Enter to open a request, and Shift F10 for actions.</span>
  <div class="tree-virtual-space" style:height={`${rows.length * TREE_ROW_HEIGHT}px`}>
    <!-- Reuse viewport slots; request identity lives in row data and logical focus. -->
    {#each visible as row, offset}
      {@const index = range.start + offset}
      {@const branch = row.kind === 'collection' || row.kind === 'folder'}
      {@const active = row.kind === 'request' && row.owner.root === selectedRoot && row.path === selectedPath}
      {@const summary = active && activeSummary ? activeSummary : summaries[row.owner.root]?.[row.path]}
      <div id={`${id}-${index}`} role="treeitem" tabindex={-1} onkeydown={keydown} aria-level={row.depth + 1} aria-posinset={row.position} aria-setsize={row.siblings} aria-expanded={branch ? !closed.has(row.key) : undefined} aria-selected={row.kind === 'request' ? active : undefined} aria-disabled={row.kind === 'warning' || !!row.owner.warning || row.kind === 'empty' && row.owner.readOnly} aria-label={row.kind === 'request' ? `${summary?.method ?? 'HTTP'} ${summary?.name ?? row.name.replace(/\.ya?ml$/, '')}` : row.name} class="tree-virtual-row tree-row" class:active class:workspace-collection={row.kind === 'collection'} class:tree-focused={row.key === focused} style:top={`${index * TREE_ROW_HEIGHT}px`} style:--tree-depth={row.depth} data-tree-key={row.key} data-tree-root={row.owner.root} data-tree-path={row.path} data-tree-folder={row.kind === 'folder' ? 'true' : row.kind === 'request' ? 'false' : undefined} data-tree-collection={row.kind === 'collection' ? 'true' : undefined} onclick={() => focused = row.key}>
        {#if row.kind === 'collection'}
          <Button variant="ghost" class="tree-toggle" tabindex={-1} aria-label={`${closed.has(row.key) ? 'Expand' : 'Collapse'} collection ${row.name}`} aria-expanded={!closed.has(row.key)} onclick={() => toggle(row)}><ChevronRight size={12} class={`tree-chevron ${!closed.has(row.key) ? 'expanded' : ''}`} aria-hidden="true" /></Button>
          <Button variant="ghost" class={`collection-select ${selectedRoot === row.owner.root ? 'current' : ''}`} tabindex={-1} title={row.owner.root} disabled={!!row.owner.warning} onclick={() => activate(row)}><Folder size={14} aria-hidden="true" /><span>{row.name}</span>{#if row.owner.readOnly}<span class="readonly-tag">RO</span>{/if}</Button>
        {:else if row.kind === 'request'}
          <Button variant="ghost" class={`file ${active ? 'active' : ''}`} tabindex={-1} title={row.path} onclick={() => activate(row)}><span class="request-method http-method" data-method={summary?.method} title={summary?.method}>{summary?.method === 'DELETE' ? 'DEL' : summary?.method === 'OPTIONS' ? 'OPT' : summary?.method ?? 'HTTP'}</span><span class="tree-name">{summary?.name ?? row.name.replace(/\.ya?ml$/, '')}</span></Button>
        {:else if row.kind === 'folder'}
          <Button variant="ghost" class="folder-toggle folder-label" tabindex={-1} aria-expanded={!closed.has(row.key)} onclick={() => toggle(row)}><ChevronRight size={12} class={`tree-chevron ${!closed.has(row.key) ? 'expanded' : ''}`} aria-hidden="true" /><Folder size={14} class="folder-symbol" aria-hidden="true" /><span class="tree-name">{row.name}</span></Button>
        {:else if row.kind === 'empty'}
          <Button variant="ghost" class="tree-create" tabindex={-1} aria-label={`New request in ${row.owner.name}${row.path ? ` / ${row.path}` : ''}`} disabled={row.owner.readOnly} onclick={() => activate(row)}><Plus size={14} aria-hidden="true" />Request</Button>
        {:else}<span class="tree-warning" role="alert" title={row.name}>{row.name}</span>{/if}
        {#if branch || row.kind === 'request'}<Button variant="ghost" class="tree-action" tabindex={-1} aria-label={row.kind === 'collection' ? `Actions for collection ${row.name}` : row.kind === 'folder' ? `Actions for folder ${row.name}` : `Actions for ${row.name}`} title={`${row.kind === 'request' ? 'Request' : row.kind === 'folder' ? 'Folder' : 'Collection'} actions`} aria-haspopup="menu" disabled={row.kind !== 'collection' && row.owner.readOnly} onclick={onactions}><Ellipsis size={14} aria-hidden="true" /></Button>{/if}
      </div>
    {/each}
  </div>
</ContextMenu.Trigger>
