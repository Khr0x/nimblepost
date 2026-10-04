<script lang="ts">
  import { onMount, tick, untrack } from 'svelte';
  import { version as appVersion } from '../package.json';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { listen } from '@tauri-apps/api/event';
  import { readTheme, saveTheme, type Theme } from '$lib/layout/theme';
  import { SIDEBAR_MIN_WIDTH, sidebarMaxWidth, clampSidebarWidth, readSidebarWidth, saveSidebarWidth } from '$lib/layout/sidebar';
  import { RESPONSE_DEFAULT_HEIGHT, responseMinHeight, responseMaxHeight, clampResponseHeight, readResponseHeight, saveResponseHeight } from '$lib/features/responses/response-layout';
  import { api, type Collection, type CollectionChange, type WorkspaceView, type ResponseMeta, type HistoryView, type RequestInfo, type RequestSummary } from '$lib/desktop/api';
  import { fileTree, type TreeNode, type TreeKind } from '$lib/features/collections/tree';
  import CollectionTree from '$lib/features/collections/CollectionTree.svelte';
  import { recordStage } from '$lib/desktop/native-profile';
  import { draftFrom, editDraft, type Draft } from '$lib/features/requests/draft';
  import { untitledDraft, refreshTabDraft, refreshExternalTab, type RequestTab } from '$lib/features/requests/request-tabs';
  import { captureRecovery, restoreRecovery, recoverAsUntitled, type RecoverySnapshot } from '$lib/features/requests/recovery';
  import RequestEditor from '$lib/features/requests/RequestEditor.svelte';
  import RequestTabs from '$lib/features/requests/RequestTabs.svelte';
  let validationHideResponseView = $state(false);
  import ResponseText from '$lib/features/responses/ResponseText.svelte';
  import brandLight from './assets/nimblepost-glyph-light.svg';
  import brandDark from './assets/nimblepost-glyph-dark.svg';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Breadcrumb from '$lib/components/ui/breadcrumb';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Collapsible from '$lib/components/ui/collapsible';
  import { Separator } from '$lib/components/ui/separator';
  import * as Select from '$lib/components/ui/select';
  import * as ContextMenu from '$lib/components/ui/context-menu';
  import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
  import { Check, ChevronDown, Copy, Folder, FolderOpen, FolderPlus, History, House, LoaderCircle, Pencil, Plus, RotateCcw, Save, Send, Settings2, Trash2, X } from '@lucide/svelte';

  let theme = $state<Theme>(readTheme());
  let sidebarWidth = $state(readSidebarWidth());
  let viewportWidth = $state(window.innerWidth);
  const visibleSidebarWidth = $derived(clampSidebarWidth(sidebarWidth, viewportWidth));
  let resizingSidebar = $state(false);
  let sidebarDrag: { pointerId: number; x: number; width: number } | null = null;
  function startSidebarResize(event: PointerEvent) {
    if (event.button !== 0 || sidebarDrag) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    sidebarDrag = { pointerId: event.pointerId, x: event.clientX, width: visibleSidebarWidth };
    resizingSidebar = true;
  }
  function resizeSidebar(event: PointerEvent) {
    if (sidebarDrag?.pointerId !== event.pointerId) return;
    sidebarWidth = clampSidebarWidth(sidebarDrag.width + event.clientX - sidebarDrag.x, viewportWidth);
  }
  function finishSidebarResize(event: PointerEvent) {
    if (sidebarDrag?.pointerId !== event.pointerId) return;
    sidebarDrag = null; resizingSidebar = false;
    saveSidebarWidth(sidebarWidth);
  }
  function sidebarResizeKey(event: KeyboardEvent) {
    const widths: Record<string, number> = { ArrowLeft: visibleSidebarWidth - 16, ArrowRight: visibleSidebarWidth + 16, Home: SIDEBAR_MIN_WIDTH, End: sidebarMaxWidth(viewportWidth) };
    if (!(event.key in widths)) return;
    event.preventDefault();
    sidebarWidth = clampSidebarWidth(widths[event.key], viewportWidth);
    saveSidebarWidth(sidebarWidth);
  }
  function resetSidebarWidth() { sidebarWidth = SIDEBAR_MIN_WIDTH; saveSidebarWidth(sidebarWidth); }
  let responseHeight = $state(readResponseHeight());
  let responseAreaHeight = $state(0);
  const visibleResponseHeight = $derived(clampResponseHeight(responseHeight, responseAreaHeight));
  let resizingResponse = $state(false);
  let responseDrag: { pointerId: number; y: number; height: number } | null = null;
  function startResponseResize(event: PointerEvent) {
    if (event.button !== 0 || responseDrag) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    responseDrag = { pointerId: event.pointerId, y: event.clientY, height: visibleResponseHeight };
    resizingResponse = true;
  }
  function resizeResponse(event: PointerEvent) {
    if (responseDrag?.pointerId !== event.pointerId) return;
    responseHeight = clampResponseHeight(responseDrag.height + responseDrag.y - event.clientY, responseAreaHeight);
  }
  function finishResponseResize(event: PointerEvent) {
    if (responseDrag?.pointerId !== event.pointerId) return;
    responseDrag = null; resizingResponse = false;
    saveResponseHeight(responseHeight);
  }
  function responseResizeKey(event: KeyboardEvent) {
    const heights: Record<string, number> = { ArrowUp: visibleResponseHeight + 16, ArrowDown: visibleResponseHeight - 16, Home: responseMinHeight(responseAreaHeight), End: responseMaxHeight(responseAreaHeight) };
    if (!(event.key in heights)) return;
    event.preventDefault();
    responseHeight = clampResponseHeight(heights[event.key], responseAreaHeight);
    saveResponseHeight(responseHeight);
  }
  function resetResponseHeight() { responseHeight = RESPONSE_DEFAULT_HEIGHT; saveResponseHeight(responseHeight); }
  $effect(() => {
    saveTheme(theme);
    if (isTauri()) void getCurrentWindow().setTheme(theme).catch(error => console.warn('Window theme could not be updated.', error));
  });

  let workspace = $state<WorkspaceView | null>(null);
  let requestSummaries = $state<Record<string, Record<string, RequestSummary>>>({});
  let summariesPending = $state(false);
  let visibleSummaryRequests: { root: string; paths: string[] }[] = [];
  const summaryAttempts = new Map<string, Set<string>>();
  function pruneSummaries(root: string, attempts: Set<string>) {
    const visible = new Set(visibleSummaryRequests.find(item => item.root === root)?.paths ?? []);
    // Keep the current viewport and a bounded recent set, including failed reads.
    const limit = Math.max(512, visible.size);
    for (const path of attempts) {
      if (attempts.size <= limit) break;
      if (visible.has(path)) continue;
      attempts.delete(path);
      if (requestSummaries[root]) delete requestSummaries[root][path];
    }
  }
  let summariesRunning = false, summariesDisposed = false;
  onMount(() => () => { summariesDisposed = true; });
  function visibleRequests(owners: typeof visibleSummaryRequests) {
    visibleSummaryRequests = owners;
    void loadVisibleSummaries();
  }
  async function loadVisibleSummaries() {
    if (summariesRunning || summariesDisposed) return;
    summariesRunning = true; summariesPending = true;
    const started = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
    let completed = 0;
    try {
      // One IPC at a time; scrolling replaces the next window instead of queuing it.
      while (!summariesDisposed) {
        const missing = (root: string, path: string) => !requestSummaries[root]?.[path] && !summaryAttempts.get(root)?.has(path);
        const owner = visibleSummaryRequests.find(owner => owner.paths.some(path => missing(owner.root, path)));
        if (!owner) break;
        const paths = owner.paths.filter(path => missing(owner.root, path)).slice(0, 128);
        const attempts = summaryAttempts.get(owner.root) ?? new Set<string>();
        summaryAttempts.set(owner.root, attempts);
        // Missing/invalid YAML also counts as attempted, so it cannot cause a retry loop.
        for (const path of paths) attempts.add(path);
        try {
          const readStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
          const summaries = await api.requestSummaries(owner.root, paths);
          if (import.meta.env.VITE_NATIVE_VALIDATION === '1') recordStage('summaries-ipc', readStarted, paths.length);
          completed += paths.length;
          // A refresh, workspace switch or removed path invalidates the pending read.
          if (summariesDisposed || summaryAttempts.get(owner.root) !== attempts) continue;
          const renderStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
          requestSummaries[owner.root] ??= {};
          for (const [path, summary] of Object.entries(summaries)) requestSummaries[owner.root][path] ??= summary;
          if (import.meta.env.VITE_NATIVE_VALIDATION === '1') {
            recordStage('summaries-assign', renderStarted, Object.keys(summaries).length);
            void tick().then(() => recordStage('summaries-dom-flush', renderStarted, Object.keys(summaries).length));
          }
        } catch (error) {
          if (!summariesDisposed && summaryAttempts.get(owner.root) === attempts) console.warn('Request labels could not be loaded.', error);
        } finally {
          if (!summariesDisposed && summaryAttempts.get(owner.root) === attempts) pruneSummaries(owner.root, attempts);
        }
      }
    } finally {
      summariesRunning = false;
      if (!summariesDisposed) summariesPending = false;
      if (import.meta.env.VITE_NATIVE_VALIDATION === '1') recordStage('summaries-load', started, completed);
    }
  }
  const treeCache = new Map<string, { requests: string[]; folders: string[]; nodes: TreeNode[] }>();
  const samePaths = (next: string[], previous: string[]) => next.length === previous.length && next.every((path, index) => path === previous[index]);
  let treeView = $state<CollectionTree | null>(null);
  const collectionTrees = $derived((workspace?.collections ?? []).map(owner => ({ owner, nodes: collectionTree(owner) })));
  function collectionTree(owner: Collection) {
    const cached = treeCache.get(owner.root);
    if (cached && samePaths(owner.requests, cached.requests) && samePaths(owner.folders, cached.folders)) return cached.nodes;
    const started = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
    const nodes = fileTree(owner.requests, owner.folders);
    treeCache.set(owner.root, { requests: [...owner.requests], folders: [...owner.folders], nodes });
    if (import.meta.env.VITE_NATIVE_VALIDATION === '1') recordStage('tree-build', started, owner.requests.length + owner.folders.length);
    return nodes;
  }
  $effect(() => {
    const roots = new Set((workspace?.collections ?? []).map(owner => owner.root));
    for (const root of treeCache.keys()) if (!roots.has(root)) treeCache.delete(root);
    untrack(() => {
      for (const root of summaryAttempts.keys()) if (!roots.has(root)) summaryAttempts.delete(root);
      for (const root of Object.keys(requestSummaries)) if (!roots.has(root)) delete requestSummaries[root];
    });
  });
  let workspaceValue = $state('1');
  const workspaceOptions = $derived((workspace?.workspaces ?? [{ id: 1, name: 'My Workspace' }]).map(item => ({ value: String(item.id), label: item.name })));
  $effect(() => { workspaceValue = String(workspace?.activeWorkspaceId ?? 1); });
  let collection = $state<Collection | null>(null);
  let home = $state(false);
  const welcomeCollection = $derived(home ? null : collection);
  let selected = $state('');
  let requestName = $state('');
  let requestNameEdit = $state<string | null>(null);
  const requestNameChanged = $derived(!!requestNameEdit?.trim() && requestNameEdit.trim() !== requestName);
  let method = $state('GET');
  const methodOptions = $derived(Array.from(new Set([method, 'GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'])).map(value => ({ value, label: value })));
  let url = $state('');
  let environment = $state('');
  const environmentOptions = $derived([{ value: '0', label: 'No environment' }, ...(collection?.environments ?? []).map((name, index) => ({ value: String(index + 1), label: name }))]);
  let baseUrl = $state('');
  let busy = $state(false);
  let loading = $state(true);
  let error = $state('');
  let response = $state<ResponseMeta | null>(null);
  let body = $state('');
  let offset = $state(0);
  let chunkLoading = $state(false);
  let decoder = new TextDecoder();
  let selectionVersion = 0;
  let draft = $state<Draft | null>(null);
  let bodyEditorReset = $state(0);
  let requestTabs = $state<RequestTab[]>([]);
  let activeRequestTab = $state(0);
  let nextRequestTab = 0;
  let closingRequestTab = $state<number | null>(null);
  let closeTabError = $state('');
  let closeAfterSave: number | null = null;
  const closingTab = $derived(requestTabs.find(item => item.id === closingRequestTab));
  const closingTabReadOnly = $derived(workspace?.collections.some(item => item.root === closingTab?.root && (item.readOnly || !!item.warning)) ?? false);
  const untitled = $derived(!!draft && !selected);
  const workbenchOpen = $derived(!home && (!!selected || !!draft || requestTabs.length > 0));
  const unsavedTabs = $derived(requestTabs.some(item => item.id === activeRequestTab ? !!draft?.edits.length : !!item.draft.edits.length));
  const isEdited = $derived(!!draft?.edits.length);
  let envDraft = $state<Draft | null>(null);
  let environmentOpen = $state(false);
  let EnvironmentManager = $state<typeof import('$lib/features/environments/EnvironmentManager.svelte').default | null>(null);
  let addingEnvironment = $state(false);
  let environmentMenuTrigger = $state<HTMLButtonElement | null>(null);
  let newEnvironment = $state('');
  let secretNames = $state<string[]>([]);
  let secrets = $state<Record<string, string>>({});
  let history = $state<HistoryView>({ entries: [], warning: null });
  let historyOpen = $state(false);
  let notice = $state('');
  let recoveryReady = $state(false);
  let recoveryBlocked = $state(false);
  let recoveryWarning = $state('');
  let watchWarning = $state('');
  let lastWatchWarning = '';
  const activeExternalChange = $derived(requestTabs.find(item => item.id === activeRequestTab)?.externalChange ?? '');
  const pendingExternalRoots = new Set<string>();
  let externalTimer: ReturnType<typeof setTimeout> | undefined;
  let externalDisposed = false;
  let closingWindow = $state(false);
  let recoveryWrite: Promise<boolean> | null = null;
  let recoveryPending: RecoverySnapshot | null = null;
  const requestSaved = $derived(notice === 'Request saved.' || notice === 'Request created and saved.');
  const requestReloaded = $derived(notice === 'Reloaded from disk.');
  const environmentSaved = $derived(notice === 'Environment saved.');
  const iconNotice = $derived(requestSaved || requestReloaded || environmentSaved);
  $effect(() => {
    if (!iconNotice) return;
    const currentNotice = notice;
    const timer = setTimeout(() => { if (notice === currentNotice) notice = ''; }, 2000);
    return () => clearTimeout(timer);
  });
  type Creation = 'rename-workspace' | 'rename-folder' | 'workspace' | 'collection' | 'folder' | 'request' | 'rename' | 'duplicate' | 'save-request';
  let creation = $state<Creation | null>(null);
  const workspaceCreation = $derived(creation === 'workspace' || creation === 'rename-workspace');
  let workspaceActionId = 1;
  let workspaceMenuTrigger = $state<HTMLButtonElement | null>(null);
  let manageWorkspacesOpen = $state(false);
  let WorkspaceManager = $state<typeof import('$lib/features/workspaces/WorkspaceManager.svelte').default | null>(null);
  let deletingWorkspace = $state<{ id: number; name: string } | null>(null);
  let deletingRequest = $state<{ root: string; path: string; name: string; revision: number } | null>(null);
  const deletingRequestEdited = $derived(requestTabs.some(item => item.root === deletingRequest?.root && item.path === deletingRequest?.path && (item.id === activeRequestTab ? !!draft?.edits.length : !!item.draft.edits.length)));
  let deletionError = $state('');
  const titles = { 'rename-workspace': 'Rename workspace', 'rename-folder': 'Rename folder', workspace: 'New workspace', collection: 'New collection', folder: 'New folder', request: 'New request', rename: 'Rename request', duplicate: 'Duplicate request', 'save-request': 'Save request' };
  let saveRoot = $state('');
  let creatingSaveCollection = false;
  let pendingSaveName = '';
  const saveCollections = $derived((workspace?.collections ?? []).filter(item => !item.readOnly && !item.warning));
  const saveCollection = $derived(saveCollections.find(item => item.root === saveRoot));
  let creationFolder = $state('');
  let creationSource = $state('');
  let creationRevision = $state(0);
  let menuRoot = $state('');
  let menuPath = $state('');
  let menuFolder = $state(false);
  let menuCollection = $state(false);
  const menuCannotCreate = $derived(workspace?.collections.some(owner => owner.root === menuRoot && (owner.readOnly || !!owner.warning)) ?? true);
  let menuInvoker: HTMLElement | null = null;
  let creationReturnFocus: HTMLElement | null = null;
  const treeFocusTargets = new WeakMap<HTMLElement, { root: string; path: string; kind: TreeKind; action: boolean }>();
  function rememberTreeFocus(target: HTMLElement | null) {
    const row = target?.closest<HTMLElement>('[data-tree-root]');
    if (target && row?.dataset.treeRoot) treeFocusTargets.set(target, {
      root: row.dataset.treeRoot, path: row.dataset.treePath ?? '',
      kind: row.dataset.treeCollection === 'true' ? 'collection' : row.dataset.treeFolder === 'true' ? 'folder' : target.classList.contains('tree-create') ? 'empty' : 'request',
      action: target.classList.contains('tree-action'),
    });
    return target;
  }
  let creationName = $state('');
  let creationFile = $state('');
  let creationMethod = $state('GET');
  const creationMethods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'].map(value => ({ value, label: value }));
  let creationUrl = $state('');
  let creationError = $state('');
  $effect(() => {
    if (!recoveryReady || recoveryBlocked || !workspace || loading || closingWindow) return;
    const snapshot = captureRecovery(workspace.activeWorkspaceId, requestTabs, activeRequestTab, draft, environment, home);
    const timer = setTimeout(() => { void persistRecovery(snapshot); }, 400);
    return () => clearTimeout(timer);
  });
  onMount(() => { void refreshHistory(); if (!isTauri()) void restoreWorkspace(); });
  onMount(() => {
    if (!isTauri()) return;
    const focus = () => queueExternalRefresh();
    window.addEventListener('focus', focus);
    const listener = listen<CollectionChange>('collection-changed', ({ payload }) => {
      lastWatchWarning = payload.warning ? `File notifications failed: ${payload.warning}. Use Refresh collections to retry.` : '';
      watchWarning = lastWatchWarning;
      queueExternalRefresh(payload.roots);
    }).catch(e => { if (!externalDisposed) { lastWatchWarning = `File notifications are unavailable: ${String(e)}. Use Refresh collections.`; watchWarning = lastWatchWarning; } });
    void listener.then(() => { if (!externalDisposed) void restoreWorkspace(); });
    return () => { externalDisposed = true; clearTimeout(externalTimer); window.removeEventListener('focus', focus); void listener.then(unlisten => unlisten?.()); };
  });
  onMount(() => {
    if (!isTauri()) return;
    const listener = getCurrentWindow().onCloseRequested(async event => {
      if (busy || loading || chunkLoading) { event.preventDefault(); error = 'Wait for the current operation before closing this window.'; return; }
      if (envDraft?.edits.length) { event.preventDefault(); error = 'Save or Discard your environment changes before closing this window.'; return; }
      finishRequestName();
      closingWindow = true; loading = true;
      const saved = !!workspace && await persistRecovery(captureRecovery(workspace.activeWorkspaceId, requestTabs, activeRequestTab, draft, environment, home));
      if (!saved && (unsavedTabs || isEdited)) { event.preventDefault(); closingWindow = false; loading = false; }
    });
    return () => { void listener.then(unlisten => unlisten()); };
  });
  function modal(node: HTMLDialogElement) {
    node.showModal();
    return { destroy() {
      if (node.open) node.close();
      if (node.classList.contains('creation-modal')) {
        void restoreTreeFocus(creationReturnFocus).then(restored => {
          if (!restored) document.querySelector<HTMLElement>('.sidebar-create button')?.focus();
        });
      }
    } };
  }

  async function refreshHistory() { try { history = await api.history(); } catch (e) { history.warning = String(e); } }
  function queueExternalRefresh(roots = workspace?.collections.map(owner => owner.root) ?? []) {
    if (externalDisposed || closingWindow) return;
    for (const root of roots) pendingExternalRoots.add(root);
    if (!pendingExternalRoots.size) return;
    clearTimeout(externalTimer);
    externalTimer = setTimeout(() => { void refreshExternalCollections(); }, 300);
  }
  async function refreshExternalCollections() {
    if (externalDisposed || closingWindow) return;
    // Defer batches while a user operation or dialog owns the current revisions.
    if (busy || loading || chunkLoading || creation || deletingRequest || deletingWorkspace || closingRequestTab !== null || environmentOpen || addingEnvironment || requestNameEdit !== null) { externalTimer = setTimeout(() => { void refreshExternalCollections(); }, 500); return; }
    if (!workspace) return;
    const roots = new Set(pendingExternalRoots); pendingExternalRoots.clear();
    if (!roots.size) return;
    const workspaceId = workspace.activeWorkspaceId;
    loading = true; stashRequestTab();
    try {
      const result = await api.refreshCollections();
      if (externalDisposed || workspace.activeWorkspaceId !== workspaceId || result.activeWorkspaceId !== workspaceId) return;
      watchWarning = result.watchWarning || lastWatchWarning;
      for (const root of roots) { delete requestSummaries[root]; summaryAttempts.delete(root); }
      workspace.collections = result.collections;
      if (collection) collection = result.collections.find(owner => owner.root === collection?.root) ?? collection;
      for (const item of requestTabs) {
        if (!item.root || !item.path || !roots.has(item.root)) continue;
        const owner = result.collections.find(owner => owner.root === item.root);
        let updated = false;
        if (!owner || (!owner.warning && !owner.requests.includes(item.path))) {
          recoverAsUntitled(item, 'The original file was removed or renamed outside NimblePost.'); updated = true;
        } else if (owner.warning) {
          item.externalChange = 'The collection is unavailable. Your request is preserved; Save copy to choose another location.';
        } else {
          try { updated = refreshExternalTab(item, await api.inspectRequest(item.root, item.path, item.draft.revision)); }
          catch { item.externalChange = 'The original file could not be read. Your request is preserved. Reload from disk after fixing it, or Save copy.'; }
        }
        if (externalDisposed) return;
        if (item.id === activeRequestTab && updated) {
          selected = item.path; draft = item.draft;
          requestName = draft.value.info?.name ?? 'Untitled'; method = draft.value.http?.method ?? 'GET'; url = draft.value.http?.url ?? '';
          response = null; body = ''; offset = 0;
          if (item.path) {
            try { accept(await api.request(item.path)); notice = 'Reloaded from disk.'; }
            catch { item.externalChange = 'The file changed again before it could be reopened. Your request is preserved; Refresh collections to retry.'; }
          } else notice = item.recoveryNotice ?? '';
        }
      }
      if (environment && collection && !collection.warning && !collection.environments.includes(environment)) {
        environment = ''; secrets = {}; secretNames = []; envDraft = null;
      }
    } catch (e) {
      watchWarning = `External changes could not be loaded: ${String(e)}. Use Refresh collections to retry.`;
    } finally {
      loading = false;
      if (pendingExternalRoots.size && !externalDisposed) queueExternalRefresh([...pendingExternalRoots]);
    }
  }
  async function saveExternalCopy() {
    if (busy || loading) return;
    stashRequestTab();
    const item = requestTabs.find(item => item.id === activeRequestTab);
    if (!item || !draft) return;
    recoverAsUntitled(item, 'The original file remains unchanged.');
    selected = ''; draft = item.draft; notice = item.recoveryNotice ?? ''; error = '';
    await saveRequest();
  }
  async function persistRecovery(snapshot: RecoverySnapshot) {
    if (recoveryBlocked) return false;
    // Retain only the in-flight backup and the latest queued snapshot.
    recoveryPending = snapshot;
    recoveryWrite ??= Promise.resolve().then(async () => {
      let saved = false;
      while (recoveryPending) {
        const next = recoveryPending; recoveryPending = null;
        try { await api.saveRecovery(next); recoveryWarning = ''; saved = true; }
        catch (e) { recoveryWarning = `Tabs and drafts could not be backed up: ${String(e)}. Keep this window open; editing a request or closing the window will retry.`; saved = false; }
      }
      recoveryWrite = null;
      return saved;
    });
    return recoveryWrite;
  }
  function canLeave() { if (unsavedTabs || isEdited || envDraft?.edits.length) { error = 'Save or close your unsaved request tabs before switching workspaces or collections.'; return false; } return true; }
  function stashRequestTab() {
    const item = requestTabs.find(item => item.id === activeRequestTab);
    if (item && draft) Object.assign(item, { draft, environment, baseUrl, response, body, offset });
  }
  async function newUntitled() {
    if (busy || loading || chunkLoading || envDraft?.edits.length) return;
    stashRequestTab();
    draft = untitledDraft(); selected = ''; requestName = 'Untitled'; method = 'GET'; url = '';
    response = null; body = ''; offset = 0; error = ''; notice = ''; home = false; manageWorkspacesOpen = false; environmentOpen = false; envDraft = null;
    activeRequestTab = ++nextRequestTab;
    requestTabs.push({ id: activeRequestTab, root: collection?.root ?? null, path: '', source: null, draft, environment, baseUrl, response: null, body: '', offset: 0 });
    await tick(); document.querySelector<HTMLInputElement>('input[aria-label="Request URL"]')?.focus();
  }
  async function activateRequestTab(id: number, internal = false) {
    if (busy || (loading && !internal) || chunkLoading || envDraft?.edits.length || id === activeRequestTab) return;
    stashRequestTab();
    const item = requestTabs.find(item => item.id === id);
    if (!item) return;
    loading = true; error = '';
    const previousRoot = collection?.root;
    try {
      if (item.root && item.root !== previousRoot) {
        if (!internal && requestTabs.some(tab => tab.path && tab.draft.edits.length)) throw new Error('Save or Discard edited collection requests before switching collections.');
        updateCollection(await api.selectCollection(item.root));
      }
      if (item.path && !item.externalChange) {
        try {
          const fresh = await api.request(item.path);
          try { item.draft = refreshTabDraft(item, fresh); item.source = fresh; item.restored = false; }
          catch (e) { if (item.restored) recoverAsUntitled(item, 'The original file changed while NimblePost was closed.'); else error = String(e); }
        } catch (e) { if (item.restored) recoverAsUntitled(item, 'The original file could not be opened.'); else throw e; }
      }
      activeRequestTab = id; selected = item.path; draft = item.draft;
      requestName = draft.value.info?.name ?? 'Untitled'; method = draft.value.http?.method ?? 'GET'; url = draft.value.http?.url ?? '';
      environment = collection?.environments.includes(item.environment) ? item.environment : ''; baseUrl = item.baseUrl;
      response = item.response; body = item.body; offset = item.offset; decoder = new TextDecoder(); notice = item.recoveryNotice ?? ''; home = false;
      environmentOpen = false;
      await environmentChanged();
    } catch (e) {
      error = String(e);
      if (previousRoot && previousRoot !== collection?.root) updateCollection(await api.selectCollection(previousRoot));
    } finally { loading = false; }
  }
  async function closeRequestTab(id: number) {
    if (busy || loading || chunkLoading) return;
    stashRequestTab();
    const item = requestTabs.find(item => item.id === id);
    if (!item) return;
    if (item.draft.edits.length) {
      creationReturnFocus = rememberTreeFocus(document.activeElement instanceof HTMLElement ? document.activeElement : null);
      closeTabError = ''; closingRequestTab = id; return;
    }
    await removeRequestTab(id);
  }
  async function removeRequestTab(id: number) {
    const responseId = id === activeRequestTab ? response?.id : requestTabs.find(item => item.id === id)?.response?.id;
    requestTabs = requestTabs.filter(item => item.id !== id);
    const released = responseId === undefined ? Promise.resolve() : api.releaseResponse(responseId).catch(e => console.warn('Closed response could not be released.', e));
    if (id === activeRequestTab) {
      draft = null; selected = ''; response = null; body = ''; offset = 0; error = ''; notice = ''; activeRequestTab = 0;
      const next = requestTabs.at(-1);
      if (next) await activateRequestTab(next.id);
    }
    await released;
  }
  async function saveClosingTab() {
    const id = closingRequestTab;
    if (id === null || busy || loading || chunkLoading) return;
    closeTabError = '';
    if (id !== activeRequestTab) {
      await activateRequestTab(id);
      if (activeRequestTab !== id || error) { closeTabError = error || 'This request could not be opened for saving.'; return; }
    }
    if (untitled) {
      closingRequestTab = null; closeAfterSave = id;
      await saveRequest();
    } else if (await saveRequest()) {
      closingRequestTab = null; await removeRequestTab(id);
    } else closeTabError = error;
  }
  async function discardClosingTab() {
    const id = closingRequestTab;
    if (id === null || busy || loading) return;
    closingRequestTab = null; await removeRequestTab(id);
  }
  function cancelCreation() { creation = null; closeAfterSave = null; creatingSaveCollection = false; }
  function accept(view: RequestInfo) {
    draft = draftFrom(view); requestName = view.name; method = view.method; url = view.url;
    const existing = requestTabs.find(item => item.id === activeRequestTab);
    if (existing) Object.assign(existing, { root: collection?.root ?? null, path: selected, source: view, draft, restored: false, recoveryNotice: '', externalChange: '' });
    else if (selected) {
      activeRequestTab = ++nextRequestTab;
      requestTabs.push({ id: activeRequestTab, root: collection?.root ?? null, path: selected, source: view, draft, environment, baseUrl, response: null, body: '', offset: 0 });
    }
    if (collection && selected) {
      requestSummaries[collection.root] ??= {};
      requestSummaries[collection.root][selected] = { name: view.name, method: view.method.toUpperCase() };
      const attempts = summaryAttempts.get(collection.root) ?? new Set<string>();
      summaryAttempts.set(collection.root, attempts);
      attempts.add(selected);
      pruneSummaries(collection.root, attempts);
    }
  }
  function change(path: string[], value: unknown) {
    if (!draft) return;
    editDraft(draft, path, value); requestName = draft.value.info?.name ?? requestName; method = draft.value.http?.method ?? method; url = draft.value.http?.url ?? url; notice = '';
  }
  async function editRequestName() {
    if (!draft || busy || loading || (!untitled && collection?.readOnly)) return;
    requestNameEdit = requestName;
    await tick();
    const input = document.querySelector<HTMLInputElement>('.request-name-input');
    input?.focus(); input?.select();
  }
  function finishRequestName() {
    const name = requestNameEdit?.trim();
    requestNameEdit = null;
    if (name && name !== requestName) change(['info', 'name'], name);
  }
  function requestNameKey(event: KeyboardEvent) {
    if (event.key !== 'Enter' && event.key !== 'Escape') return;
    event.preventDefault(); event.stopPropagation();
    if (event.key === 'Enter') finishRequestName(); else requestNameEdit = null;
    void tick().then(() => document.querySelector<HTMLButtonElement>('.request-name-trigger')?.focus());
  }
  async function saveRequest() {
    if (!draft || busy || loading) return false;
    if (activeExternalChange) { error = activeExternalChange; return false; }
    finishRequestName();
    if (untitled) {
      creatingSaveCollection = false;
      stashRequestTab(); creationReturnFocus = rememberTreeFocus(document.activeElement instanceof HTMLElement ? document.activeElement : null);
      creationName = requestName === 'Untitled' ? '' : requestName; creationError = ''; saveRoot = saveCollections.find(item => item.root === collection?.root)?.root ?? saveCollections[0]?.root ?? '';
      creationFolder = ''; creation = 'save-request'; return false;
    }
    loading = true; error = ''; notice = '';
    try { accept(await api.save(draft.revision, draft.edits)); notice = 'Request saved.'; return true; }
    catch (e) { error = String(e); return false; }
    finally { loading = false; }
  }
  async function discard() {
    if (busy || loading) return;
    if (untitled) { draft = untitledDraft(); requestName = 'Untitled'; method = 'GET'; url = ''; error = ''; notice = ''; stashRequestTab(); return; }
    if (selected) { loading = true; try { accept(await api.request(selected)); bodyEditorReset++; error = ''; notice = 'Reloaded from disk.'; } catch (e) { error = String(e); } finally { loading = false; } }
  }
  async function environmentChanged(next = environment) {
    secrets = {}; secretNames = [];
    if (!environmentOpen) envDraft = null;
    const wasLoading = loading; loading = true;
    try {
      const view = next ? await api.environment(next) : null;
      environment = next; secretNames = (view?.document.variables ?? []).filter((row: { secret?: boolean; disabled?: boolean }) => row.secret && !row.disabled).map((row: { name: string }) => row.name);
      envDraft = environmentOpen && view ? draftFrom(view) : null;
    }
    catch (e) { error = String(e); }
    finally { loading = wasLoading; }
  }
  async function manageEnvironment(create = false) {
    if (busy || loading || !canLeaveEnvironment()) return;
    error = '';
    if (create) { newEnvironment = ''; addingEnvironment = true; return; }
    loading = true; error = '';
    try {
      EnvironmentManager ??= (await import('$lib/features/environments/EnvironmentManager.svelte')).default;
      envDraft = environment ? draftFrom(await api.environment(environment)) : null;
      manageWorkspacesOpen = false; environmentOpen = true;
    }
    catch (e) { error = String(e); }
    finally { loading = false; }
    await tick();
    (environmentOpen ? document.querySelector<HTMLElement>('#environment-manager-heading') : environmentMenuTrigger)?.focus();
  }
  function canLeaveEnvironment() {
    if (envDraft?.edits.length) { error = 'Save or Discard your environment changes before leaving or switching environments.'; return false; }
    return true;
  }
  async function chooseEnvironment(name: string) {
    if (busy || loading || name === environment || !canLeaveEnvironment()) return;
    error = '';
    await environmentChanged(name);
  }
  async function createEnvironment() {
    if (busy || loading || !newEnvironment.trim() || !collection || collection.readOnly) return;
    loading = true; error = '';
    try { const view = await api.createEnvironment(newEnvironment.trim()); envDraft = draftFrom(view); environment = view.name; collection.environments = [...collection.environments, view.name].sort(); newEnvironment = ''; secrets = {}; secretNames = []; addingEnvironment = false; }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function saveEnvironment() {
    if (!envDraft || busy || loading) return;
    loading = true; error = '';
    try {
      const old = environment; const view = await api.save(envDraft.revision, envDraft.edits);
      envDraft = draftFrom(view); environment = view.name; collection!.environments = collection!.environments.map(name => name === old ? view.name : name).sort();
      secretNames = (view.document.variables ?? []).filter((row: { secret?: boolean; disabled?: boolean }) => row.secret && !row.disabled).map((row: { name: string }) => row.name);
      secrets = {}; notice = 'Environment saved.';
    } catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function closeEnvironment() {
    if (busy || loading || !canLeaveEnvironment()) return;
    environmentOpen = false; error = '';
    await tick(); environmentMenuTrigger?.focus();
  }
  function closeEnvironmentCreation() { if (!loading) { addingEnvironment = false; error = ''; } }
  async function discardEnvironment() {
    if (busy || loading) return;
    loading = true; error = '';
    try { envDraft = environment ? draftFrom(await api.environment(environment)) : null; }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function clearHistory() { try { await api.clearHistory(); await refreshHistory(); } catch (e) { history.warning = String(e); } }

  function beginCreate(kind: 'workspace' | 'collection' | 'folder' | 'request', folder = '') {
    if (busy || loading || !canLeave() || (kind !== 'collection' && kind !== 'workspace' && (!collection || collection.readOnly))) return;
    creationReturnFocus = kind === 'workspace' ? workspaceMenuTrigger : document.activeElement instanceof HTMLElement ? (document.activeElement.closest('[role="menu"]') ? menuInvoker : rememberTreeFocus(document.activeElement)) : null;
    creatingSaveCollection = false;
    creationName = ''; creationFile = ''; creationMethod = 'GET'; creationUrl = ''; creationError = ''; creationFolder = folder; creationSource = ''; creation = kind;
  }
  async function createSaveCollection() {
    if (requestTabs.some(item => item.path && item.draft.edits.length)) { creationError = 'Save or Discard edited collection requests before creating another collection.'; return; }
    pendingSaveName = creationName; creatingSaveCollection = true;
    creationName = ''; creationFile = ''; creationError = ''; creation = 'collection';
    await tick(); document.querySelector<HTMLInputElement>('dialog input[aria-label="Collection name"]')?.focus();
  }
  async function beginRequestAction(kind: 'rename' | 'duplicate', path: string, root = collection?.root) {
    if (root !== collection?.root && !await chooseCollection(root!)) return;
    if (busy || loading || !canLeave() || !collection || collection.readOnly) return;
    creationReturnFocus = menuInvoker;
    loading = true; error = '';
    try {
      const view = path === selected && draft ? { revision: draft.revision, name: requestName } : await api.request(path);
      creationSource = path; creationRevision = view.revision; creationFolder = path.split('/').slice(0, -1).join('/');
      creationName = kind === 'duplicate' ? `${view.name.slice(0, 123)} copy` : view.name;
      const stem = path.split('/').pop()!.replace(/\.ya?ml$/, '');
      creationFile = kind === 'duplicate' ? `${stem.slice(0, 59)}-copy` : stem;
      creationError = ''; creation = kind;
    } catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function beginFolderRename(root: string, folder: string) {
    if (root !== collection?.root && !await chooseCollection(root)) return;
    if (busy || loading || !canLeave() || !collection || collection.readOnly) return;
    creationReturnFocus = menuInvoker;
    creationSource = folder; creationName = folder.split('/').pop()!;
    creationError = ''; creation = 'rename-folder';
  }
  async function beginRequestDelete(path: string, root: string) {
    if (busy || loading || chunkLoading) return;
    if (root !== collection?.root && !await chooseCollection(root)) return;
    if (!collection || collection.readOnly || collection.warning) return;
    creationReturnFocus = menuInvoker;
    loading = true; error = ''; deletionError = '';
    try {
      const view = path === selected && draft ? { revision: draft.revision, name: requestName } : await api.request(path);
      deletingRequest = { root, path, name: view.name, revision: view.revision };
    } catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function deleteRequest() {
    const target = deletingRequest;
    if (!target || busy || loading || chunkLoading || target.root !== collection?.root) return;
    loading = true; deletionError = '';
    try {
      updateCollection(await api.deleteRequest(target.revision));
      deletingRequest = null;
    } catch (e) { deletionError = String(e); }
    finally { loading = false; }
    if (!deletingRequest) {
      const tab = requestTabs.find(item => item.root === target.root && item.path === target.path);
      if (tab) await removeRequestTab(tab.id);
      notice = 'Request deleted.';
    }
  }
  function treeMenuTarget(event: MouseEvent | PointerEvent) {
    if (busy || loading) { event.preventDefault(); return; }
    const row = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-tree-path]') : null;
    if (!row) { event.preventDefault(); return; }
    menuInvoker = rememberTreeFocus(event.target instanceof Element ? event.target.closest<HTMLElement>('button') ?? row.querySelector('button') : null);
    const owner = workspace?.collections.find(collection => collection.root === row.dataset.treeRoot);
    if (!owner || ((owner.readOnly || owner.warning) && row.dataset.treeCollection !== 'true')) { event.preventDefault(); return; }
    menuRoot = owner.root; menuPath = row.dataset.treePath!; menuFolder = row.dataset.treeFolder === 'true'; menuCollection = row.dataset.treeCollection === 'true';
  }
  async function restoreTreeFocus(target: HTMLElement | null) {
    const previous = target && treeFocusTargets.get(target);
    if (!previous) { target?.focus(); return !!target?.isConnected; }
    const row = await treeView?.reveal(previous.root, previous.path, previous.kind);
    const button = row?.querySelector<HTMLElement>(previous.action ? '.tree-action' : 'button');
    button?.focus();
    return !!button;
  }
  function showTreeMenu(event: MouseEvent) {
    event.preventDefault(); event.stopPropagation();
    if (busy || loading) return;
    const button = event.currentTarget as HTMLElement;
    const rect = button.getBoundingClientRect();
    button.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rect.right, clientY: rect.bottom }));
  }
  function creationNameChanged(name: string) {
    creationName = name;
    if (creation === 'collection' || creation === 'folder') creationFile = name.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 64);
  }
  function updateCollection(result: Collection) {
    const previous = workspace?.collections.find(owner => owner.root === result.root);
    if (previous && !samePaths(result.requests, previous.requests)) {
      summaryAttempts.delete(result.root);
      const retained = new Set(result.requests);
      for (const path of Object.keys(requestSummaries[result.root] ?? {})) if (!retained.has(path)) delete requestSummaries[result.root][path];
    }
    collection = result;
    if (workspace) {
      workspace.collections = workspace.collections.some(item => item.root === result.root) ? workspace.collections.map(item => item.root === result.root ? result : item) : [...workspace.collections, result];
      workspace.activeCollection = result.root;
    }
  }
  function goHome() {
    if (busy || loading || !canLeave()) return;
    manageWorkspacesOpen = false; environmentOpen = false; home = true; error = ''; notice = '';
  }
  async function applyWorkspace(result: WorkspaceView, recovered: RecoverySnapshot | null = null) {
    const renderStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
    requestSummaries = {};
    summaryAttempts.clear(); visibleSummaryRequests = [];
    requestTabs = []; activeRequestTab = 0;
    workspace = result; collection = null; selected = ''; draft = null; envDraft = null; environmentOpen = false; addingEnvironment = false; environment = ''; secretNames = []; secrets = {}; response = null; body = ''; baseUrl = ''; notice = '';
    if (import.meta.env.VITE_NATIVE_VALIDATION === '1') void tick().then(() => recordStage('workspace-dom-flush', renderStarted, result.collections.reduce((count, owner) => count + owner.requests.length, 0)));
    watchWarning = result.watchWarning || lastWatchWarning;
    const active = result.collections.find(item => item.root === result.activeCollection && !item.warning);
    if (recovered?.workspaceId === result.activeWorkspaceId) {
      if (active) updateCollection(active);
      requestTabs = restoreRecovery(recovered, result.collections);
      nextRequestTab = Math.max(nextRequestTab, ...requestTabs.map(item => item.id));
      const target = requestTabs.find(item => item.id === recovered.activeTab) ?? requestTabs.at(-1);
      if (target) await activateRequestTab(target.id, true);
      home = recovered.home;
    } else if (active) await useCollection(active);
  }
  async function restoreWorkspace() {
    loading = true;
    try {
      const readStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
      const [result, recovered] = await Promise.all([api.workspace(), api.recovery().catch(() => ({ snapshot: null, warning: 'Recovery could not be read' }))]);
      if (import.meta.env.VITE_NATIVE_VALIDATION === '1') recordStage('workspace-ipc', readStarted);
      recoveryBlocked = !!recovered.warning;
      recoveryWarning = recovered.warning ? 'The recovery file could not be loaded and has been preserved. Repair the file and restart NimblePost to back up tabs and drafts again.' : '';
      const applyStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
      await applyWorkspace(result, recovered.snapshot);
      if (import.meta.env.VITE_NATIVE_VALIDATION === '1') recordStage('restore-apply', applyStarted);
      recoveryReady = true;
    }
    catch (e) { error = String(e); }
    finally {
      const renderStarted = import.meta.env.VITE_NATIVE_VALIDATION === '1' ? performance.now() : 0;
      loading = false;
      if (isTauri() && import.meta.env.VITE_NATIVE_VALIDATION === '1') {
        void import('$lib/desktop/native-validation').then(({ validateNative }) => validateNative({
          error: error || recoveryWarning || watchWarning || workspace?.warning || '',
          requests: workspace?.collections.reduce((count, item) => count + item.requests.length, 0) ?? 0,
          tabs: requestTabs.length,
          untitled,
        }, renderStarted, workspace?.validationPlan ? {
          url: workspace.validationPlan.url,
          treeOnly: workspace.validationPlan.treeOnly,
          responseOnly: workspace.validationPlan.responseOnly,
          heapInspector: workspace.validationPlan.heapInspector,
          hideResponseView: (hidden: boolean) => { validationHideResponseView = hidden; },
          hiddenResponseView: workspace.validationPlan.hideResponseView,
          open: select,
          activate: activateRequestTab,
          close: async (id: number) => { await closeRequestTab(id); if (closingRequestTab === id) await discardClosingTab(); },
          send: async (address: string) => { change(['http', 'url'], address); await send(); if (error || !response) throw new Error(error || 'No response'); },
          more: loadMore,
          reset: () => changeWorkspace(2),
          inspect: () => ({
            tabs: requestTabs.map(item => item.id),
            activeBodyChars: body.length, activeOffset: offset, bodyBytes: response?.bodyBytes ?? 0,
            cachedBodyChars: requestTabs.reduce((sum, item) => sum + item.body.length, 0),
            activeCachedBodyChars: requestTabs.find(item => item.id === activeRequestTab)?.body.length ?? 0,
            labels: Object.values(requestSummaries).reduce((sum, summaries) => sum + Object.keys(summaries).length, 0),
            attempts: [...summaryAttempts.values()].reduce((sum, paths) => sum + paths.size, 0),
            treeCaches: treeCache.size, error, recoveryWarning,
          }),
        } : undefined));
      }
    }
  }
  async function changeWorkspace(id: number) {
    if (id === workspace?.activeWorkspaceId || busy || loading || !canLeave()) return;
    loading = true; error = '';
    try { await applyWorkspace(await api.selectWorkspace(id)); manageWorkspacesOpen = false; }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function openWorkspaceManager() {
    if (busy || loading || !canLeaveEnvironment()) return;
    error = '';
    if (!WorkspaceManager) {
      loading = true;
      try { WorkspaceManager = (await import('$lib/features/workspaces/WorkspaceManager.svelte')).default; }
      catch (e) { error = 'Workspace management could not be opened.'; console.warn('Workspace manager could not be loaded.', e); }
      finally { loading = false; }
      if (!WorkspaceManager) { await tick(); workspaceMenuTrigger?.focus(); return; }
    }
    environmentOpen = false; manageWorkspacesOpen = true;
    await tick(); document.querySelector<HTMLElement>('#workspace-manager-heading')?.focus();
  }
  async function closeWorkspaceManager() {
    manageWorkspacesOpen = false; error = '';
    await tick();
    (workbenchOpen ? document.querySelector<HTMLElement>('[aria-label="Request URL"]') : workspaceMenuTrigger)?.focus();
  }
  function restoreWorkspaceManagerFocus() {
    (creationReturnFocus?.isConnected ? creationReturnFocus : document.querySelector<HTMLElement>('#workspace-manager-heading'))?.focus();
  }
  function beginWorkspaceAction(kind: 'rename' | 'delete', id: number) {
    if (busy || loading || !canLeave()) return;
    const target = workspace?.workspaces.find(item => item.id === id);
    if (!target) return;
    workspaceActionId = target.id;
    creationReturnFocus = document.querySelector<HTMLElement>(`[data-workspace-id="${id}"] [data-workspace-action="${kind}"]`) ?? workspaceMenuTrigger;
    if (kind === 'rename') {
      creationName = target.name; creationFile = ''; creationError = ''; creation = 'rename-workspace';
    } else { deletionError = ''; deletingWorkspace = target; }
  }
  async function deleteWorkspace() {
    if (!deletingWorkspace || busy || loading || !canLeave()) return;
    loading = true; deletionError = '';
    try {
      const id = deletingWorkspace.id;
      const result = await api.removeWorkspace(id);
      if (workspace?.activeWorkspaceId === id) await applyWorkspace(result); else workspace = result;
      deletingWorkspace = null;
    } catch (e) { deletionError = String(e); }
    finally { loading = false; }
  }
  async function chooseCollection(root: string, path?: string) {
    if (busy || loading || !canLeaveEnvironment()) return false;
    if (collection?.root === root) { home = false; manageWorkspacesOpen = false; environmentOpen = false; envDraft = null; if (path) await select(path); return true; }
    if (!canLeave()) return false;
    loading = true; error = '';
    try { await useCollection(await api.selectCollection(root), false, path); manageWorkspacesOpen = false; return true; }
    catch (e) { error = String(e); return false; }
    finally { loading = false; }
  }
  async function createIn(root: string, kind: 'request' | 'folder', folder = '') {
    if (root !== collection?.root && !await chooseCollection(root)) return;
    beginCreate(kind, folder);
  }
  async function removeCollection(root: string) {
    if (busy || loading || !canLeave()) return;
    loading = true; error = '';
    try { await applyWorkspace(await api.removeCollection(root)); }
    catch (e) { error = String(e); }
    finally {
      loading = false;
      await tick();
      if (!await restoreTreeFocus(menuInvoker)) document.querySelector<HTMLElement>('[aria-label="New collection"]')?.focus();
    }
  }
  async function useCollection(result: Collection, example = false, path?: string) {
    environmentOpen = false; envDraft = null;
    requestTabs = []; activeRequestTab = 0;
    home = false; updateCollection(result); selected = ''; response = null; body = ''; baseUrl = ''; draft = null; secrets = {}; notice = '';
    environment = result.environments.includes('local') ? 'local' : '';
    await environmentChanged();
    if (result.requests.length) await select(path ?? (example ? (result.requests.find(path => path.endsWith('/list.yml')) ?? result.requests[0]) : result.requests[0]), true);
  }
  async function create() {
    if (!creation || busy || loading || (creation !== 'save-request' && !creatingSaveCollection && !canLeave())) return;
    loading = true; creationError = ''; error = '';
    try {
      if (creation === 'save-request') {
        if (!draft || !saveCollection) throw new Error('Choose a writable collection to save this request.');
        if (saveRoot !== collection?.root) {
          if (requestTabs.some(item => item.path && item.draft.edits.length)) throw new Error('Save or Discard edited collection requests before choosing another collection.');
          updateCollection(await api.selectCollection(saveRoot));
          environment = ''; secrets = {}; secretNames = [];
        }
        const result = await api.createRequest({ name: creationName.trim(), folder: creationFolder, document: draft.value });
        updateCollection(result.collection); selected = result.path; accept(result.request); notice = 'Request saved.';
      } else if (creation === 'rename-workspace') {
        workspace = await api.renameWorkspace(workspaceActionId, creationName.trim());
        notice = 'Workspace renamed.';
      } else if (creation === 'workspace') {
        await applyWorkspace(await api.createWorkspace(creationName.trim()));
      } else if (creation === 'collection') {
        const result = await api.createCollection(creationName.trim(), creationFile);
        if (!result) return;
        if (creatingSaveCollection) {
          updateCollection(result); saveRoot = result.root; creationFolder = ''; creationName = pendingSaveName;
          creatingSaveCollection = false; creation = 'save-request'; return;
        }
        await useCollection(result); manageWorkspacesOpen = false;
      } else if (creation === 'folder') {
        updateCollection(await api.createFolder(creationFolder, creationName.trim(), creationFile));
        notice = 'Folder created.';
      } else if (creation === 'rename-folder') {
        const destination = [...creationSource.split('/').slice(0, -1), creationName.trim()].join('/');
        const movedSelection = selected.startsWith(`${creationSource}/`) ? destination + selected.slice(creationSource.length) : null;
        updateCollection(await api.renameFolder(creationSource, creationName.trim()));
        requestTabs = requestTabs.filter(item => item.root !== collection?.root || !item.path.startsWith(`${creationSource}/`));
        if (movedSelection) { activeRequestTab = 0; await select(movedSelection, true); }
        notice = 'Folder renamed.';
        await tick();
        creationReturnFocus = rememberTreeFocus((await treeView?.reveal(collection!.root, destination, 'folder'))?.querySelector('button') ?? null);
      } else {
        const input = { revision: creationRevision, name: creationName.trim(), fileName: creationFile };
        const result = creation === 'rename' ? await api.renameRequest(input) : creation === 'duplicate' ? await api.duplicateRequest(input) : await api.createRequest({ name: input.name, folder: creationFolder, method: creationMethod, url: creationUrl.trim() });
        stashRequestTab();
        activeRequestTab = creation === 'rename' ? requestTabs.find(item => item.root === collection?.root && item.path === creationSource)?.id ?? 0 : 0;
        home = false; manageWorkspacesOpen = false; updateCollection(result.collection); selected = result.path; response = null; body = ''; offset = 0;
        accept(result.request); notice = creation === 'rename' ? 'Request renamed.' : creation === 'duplicate' ? 'Request duplicated.' : 'Request created and saved.';
      }
      creation = null;
    } catch (e) { creationError = String(e); }
    finally { loading = false; }
    if (!creation && closeAfterSave !== null) {
      const id = closeAfterSave; closeAfterSave = null;
      await removeRequestTab(id);
    }
  }

  async function open(example = false) {
    if (busy || loading) return;
    if (!canLeave()) return;
    loading = true; error = '';
    try {
      const result = example ? await api.example() : await api.open();
      if (result) { await useCollection(result, example); manageWorkspacesOpen = false; }
    } catch (e) { error = String(e); }
    finally { loading = false; }
  }

  async function select(path: string, internal = false) {
    if (busy || (loading && !internal) || chunkLoading || envDraft?.edits.length) return;
    environmentOpen = false; envDraft = null;
    const existing = requestTabs.find(item => item.root === collection?.root && item.path === path);
    if (existing) { await activateRequestTab(existing.id); return; }
    stashRequestTab();
    const version = ++selectionVersion;
    activeRequestTab = 0;
    selected = path; requestName = ''; response = null; body = ''; error = ''; loading = true;
    try {
      const result = await api.request(path);
      if (version !== selectionVersion) return;
      accept(result);
    } catch (e) { if (version === selectionVersion) { error = String(e); selected = ''; draft = null; activeRequestTab = 0; } }
    finally { if (version === selectionVersion) loading = false; }
  }

  async function send() {
    if (!draft || busy || loading || chunkLoading || environmentOpen || addingEnvironment || creation || deletingWorkspace || deletingRequest) return;
    if (activeExternalChange) { error = activeExternalChange; return; }
    busy = true; error = ''; response = null; body = ''; offset = 0; decoder = new TextDecoder();
    stashRequestTab();
    try {
      response = await api.send({ path: selected || 'Untitled', environment: environment || null, method, url, baseUrl, revision: untitled ? undefined : draft.revision, edits: untitled ? [] : draft.edits, secrets, ...(untitled ? {document:draft.value} : {}) });
      await loadMore();
    } catch (e) { error = String(e); }
    finally { busy = false; await refreshHistory(); }
  }

  async function loadMore() {
    if (!response || chunkLoading) return;
    const current = response;
    chunkLoading = true;
    try {
      const encoded = await api.chunk(current.id, offset);
      if (response?.id !== current.id) return;
      const bytes = Uint8Array.from(atob(encoded), character => character.charCodeAt(0));
      offset += bytes.length;
      if (current.text) body += decoder.decode(bytes, { stream: offset < current.bodyBytes });
      else body += Array.from(bytes).map(byte => byte.toString(16).padStart(2, '0')).join(' ') + '\n';
    } catch (e) { if (response?.id === current.id) error = String(e); }
    finally { chunkLoading = false; }
  }

  async function cancel() { try { await api.cancel(); } catch (e) { error = String(e); } }
  function shortcut(e: KeyboardEvent) { if (creation || addingEnvironment || deletingWorkspace || deletingRequest || manageWorkspacesOpen || closingRequestTab !== null) return; if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') { e.preventDefault(); void send(); } if ((e.metaKey || e.ctrlKey) && e.key === 's') { e.preventDefault(); if (environmentOpen) void saveEnvironment(); else void saveRequest(); } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
</script>

<svelte:window onkeydown={shortcut} bind:innerWidth={viewportWidth} />

<div class="app-shell" class:has-recovery-warning={!!recoveryWarning || !!watchWarning}>
  <header class="topbar" data-tauri-drag-region>
    <div class="workspace-picker"><Button variant="ghost" size="icon-sm" aria-label="Home" aria-current={home ? 'page' : undefined} title="Home" onclick={goHome} disabled={busy || loading}><House aria-hidden="true" /></Button><DropdownMenu.Root>
        <DropdownMenu.Trigger bind:ref={workspaceMenuTrigger} class="workspace-select" aria-label="Workspace" title="Switch workspace" disabled={busy || loading}><span class="workspace-name">{workspaceOptions.find(item => item.value === String(workspace?.activeWorkspaceId ?? 1))?.label ?? 'My Workspace'}</span><ChevronDown size={12} aria-hidden="true" /></DropdownMenu.Trigger>
        <DropdownMenu.Content class="tree-menu workspace-selector-menu motion-reduce:animate-none" align="start" sideOffset={6} onCloseAutoFocus={(event) => { if (creation || manageWorkspacesOpen) event.preventDefault(); }}>
          <div class="workspace-options"><DropdownMenu.RadioGroup bind:value={workspaceValue} onValueChange={async (value) => { await changeWorkspace(Number(value)); workspaceValue = String(workspace?.activeWorkspaceId ?? 1); }}>
            {#each workspaceOptions as item (item.value)}<DropdownMenu.RadioItem class="workspace-option" value={item.value} closeOnSelect={true} title={item.label}>{#snippet children()}<span class="workspace-option-name">{item.label}</span>{/snippet}</DropdownMenu.RadioItem>{/each}
          </DropdownMenu.RadioGroup></div>
          <DropdownMenu.Label class="workspace-menu-label">Workspaces</DropdownMenu.Label>
          <DropdownMenu.Item class="tree-menu-item" onSelect={() => beginCreate('workspace')}><Plus size={14} aria-hidden="true" />Create workspace</DropdownMenu.Item>
          <DropdownMenu.Item class="tree-menu-item" onSelect={openWorkspaceManager}><Settings2 size={14} aria-hidden="true" />Manage workspaces</DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Root></div>
    <div class="brand"><img class="brand-mark" src={theme === 'dark' ? brandDark : brandLight} width="18" height="18" alt="" /><span class="brand-wordmark">Nimble<span class="brand-post">Post</span></span><span class="alpha">ALPHA</span></div>
  </header>
  {#if recoveryWarning || watchWarning}<div>{#if recoveryWarning}<div class="error recovery-warning" role="alert">{recoveryWarning}</div>{/if}{#if watchWarning}<div class="error recovery-warning" role="alert">{watchWarning}</div>{/if}</div>{/if}
  <div class="workspace" class:resizing-sidebar={resizingSidebar} style:--sidebar-width={`${visibleSidebarWidth}px`}>
    <aside id="collection-sidebar">
      <div class="aside-heading"><span class="aside-title"><Folder size={14} aria-hidden="true" />Collections</span><div class="collection-actions"><Button variant="ghost" size="icon-sm" aria-label="Refresh collections" title="Refresh collections from disk" onclick={() => queueExternalRefresh()} disabled={busy || loading}><RotateCcw aria-hidden="true" /></Button><Button variant="ghost" size="icon-sm" aria-label="New collection" title="New collection" onclick={() => beginCreate('collection')} disabled={busy || loading}><FolderPlus aria-hidden="true" /></Button><Button variant="ghost" size="icon-sm" aria-label="Open collection" title="Open collection" onclick={() => open()} disabled={busy || loading}><FolderOpen aria-hidden="true" /></Button></div></div>
      <span class="sr-only" role="status">{busy ? 'Request in progress. Collections are temporarily unavailable.' : loading ? 'Loading. Collections are temporarily unavailable.' : ''}</span>
      {#if workspace?.warning}<p class="workspace-warning" role="alert">{workspace.warning}</p>{/if}
      {#if workspace?.collections.length}
        <ContextMenu.Root>
          <CollectionTree bind:this={treeView} collections={collectionTrees} summaries={requestSummaries} {summariesPending} onvisible={visibleRequests} selectedRoot={!home ? collection?.root ?? '' : ''} selectedPath={!home ? selected : ''} activeSummary={draft ? { name: requestName, method: method.toUpperCase() } : null} locked={busy || loading} onchoose={chooseCollection} oncreate={createIn} oncontextmenu={treeMenuTarget} onactions={showTreeMenu} />
          <ContextMenu.Content class="tree-menu" onCloseAutoFocus={(event) => { event.preventDefault(); if (!creation && !deletingRequest) void restoreTreeFocus(menuInvoker); }}>
            {#if menuCollection || menuFolder}
              <ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => createIn(menuRoot, 'request', menuPath)}><Plus size={14} aria-hidden="true" />New request</ContextMenu.Item>
              <ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => createIn(menuRoot, 'folder', menuPath)}><FolderPlus size={14} aria-hidden="true" />New folder</ContextMenu.Item>
              {#if menuFolder}<ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => beginFolderRename(menuRoot, menuPath)}><Pencil size={14} aria-hidden="true" />Rename</ContextMenu.Item>{/if}
              {#if menuCollection}
                <ContextMenu.Separator class="tree-menu-separator" />
                <ContextMenu.Item class="tree-menu-item" onSelect={() => treeView?.collapse(menuRoot)}><Folder size={14} aria-hidden="true" />Collapse</ContextMenu.Item>
                <ContextMenu.Separator class="tree-menu-separator" />
                <ContextMenu.Item class="tree-menu-item" title="Remove from workspace · files are kept" onSelect={() => removeCollection(menuRoot)}><X size={14} aria-hidden="true" />Remove from workspace</ContextMenu.Item>
              {/if}
            {:else}
              <ContextMenu.Item class="tree-menu-item" onSelect={() => beginRequestAction('rename', menuPath, menuRoot)}><Pencil size={14} aria-hidden="true" />Rename</ContextMenu.Item>
              <ContextMenu.Item class="tree-menu-item" onSelect={() => beginRequestAction('duplicate', menuPath, menuRoot)}><Copy size={14} aria-hidden="true" />Duplicate</ContextMenu.Item>
              <ContextMenu.Separator class="tree-menu-separator" />
              <ContextMenu.Item class="tree-menu-item" variant="destructive" onSelect={() => beginRequestDelete(menuPath, menuRoot)}><Trash2 size={14} aria-hidden="true" />Delete</ContextMenu.Item>
            {/if}
          </ContextMenu.Content>
        </ContextMenu.Root>
        <div class="aside-foot"><Folder size={12} aria-hidden="true" /> Files on your machine</div>
      {:else}
        <div class="sidebar-empty"><p>No collections found.</p><div class="sidebar-empty-actions"><Button variant="link" size="xs" class="h-auto rounded-sm p-0 text-[12px] font-normal text-[var(--brand-text)] underline underline-offset-2" aria-label="Create collection" onclick={() => beginCreate('collection')} disabled={busy || loading}>Create</Button><span>or</span><Button variant="link" size="xs" class="h-auto rounded-sm p-0 text-[12px] font-normal text-[var(--brand-text)] underline underline-offset-2" aria-label="Open collection" onclick={() => open()} disabled={busy || loading}>Open</Button><span>Collection.</span></div></div>
      {/if}
      <!-- ARIA window splitters use focusable separators; Svelte classifies the role as static. -->
      <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
      <div class="sidebar-resizer" role="separator" tabindex="0" aria-label="Resize collection sidebar" aria-orientation="vertical" aria-controls="collection-sidebar" aria-valuemin={SIDEBAR_MIN_WIDTH} aria-valuemax={sidebarMaxWidth(viewportWidth)} aria-valuenow={visibleSidebarWidth} aria-valuetext={`${visibleSidebarWidth} pixels`} title="Drag to resize · Double-click to reset" onpointerdown={startSidebarResize} onpointermove={resizeSidebar} onpointerup={finishSidebarResize} onpointercancel={finishSidebarResize} onlostpointercapture={finishSidebarResize} onkeydown={sidebarResizeKey} ondblclick={resetSidebarWidth}></div>
    </aside>
    <main class:request-workbench={!home && !manageWorkspacesOpen && !environmentOpen}>
      {#if manageWorkspacesOpen && WorkspaceManager}
        <WorkspaceManager {workspace} {error} disabled={busy || loading} onback={closeWorkspaceManager} onrename={(id) => beginWorkspaceAction('rename', id)} ondelete={(id) => beginWorkspaceAction('delete', id)} />
      {/if}
      {#if environmentOpen && EnvironmentManager && collection}
        <EnvironmentManager {collection} {environment} draft={envDraft} {baseUrl} {error} saved={environmentSaved} disabled={busy || loading}
          onback={closeEnvironment} onselect={chooseEnvironment} oncreate={() => { void manageEnvironment(true); }} onsave={saveEnvironment}
          ondiscard={() => { void discardEnvironment().catch(e => error = String(e)); }} onedit={(path, value) => { if (envDraft) editDraft(envDraft, path, value); }} onbaseurlchange={value => baseUrl = value} />
      {/if}
      <div class="request-view" hidden={manageWorkspacesOpen || environmentOpen}>
      {#if !home}
        <div class="request-toolbar">
          <RequestTabs tabs={requestTabs} activeId={activeRequestTab} {draft} disabled={busy || loading || chunkLoading} visible={!manageWorkspacesOpen && !environmentOpen} onactivate={activateRequestTab} onclose={closeRequestTab} onnew={newUntitled} />
          {#if collection}<div class="request-environment"><DropdownMenu.Root>
            <DropdownMenu.Trigger bind:ref={environmentMenuTrigger} class="workspace-select environment-select" aria-label="Environment" title="Switch environment" disabled={busy || loading}><span class="workspace-name">{environment || 'No environment'}</span><ChevronDown size={12} aria-hidden="true" /></DropdownMenu.Trigger>
            <DropdownMenu.Content class="tree-menu workspace-selector-menu motion-reduce:animate-none" align="end" sideOffset={6} onCloseAutoFocus={(event) => { if (environmentOpen || addingEnvironment) event.preventDefault(); }}>
              <div class="workspace-options"><DropdownMenu.RadioGroup value={String((collection?.environments ?? []).indexOf(environment) + 1)} onValueChange={(value) => { void chooseEnvironment(collection?.environments[Number(value) - 1] ?? ''); }}>
                {#each environmentOptions as item (item.value)}<DropdownMenu.RadioItem class="workspace-option" value={item.value} closeOnSelect={true} title={item.label}>{#snippet children()}<span class="workspace-option-name">{item.label}</span>{/snippet}</DropdownMenu.RadioItem>{/each}
              </DropdownMenu.RadioGroup></div>
              <DropdownMenu.Label class="workspace-menu-label">Environments</DropdownMenu.Label>
              <DropdownMenu.Item class="tree-menu-item" disabled={collection.readOnly} onSelect={() => manageEnvironment(true)}><Plus size={14} aria-hidden="true" />Create environment</DropdownMenu.Item>
              <DropdownMenu.Item class="tree-menu-item" onSelect={() => manageEnvironment()}><Settings2 size={14} aria-hidden="true" />Manage environments</DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Root></div>{/if}
        </div>
      {/if}
      {#if workbenchOpen}
        <div class="request-content" bind:clientHeight={responseAreaHeight} class:resizing-response={resizingResponse}>
        <div class="request-inputs">
        <div class="request-heading">
          <Breadcrumb.Root class="request-breadcrumb" aria-label="Request path"><Breadcrumb.List class="flex-nowrap">{#if !untitled && collection}<Breadcrumb.Item><span title={collection.name}>{collection.name}</span></Breadcrumb.Item>{#each selected.split('/').slice(0, -1) as folder}<Breadcrumb.Separator /><Breadcrumb.Item><span title={folder}>{folder}</span></Breadcrumb.Item>{/each}{:else}<Breadcrumb.Item><span>Unsaved request</span></Breadcrumb.Item>{/if}<Breadcrumb.Separator /><Breadcrumb.Item aria-current="page">{#if requestNameEdit !== null}<Input class="request-name-input" aria-label="Edit request name" value={requestNameEdit} maxlength={128} oninput={(event) => requestNameEdit = event.currentTarget.value} onblur={finishRequestName} onkeydown={requestNameKey} />{:else}<h1 aria-label={requestName || 'Untitled'}><Button variant="ghost" class="request-name-trigger" title={`${requestName || 'Untitled'} · Double-click to rename`} aria-label="Edit request name" disabled={busy || loading || (!untitled && collection?.readOnly)} ondblclick={editRequestName} onkeydown={(event) => { if (event.key === 'Enter' || event.key === 'F2') { event.preventDefault(); void editRequestName(); } }}>{requestName || 'Untitled'}</Button></h1>{/if}</Breadcrumb.Item></Breadcrumb.List></Breadcrumb.Root>
          <div class="save-actions">{#if notice}<span class="request-save-notice" class:sr-only={iconNotice} role="status" title={notice}>{notice}</span>{/if}{#if !untitled && collection?.readOnly}<span class="request-readonly">Read-only</span>{/if}<Button variant="ghost" size="sm" class="text-[12px] font-normal" title="Save request (⌘ / Ctrl + S)" disabled={busy || loading || !!activeExternalChange || !draft || (!untitled && ((!isEdited && !requestNameChanged) || collection?.readOnly))} onclick={saveRequest}>{#if requestSaved}<Check class="action-confirmation" aria-hidden="true" />{:else}<Save aria-hidden="true" />{/if}Save</Button>{#if isEdited || requestNameChanged || requestReloaded}<Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label="Discard changes" title="Discard changes" disabled={busy || loading || requestReloaded} onclick={discard}>{#if requestReloaded}<Check class="action-confirmation" aria-hidden="true" />{:else}<RotateCcw aria-hidden="true" />{/if}</Button>{/if}</div>
        </div>
        {#if activeExternalChange}<div class="error external-change" role="alert"><span>{activeExternalChange}</span><div class="save-actions"><Button variant="outline" size="sm" onclick={discard} disabled={busy || loading}>Reload from disk</Button><Button variant="outline" size="sm" onclick={saveExternalCopy} disabled={busy || loading}>Save copy</Button></div></div>{/if}
        <form class="request-bar" onsubmit={(e) => { e.preventDefault(); void send(); }}>
          <div class="request-target">
          <Select.Root type="single" value={method} items={methodOptions} allowDeselect={false} disabled={busy || loading} onValueChange={(value) => change(['http', 'method'], value)}><Select.Trigger class="request-method-select" aria-label="HTTP method" data-method={method.toUpperCase()}><Select.Value class="http-method" data-method={method.toUpperCase()} /></Select.Trigger><Select.Content align="start">{#each methodOptions as option}<Select.Item value={option.value} label={option.label}><span class="http-method" data-method={option.value.toUpperCase()}>{option.label}</span></Select.Item>{/each}</Select.Content></Select.Root>
          <Input class="h-full rounded-none" aria-label="Request URL" value={url} oninput={(e) => change(['http', 'url'], e.currentTarget.value)} placeholder="https://api.example.com" disabled={busy || loading} required />
          </div>
          {#if busy}<Button type="button" variant="secondary" size="lg" onclick={cancel}><LoaderCircle class="animate-spin" aria-hidden="true" />Cancel</Button>{:else}<Button type="submit" size="lg" disabled={loading || chunkLoading || !!activeExternalChange}><Send aria-hidden="true" />Send</Button>{/if}
        </form>
        {#if error}<div class="error" role="alert">{error}</div>{/if}
        {#if draft?.diagnostics.length}<div class="error">{draft.diagnostics.join('\n')} · Incompatible fields are retained; execution may be blocked.</div>{/if}
        {#if draft}<RequestEditor value={draft.value} requestId={activeRequestTab} {bodyEditorReset} disabled={busy || loading} onedit={change}
          variableContextKey={`${collection?.root ?? ''}:${selected}:${environment}:${draft.revision}:${envDraft?.revision ?? 0}`}
          loadvariables={() => api.variables(selected || null, environment || null)} {baseUrl} providedSecrets={Object.keys(secrets)}
          onsecretchange={(name, value) => { secrets[name] = value; }} onbaseurlchange={value => { baseUrl = value; }} />{/if}
        {#if secretNames.length}<Collapsible.Root class="secrets-panel"><Collapsible.Trigger class="secrets-trigger">Secrets · memory only ({secretNames.length})</Collapsible.Trigger><Collapsible.Content><p>These values are used at Send. They are never saved to YAML or history.</p>{#each secretNames as name}<label>{name}<Input type="password" aria-label={`Secret ${name}`} bind:value={secrets[name]} disabled={busy || loading} autocomplete="off" /></label>{/each}</Collapsible.Content></Collapsible.Root>{/if}
        {#if response?.historyWarning}<div class="error">HTTP completed; history could not be saved: {response.historyWarning}</div>{/if}
        </div>
        {#if response && !(import.meta.env.VITE_NATIVE_VALIDATION === '1' && validationHideResponseView)}
          <div id="response-pane" class="response-pane" style:height={`${visibleResponseHeight}px`}>
            <Separator class="response-resizer" decorative={false} orientation="horizontal" tabindex={0} aria-label="Resize response panel" aria-orientation="horizontal" aria-controls="response-pane" aria-valuemin={responseMinHeight(responseAreaHeight)} aria-valuemax={responseMaxHeight(responseAreaHeight)} aria-valuenow={visibleResponseHeight} aria-valuetext={`${visibleResponseHeight} pixels`} title="Drag to resize · Double-click to reset" onpointerdown={startResponseResize} onpointermove={resizeResponse} onpointerup={finishResponseResize} onpointercancel={finishResponseResize} onlostpointercapture={finishResponseResize} onkeydown={responseResizeKey} ondblclick={resetResponseHeight} />
            {#key response.id}
              {#await import('$lib/features/responses/ResponsePanel.svelte')}
                <p class="response-body-note" role="status">Loading response view…</p>
              {:then panel}
                <panel.default {response} {body} {offset} {chunkLoading} onloadMore={loadMore} />
              {:catch}
                <section class="response-panel" aria-label="Response">
                  <div class="response-heading"><h2>Response</h2><div class="response-stats"><span class="status" class:bad={response.status >= 400}>{response.status}</span><span>{response.elapsedMs} ms</span><span>{size(response.bodyBytes)}</span></div></div>
                  <div class="response-content response-body-content response-fallback">
                    <p class="response-body-note" role="alert">The response view could not be loaded. Showing the original body and headers.</p>
                    <ResponseText text={body || (response.bodyBytes === 0 ? '(empty body)' : 'Loading body…')} />
                    {#if offset < response.bodyBytes}<Button variant="outline" size="sm" onclick={loadMore} disabled={chunkLoading}>{chunkLoading ? 'Loading…' : 'Load next 64 KB'}</Button>{/if}
                    <pre aria-label="Response headers">{response.headers.map(header => `${header.name}: ${header.value}`).join('\n') || 'No response headers.'}</pre>
                  </div>
                </section>
              {/await}
            {/key}
          </div>
        {/if}
        </div>
      {:else}
        <div class="welcome"><span class="eyebrow">{welcomeCollection ? welcomeCollection.name : 'API DEVELOPMENT, LOCALLY'}</span><h1>{#if welcomeCollection}Your collection is ready.{:else if workspace?.collections.length}Choose a collection.{:else}A small client.<br />Your whole workflow.{/if}</h1><p>{#if welcomeCollection}Create your first request to get started.{:else if workspace?.collections.length}Select a collection in the sidebar or open another folder.{:else}Create a collection or open one from your repository.<br />Send requests without leaving your machine.{/if}</p><div>{#if welcomeCollection}<Button size="lg" onclick={() => beginCreate('request')} disabled={loading || welcomeCollection.readOnly}><Plus aria-hidden="true" />New request</Button>{:else}<Button size="lg" onclick={() => beginCreate('collection')} disabled={loading}><FolderPlus aria-hidden="true" />New collection</Button><Button variant="outline" size="lg" onclick={() => open()} disabled={loading}><FolderOpen aria-hidden="true" />Open collection</Button><Button variant="ghost" size="lg" onclick={() => open(true)} disabled={loading}>Try example</Button>{/if}</div>{#if error}<div class="error" role="alert">{error}</div>{/if}</div>
      {/if}
      </div>
    </main>
  </div>
  <footer>
    <span><span class="footer-dot"></span>Local-first · OpenCollection 1.0.0</span>
    <div class="footer-actions">
      <label class="theme-picker"><span class="theme-label">Theme</span><select aria-label="Theme" bind:value={theme}><option value="dark">Dark</option><option value="light">Light</option></select></label>
      <Button variant="ghost" size="xs" class="text-[10px] text-muted-foreground" onclick={() => { historyOpen = true; void refreshHistory(); }}><History aria-hidden="true" />History</Button>
      <span aria-label="App version">v{appVersion}</span>
    </div>
  </footer>
</div>

{#if creation}
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) cancelCreation(); }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label={titles[creation]} onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); if (manageWorkspacesOpen) restoreWorkspaceManagerFocus(); else void restoreTreeFocus(creationReturnFocus); }}>
    <div class="modal-heading"><Dialog.Title>{titles[creation]}</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={cancelCreation} disabled={loading}><X aria-hidden="true" /></Button></div>
    <Dialog.Description>{creation === 'rename-workspace' ? 'Change the workspace name. Collection folders stay in place.' : creation === 'workspace' ? 'Group local collections in a workspace.' : creation === 'collection' ? 'Give it a name, then choose where to create its folder.' : creation === 'folder' ? `Organize requests in ${collection?.name}.` : creation === 'rename-folder' ? `Rename ${creationSource}. Files and subfolders stay together.` : creation === 'rename' || creation === 'duplicate' ? creationSource : creation === 'save-request' ? 'Choose a name and a location for this temporary request.' : `Add a request to ${collection?.name}${creationFolder ? ` / ${creationFolder}` : ''}.`}</Dialog.Description>
    <form onsubmit={(event) => { event.preventDefault(); void create(); }}>
      <label>Name<Input autofocus aria-label={workspaceCreation ? 'Workspace name' : creation === 'collection' ? 'Collection name' : creation === 'folder' || creation === 'rename-folder' ? 'Folder name' : 'Request name'} value={creationName} oninput={(event) => creationNameChanged(event.currentTarget.value)} placeholder={workspaceCreation ? 'My workspace' : creation === 'collection' ? 'My API' : creation === 'folder' || creation === 'rename-folder' ? 'Users' : 'List users'} pattern={creation === 'rename-folder' ? '(?:[A-Za-z0-9_]|-){1,64}' : undefined} maxlength={creation === 'rename-folder' ? 64 : 128} required disabled={loading} /></label>
      {#if !workspaceCreation && creation !== 'rename-folder' && creation !== 'request' && creation !== 'save-request'}
      <label>{creation === 'collection' || creation === 'folder' ? 'Folder name' : 'File name'}<div class="creation-file"><Input aria-label={creation === 'collection' ? 'Collection folder' : creation === 'folder' ? 'Folder directory' : 'Request file'} bind:value={creationFile} placeholder={creation === 'collection' ? 'my-api' : creation === 'folder' ? 'users' : 'list-users'} pattern={'(?:[A-Za-z0-9_]|-){1,64}'} maxlength={64} required disabled={loading} />{#if creation !== 'collection' && creation !== 'folder'}<span>.{creationSource.endsWith('.yaml') ? 'yaml' : 'yml'}</span>{/if}</div></label>
      {/if}
      {#if creation === 'save-request'}
        <div class="save-request-destination">
          <div><label for="save-request-collection">Collection</label><Select.Root type="single" value={saveRoot} items={saveCollections.map(item => ({value:item.root,label:item.name}))} allowDeselect={false} disabled={loading} onValueChange={(value) => { saveRoot = value; creationFolder = ''; }}><Select.Trigger id="save-request-collection" class="w-full" aria-label="Save request collection"><Select.Value placeholder="Choose a collection" /></Select.Trigger><Select.Content portalProps={{disabled:true}}>{#each saveCollections as item}<Select.Item value={item.root} label={item.name}>{item.name}</Select.Item>{/each}</Select.Content></Select.Root></div>
          <div><label for="save-request-folder">Folder</label><Select.Root type="single" value={creationFolder || '/'} items={[{value:'/',label:'Collection root'},...(saveCollection?.folders ?? []).map(folder=>({value:folder,label:folder}))]} allowDeselect={false} disabled={loading || !saveCollection} onValueChange={(value) => creationFolder = value === '/' ? '' : value}><Select.Trigger id="save-request-folder" class="w-full" aria-label="Save request folder"><Select.Value /></Select.Trigger><Select.Content portalProps={{disabled:true}}><Select.Item value="/" label="Collection root">Collection root</Select.Item>{#each saveCollection?.folders ?? [] as folder}<Select.Item value={folder} label={folder}>{folder}</Select.Item>{/each}</Select.Content></Select.Root></div>
        </div>
        {#if saveCollection}<p class="save-request-path">{saveCollection.root}{creationFolder ? `/${creationFolder}` : ''}/</p>{:else}<p>Create a writable collection to save this request.</p>{/if}
        <Button variant="link" size="sm" class="px-0 text-[12px] font-normal" disabled={loading} onclick={createSaveCollection}><FolderPlus aria-hidden="true" />New collection</Button>
      {/if}
      {#if creation === 'folder'}<div><label for="destination-folder">Inside</label><Select.Root type="single" value={creationFolder || '/'} items={[{value:'/',label:`${collection?.name} /`},...(collection?.folders ?? []).map(folder=>({value:folder,label:folder}))]} allowDeselect={false} disabled={loading} onValueChange={(value) => creationFolder = value === '/' ? '' : value}><Select.Trigger id="destination-folder" class="w-full" aria-label="Destination folder"><Select.Value /></Select.Trigger><Select.Content portalProps={{disabled:true}}><Select.Item value="/" label={`${collection?.name} /`}>{collection?.name} /</Select.Item>{#each collection?.folders ?? [] as folder}<Select.Item value={folder} label={folder}>{folder}</Select.Item>{/each}</Select.Content></Select.Root></div>{/if}
      {#if creation === 'request'}<div class="creation-http"><div class="creation-method"><label for="new-request-method">Method</label><Select.Root type="single" bind:value={creationMethod} items={creationMethods} allowDeselect={false} disabled={loading}>
          <Select.Trigger id="new-request-method" class="w-full px-2 text-xs" aria-label="New request method"><Select.Value class="http-method" data-method={creationMethod} /></Select.Trigger>
          <Select.Content align="start" class="p-1 motion-reduce:animate-none" portalProps={{ disabled: true }}>{#each creationMethods as verb (verb.value)}<Select.Item value={verb.value} label={verb.label}><span class="http-method text-xs" data-method={verb.value}>{verb.label}</span></Select.Item>{/each}</Select.Content>
        </Select.Root></div><label>URL (optional)<Input aria-label="New request URL" bind:value={creationUrl} placeholder="https://api.example.com/users" maxlength={8192} disabled={loading} /></label></div>{/if}
      {#if creationError}<div class="error" role="alert">{creationError}</div>{/if}
      <div class="creation-actions"><Button variant="ghost" onclick={cancelCreation} disabled={loading}>Cancel</Button><Button type="submit" disabled={loading || !creationName.trim() || (creation === 'save-request' && !saveCollection) || (!workspaceCreation && creation !== 'rename-folder' && creation !== 'request' && creation !== 'save-request' && !creationFile)}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{/if}{creation === 'save-request' ? 'Save request' : creation === 'rename-workspace' ? 'Save name' : creation === 'rename-folder' ? 'Rename folder' : creation === 'workspace' ? 'Create workspace' : creation === 'collection' ? 'Choose location & create' : creation === 'folder' ? 'Create folder' : creation === 'rename' ? 'Rename request' : creation === 'duplicate' ? 'Duplicate request' : 'Create request'}</Button></div>
    </form>
  </Dialog.Content></Dialog.Root>
{/if}

{#if closingTab}
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) closingRequestTab = null; }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="Save changes before closing?" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); if (manageWorkspacesOpen) restoreWorkspaceManagerFocus(); else void restoreTreeFocus(creationReturnFocus); }}>
    <div class="modal-heading"><Dialog.Title>Save changes before closing?</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => closingRequestTab = null} disabled={loading}><X aria-hidden="true" /></Button></div>
    <Dialog.Description>Save “{closingTab.draft.value.info?.name ?? 'Untitled'}” before closing its tab?{!closingTab.path ? ' Choose a name and location in the next step.' : ' Your changes will be saved to its current file.'}</Dialog.Description>
    {#if closeTabError}<div class="error" role="alert">{closeTabError}</div>{/if}
    <div class="creation-actions"><Button autofocus variant="ghost" onclick={() => closingRequestTab = null} disabled={loading}>Cancel</Button><Button variant="outline" onclick={discardClosingTab} disabled={loading}>Close without saving</Button><Button onclick={saveClosingTab} disabled={loading || closingTabReadOnly || !!closingTab?.externalChange}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{/if}Save</Button></div>
  </Dialog.Content></Dialog.Root>
{/if}


{#if deletingWorkspace}
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) deletingWorkspace = null; }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="Delete workspace" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); if (manageWorkspacesOpen) restoreWorkspaceManagerFocus(); else workspaceMenuTrigger?.focus(); }}>
    <div class="modal-heading"><Dialog.Title>Delete workspace</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => deletingWorkspace = null} disabled={loading}><X aria-hidden="true" /></Button></div>
    <Dialog.Description>Remove “{deletingWorkspace.name}” and its collection references from NimblePost? Collection folders and files will remain on your machine.{deletingWorkspace.id === workspace?.activeWorkspaceId ? ' You will return to the default workspace.' : ''}</Dialog.Description>
    {#if deletionError}<div class="error" role="alert">{deletionError}</div>{/if}
    <div class="creation-actions"><Button autofocus variant="ghost" onclick={() => deletingWorkspace = null} disabled={loading}>Cancel</Button><Button variant="destructive" onclick={deleteWorkspace} disabled={loading}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{:else}<Trash2 aria-hidden="true" />{/if}Delete workspace</Button></div>
  </Dialog.Content></Dialog.Root>
{/if}

{#if deletingRequest}
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) deletingRequest = null; }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="Delete request" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={async (event) => { event.preventDefault(); await tick(); if (!await restoreTreeFocus(creationReturnFocus)) (document.querySelector<HTMLElement>('.file.active') ?? document.querySelector<HTMLElement>('[aria-label="New collection"]'))?.focus(); }}>
    <div class="modal-heading"><Dialog.Title>Delete request</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => deletingRequest = null} disabled={loading}><X aria-hidden="true" /></Button></div>
    <Dialog.Description>Delete “{deletingRequest.name}” from {collection?.name}{deletingRequest.path.includes('/') ? ` / ${deletingRequest.path.split('/').slice(0, -1).join('/')}` : ''}? The file {deletingRequest.path} will be permanently deleted.{deletingRequestEdited ? ' Its unsaved changes will also be discarded.' : ''}</Dialog.Description>
    {#if deletionError}<div class="error" role="alert">{deletionError}</div>{/if}
    <div class="creation-actions"><Button autofocus variant="ghost" onclick={() => deletingRequest = null} disabled={loading}>Cancel</Button><Button variant="destructive" onclick={deleteRequest} disabled={loading}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{:else}<Trash2 aria-hidden="true" />{/if}Delete request</Button></div>
  </Dialog.Content></Dialog.Root>
{/if}

{#if addingEnvironment}<Dialog.Root open={true}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="New environment" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { event.preventDefault(); closeEnvironmentCreation(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLInputElement>('.shadcn-modal input[aria-label="New environment name"]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); (environmentOpen ? document.querySelector<HTMLElement>('#create-managed-environment') : environmentMenuTrigger)?.focus(); }}><div class="modal-heading"><Dialog.Title>New environment</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={closeEnvironmentCreation} disabled={loading}><X aria-hidden="true" /></Button></div>
  <Dialog.Description>Group variables in an environment.</Dialog.Description>
    <form onsubmit={(event) => { event.preventDefault(); void createEnvironment(); }}>
      <label>Name<Input autofocus aria-label="New environment name" bind:value={newEnvironment} placeholder="staging" pattern={'(?:[A-Za-z0-9_]|-){1,64}'} maxlength={64} required disabled={loading || collection?.readOnly} /></label>
      {#if error}<div class="error" role="alert">{error}</div>{/if}
      <div class="creation-actions"><Button variant="ghost" onclick={closeEnvironmentCreation} disabled={loading}>Cancel</Button><Button type="submit" disabled={loading || !newEnvironment.trim() || collection?.readOnly}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{/if}Create environment</Button></div>
    </form>
</Dialog.Content></Dialog.Root>{/if}

{#if historyOpen}<dialog use:modal class="modal" aria-label="Execution history" oncancel={() => historyOpen = false}><div class="modal-heading"><h2>History · metadata only</h2><div class="save-actions"><Button variant="outline" size="sm" onclick={clearHistory} disabled={!history.entries.length}>Clear history</Button><Button variant="outline" size="sm" onclick={() => historyOpen = false}><X aria-hidden="true" />Close</Button></div></div>
  <p>Last 200 executions. No URLs, credentials, variable values, headers or bodies are stored. Clearing removes all retained records.</p>{#if history.warning}<div class="error">{history.warning}</div>{/if}
  <table><thead><tr><th>When / request</th><th>Method</th><th>Result</th><th>Duration</th><th>Size</th></tr></thead><tbody>{#each history.entries as entry}<tr><td>{new Date(entry.timestamp * 1000).toLocaleString()}<br />{entry.requestPath}</td><td><span class="http-method" data-method={entry.method.toUpperCase()}>{entry.method}</span></td><td>{entry.status ?? entry.outcome}</td><td>{entry.elapsedMs} ms</td><td>{size(entry.bodyBytes)}</td></tr>{/each}</tbody></table>{#if !history.entries.length}<p>No executions recorded yet.</p>{/if}
</dialog>{/if}
