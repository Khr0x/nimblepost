// Vite removes this module from ordinary builds. The native harness uses real IPC.
import { emit } from '@tauri-apps/api/event';
import { tick } from 'svelte';
import { api } from './api';
import { frontendStages, recordStage, stopProfiling } from './native-profile';
import { TREE_ROW_HEIGHT, TREE_OVERSCAN } from '../features/collections/tree';

const frames = () => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
const profile = () => ({ frontendStages: frontendStages(),
  logicalRows: Number(document.querySelector<HTMLElement>('.tree-scroll')?.dataset.treeTotal ?? 0),
  rowLimit: Math.ceil((document.querySelector('.tree-scroll')?.clientHeight ?? 0) / TREE_ROW_HEIGHT) + TREE_OVERSCAN * 2 + 1,
  activeRequestRendered: !!document.querySelector('.tree-row.active .file'),
  labelsReady: document.querySelector<HTMLElement>('.tree-scroll')?.dataset.treeLabelsPending !== 'true',
  labeledRequestRows: document.querySelectorAll('.tree-row .request-method[data-method]').length,
  treeRows: document.querySelectorAll('.tree-row').length,
  requestRows: document.querySelectorAll('.tree-row[data-tree-folder="false"]').length,
  treeElements: document.querySelectorAll('.tree-scroll *').length });

interface MemoryControls {
  url: string;
  treeOnly?: boolean;
  responseOnly?: boolean;
  heapInspector?: boolean;
  hiddenResponseView?: boolean;
  hideResponseView: (hidden: boolean) => void;
  open: (path: string) => Promise<void>;
  activate: (id: number) => Promise<void>;
  close: (id: number) => Promise<void>;
  send: (url: string) => Promise<void>;
  more: () => Promise<void>;
  reset: () => Promise<void>;
  inspect: () => { tabs: number[]; activeBodyChars: number; activeOffset: number; bodyBytes: number; cachedBodyChars: number; activeCachedBodyChars: number; labels: number; attempts: number; treeCaches: number; error: string; recoveryWarning: string };
}

async function memoryUsage(controls: MemoryControls, requests: number) {
  stopProfiling();
  const inspector = console as Console & { takeHeapSnapshot?: (name: string) => void };
  let index = 0;
  const responseViews: WeakRef<Element>[] = [];
  if (controls.responseOnly) controls.hideResponseView(!!controls.hiddenResponseView);
  let added = 0, removed = 0;
  const detached: WeakRef<Element>[] = [];
  const tree = document.querySelector('.tree-scroll');
  const observer = controls.treeOnly ? new MutationObserver(records => {
    for (const record of records) {
      record.addedNodes.forEach(node => { if (node instanceof Element && node.matches('.tree-row')) added++; });
      record.removedNodes.forEach(node => {
        if (!(node instanceof Element) || !node.matches('.tree-row')) return;
        removed++;
        if (detached.length < 128) detached.push(new WeakRef(node));
      });
    }
  }) : null;
  if (tree) observer?.observe(tree, { childList: true, subtree: true });
  try {
    const phase = async (name: string) => {
      await tick(); await frames();
      if (controls.heapInspector) inspector.takeHeapSnapshot?.(name);
      const state = controls.inspect();
      if (state.error || state.recoveryWarning) throw new Error(state.error || state.recoveryWarning);
      await emit('native-validation-memory', { index: index++, name, ...state, ...profile(), ...(controls.responseOnly ? { responseViews: {
        mounted: document.querySelectorAll('.response-panel').length,
        renderedLines: document.querySelectorAll('[data-response-line]').length,
        sampled: responseViews.length, alive: responseViews.filter(ref => ref.deref()).length,
      } } : {}), ...(observer ? { rowLifecycle: { added, removed, sampledDetached: detached.length, sampledStillAlive: detached.filter(ref => ref.deref()).length } } : {}) });
      // The host samples the native process and WebView helpers between phases.
      await new Promise(resolve => setTimeout(resolve, 6000));
    };
    const body = async () => {
      while (controls.inspect().activeOffset < controls.inspect().bodyBytes) {
        const previous = controls.inspect().activeOffset;
        await controls.more();
        if (controls.inspect().error || controls.inspect().activeOffset <= previous) throw new Error('Response chunks stopped advancing');
      }
    };
    await phase('baseline');
    if (!controls.treeOnly) {
      for (let request = 1; request < 64; request++) await controls.open(`request-${String(request).padStart(5, '0')}.yml`);
      if (controls.inspect().tabs.length !== 64) throw new Error('Memory scenario did not open 64 tabs');
      await phase('tabs-open');
      const tabs = controls.inspect().tabs;
      for (const tab of tabs.slice(-4)) {
        await controls.activate(tab); await controls.send(controls.url); await body();
        if (controls.responseOnly) {
          const view = document.querySelector('.response-panel');
          if (view) responseViews.push(new WeakRef(view));
        }
      }
      // Switching away and back also caches the final response in its tab.
      await controls.activate(tabs[0]); await controls.activate(tabs.at(-1)!);
      await phase('responses-loaded');
      if (controls.responseOnly) {
        await controls.activate(tabs[0]); await phase('switched-empty');
        await controls.activate(tabs.at(-1)!); await phase('switched-back');
        for (const tab of controls.inspect().tabs) await controls.close(tab);
        await phase('tabs-closed'); await phase('closed-settled');
        await controls.reset(); await phase('workspace-reset'); await phase('reset-settled');
        return;
      }
      const pending = controls.send(`${controls.url}/delayed`);
      await phase('resend-pending');
      await pending; await body();
      await phase('response-replaced');
    }
    for (const tab of controls.inspect().tabs) await controls.close(tab);
    if (controls.inspect().tabs.length) throw new Error('Memory scenario did not close all tabs');
    await phase('tabs-closed');
    for (let sweep = 1; sweep <= (controls.treeOnly ? 6 : 2); sweep++) {
      const scroll = document.querySelector<HTMLElement>('.tree-scroll');
      if (!scroll) throw new Error('Tree scroll container is missing');
      const step = Math.max(1, Math.floor(scroll.clientHeight / TREE_ROW_HEIGHT));
      for (let row = 0; row < requests + step; row += step) {
        scroll.scrollTop = row * TREE_ROW_HEIGHT;
        scroll.dispatchEvent(new Event('scroll'));
        await tick(); await frames();
        const deadline = performance.now() + 60_000;
        while (!profile().labelsReady) {
          if (performance.now() > deadline) throw new Error('Navigation labels timed out');
          await new Promise(resolve => setTimeout(resolve, 10));
        }
      }
      await phase(`navigation-${sweep}`);
    }
    await controls.reset();
    await phase('workspace-reset');
  } finally { observer?.disconnect(); }
}

export async function validateNative(state: { error: string; requests: number; tabs: number; untitled: boolean }, renderStarted: number, memory?: MemoryControls) {
  await tick();
  recordStage('final-dom-flush', renderStarted);
  const frameStarted = performance.now();
  await frames();
  recordStage('ready-frame-wait', frameStarted);
  const url = document.querySelector<HTMLInputElement>('[aria-label="Request URL"]');
  await emit('native-validation-ready', {
    ...state,
    rendered: !!document.querySelector('.app-shell'),
    url: url?.value ?? null,
    urlEnabled: url ? !url.disabled : null,
    profile: profile(),
  });
  // Labels load independently of draft recovery. Keep ready comparable to the
  // existing benchmark, but wait for the current window's labels before sampling.
  const deadline = performance.now() + 60_000;
  while (!profile().labelsReady) {
    if (performance.now() > deadline) {
      await emit('native-validation-profile', { error: 'Visible request labels did not finish loading' });
      return;
    }
    await new Promise(resolve => setTimeout(resolve, 25));
  }
  await tick();
  const settledFrameStarted = performance.now();
  await frames();
  recordStage('settled-frame-wait', settledFrameStarted);
  await emit('native-validation-profile', { error: '', ...profile() });
  // Let the harness sample idle memory/CPU before the IPC smoke operations.
  await new Promise(resolve => setTimeout(resolve, 12_000));
  const checks: string[] = [];
  try {
    if (state.error) throw new Error(state.error);
    await Promise.all([import('../features/workspaces/WorkspaceManager.svelte'), import('../features/responses/ResponsePanel.svelte'), import('../features/requests/BodyEditor.svelte')]);
    checks.push('lazy-panel-chunks');
    const recovered = await api.recovery();
    if (recovered.warning) throw new Error(recovered.warning);
    if (recovered.snapshot) {
      await api.saveRecovery(recovered.snapshot);
      checks.push('recovery-read-write');
    }
    if (memory) { await memoryUsage(memory, state.requests); checks.push('memory-usage'); }
    const example = await api.example();
    if (!example.readOnly || !example.requests.length) throw new Error('Bundled example is missing');
    const request = await api.request(example.requests[0]);
    if (!request.revision || request.diagnostics.length) throw new Error('Bundled request is invalid');
    checks.push('bundled-example', 'request-ipc');
    const workspace = await api.refreshCollections();
    if (workspace.warning || workspace.watchWarning) throw new Error(workspace.warning || workspace.watchWarning!);
    checks.push('collection-refresh');
    await emit('native-validation-smoke', { checks, error: '' });
  } catch (error) {
    await emit('native-validation-smoke', { checks, error: String(error) });
  }
}
