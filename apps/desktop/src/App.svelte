<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { version as appVersion } from '../package.json';
  import { isTauri } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { readTheme, saveTheme, type Theme } from '$lib/layout/theme';
  import { SIDEBAR_MIN_WIDTH, sidebarMaxWidth, clampSidebarWidth, readSidebarWidth, saveSidebarWidth } from '$lib/layout/sidebar';
  import { RESPONSE_DEFAULT_HEIGHT, responseMinHeight, responseMaxHeight, clampResponseHeight, readResponseHeight, saveResponseHeight } from '$lib/features/responses/response-layout';
  import { api, type Collection, type WorkspaceView, type ResponseMeta, type HistoryView, type RequestInfo, type RequestSummary } from '$lib/desktop/api';
  import { fileTree, type TreeNode } from '$lib/features/collections/tree';
  import { draftFrom, editDraft, type Draft } from '$lib/features/requests/draft';
  import { untitledDraft, refreshTabDraft, type RequestTab } from '$lib/features/requests/request-tabs';
  import RequestEditor from '$lib/features/requests/RequestEditor.svelte';
  import RequestTabs from '$lib/features/requests/RequestTabs.svelte';
  import WorkspaceManager from '$lib/features/workspaces/WorkspaceManager.svelte';
  import ResponsePanel from '$lib/features/responses/ResponsePanel.svelte';
  import Rows from '$lib/components/Rows.svelte';
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
  import { Check, ChevronDown, ChevronRight, Copy, Ellipsis, Folder, FolderOpen, FolderPlus, History, House, LoaderCircle, Pencil, Plus, RotateCcw, Save, Send, Settings2, Trash2, X } from '@lucide/svelte';

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
  $effect(() => {
    const owners = (workspace?.collections ?? []).filter(owner => !owner.warning)
      .map(owner => ({ root: owner.root, paths: [...owner.requests] }));
    let cancelled = false;
    void (async () => {
      for (const owner of owners) {
        for (let offset = 0; offset < owner.paths.length; offset += 128) {
          if (cancelled) return;
          try {
            const summaries = await api.requestSummaries(owner.root, owner.paths.slice(offset, offset + 128));
            if (cancelled) return;
            requestSummaries[owner.root] = { ...summaries, ...requestSummaries[owner.root] };
          } catch (error) { if (!cancelled) console.warn('Request labels could not be loaded.', error); break; }
        }
      }
    })();
    return () => { cancelled = true; };
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
  let newEnvironment = $state('');
  let secretNames = $state<string[]>([]);
  let secrets = $state<Record<string, string>>({});
  let history = $state<HistoryView>({ entries: [], warning: null });
  let historyOpen = $state(false);
  let notice = $state('');
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
  let deletingWorkspace = $state<{ id: number; name: string } | null>(null);
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
  let creationName = $state('');
  let creationFile = $state('');
  let creationMethod = $state('GET');
  const creationMethods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'].map(value => ({ value, label: value }));
  let creationUrl = $state('');
  let creationError = $state('');
  onMount(() => { void refreshHistory(); void restoreWorkspace(); });
  onMount(() => {
    if (!isTauri()) return;
    const listener = getCurrentWindow().onCloseRequested(event => {
      if (unsavedTabs || isEdited || envDraft?.edits.length) { event.preventDefault(); error = 'Save or close your unsaved request tabs before closing this window.'; }
    });
    return () => { void listener.then(unlisten => unlisten()); };
  });
  function modal(node: HTMLDialogElement) {
    node.showModal();
    return { destroy() {
      if (node.open) node.close();
      if (node.classList.contains('creation-modal')) {
        const target = creationReturnFocus?.isConnected ? creationReturnFocus : document.querySelector<HTMLElement>('.sidebar-create button');
        target?.focus();
      }
    } };
  }

  async function refreshHistory() { try { history = await api.history(); } catch (e) { history.warning = String(e); } }
  function canLeave() { if (unsavedTabs || isEdited || envDraft?.edits.length) { error = 'Save or close your unsaved request tabs before switching workspaces or collections.'; return false; } return true; }
  function stashRequestTab() {
    const item = requestTabs.find(item => item.id === activeRequestTab);
    if (item && draft) Object.assign(item, { draft, environment, baseUrl, response, body, offset });
  }
  async function newUntitled() {
    if (busy || loading || chunkLoading || envDraft?.edits.length) return;
    stashRequestTab();
    draft = untitledDraft(); selected = ''; requestName = 'Untitled'; method = 'GET'; url = '';
    response = null; body = ''; offset = 0; error = ''; notice = ''; home = false; manageWorkspacesOpen = false;
    activeRequestTab = ++nextRequestTab;
    requestTabs.push({ id: activeRequestTab, root: collection?.root ?? null, path: '', source: null, draft, environment, baseUrl, response: null, body: '', offset: 0 });
    await tick(); document.querySelector<HTMLInputElement>('input[aria-label="Request URL"]')?.focus();
  }
  async function activateRequestTab(id: number) {
    if (busy || loading || chunkLoading || envDraft?.edits.length || id === activeRequestTab) return;
    stashRequestTab();
    const item = requestTabs.find(item => item.id === id);
    if (!item) return;
    loading = true; error = '';
    const previousRoot = collection?.root;
    try {
      if (item.root && item.root !== previousRoot) {
        if (requestTabs.some(tab => tab.path && tab.draft.edits.length)) throw new Error('Save or Discard edited collection requests before switching collections.');
        updateCollection(await api.selectCollection(item.root));
      }
      if (item.path) {
        const fresh = await api.request(item.path);
        try { item.draft = refreshTabDraft(item, fresh); item.source = fresh; }
        catch (e) { error = String(e); }
      }
      activeRequestTab = id; selected = item.path; draft = item.draft;
      requestName = draft.value.info?.name ?? 'Untitled'; method = draft.value.http?.method ?? 'GET'; url = draft.value.http?.url ?? '';
      environment = collection?.environments.includes(item.environment) ? item.environment : ''; baseUrl = item.baseUrl;
      response = item.response; body = item.body; offset = item.offset; decoder = new TextDecoder(); notice = ''; home = false;
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
      creationReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      closeTabError = ''; closingRequestTab = id; return;
    }
    await removeRequestTab(id);
  }
  async function removeRequestTab(id: number) {
    requestTabs = requestTabs.filter(item => item.id !== id);
    if (id !== activeRequestTab) return;
    draft = null; selected = ''; response = null; body = ''; error = ''; notice = ''; activeRequestTab = 0;
    const next = requestTabs.at(-1);
    if (next) await activateRequestTab(next.id);
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
    if (existing) Object.assign(existing, { root: collection?.root ?? null, path: selected, source: view, draft });
    else if (selected) {
      activeRequestTab = ++nextRequestTab;
      requestTabs.push({ id: activeRequestTab, root: collection?.root ?? null, path: selected, source: view, draft, environment, baseUrl, response: null, body: '', offset: 0 });
    }
    if (collection && selected) requestSummaries[collection.root] = { ...requestSummaries[collection.root], [selected]: { name: view.name, method: view.method.toUpperCase() } };
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
    finishRequestName();
    if (untitled) {
      creatingSaveCollection = false;
      stashRequestTab(); creationReturnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
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
    if (selected) { loading = true; try { accept(await api.request(selected)); error = ''; notice = 'Reloaded from disk.'; } catch (e) { error = String(e); } finally { loading = false; } }
  }
  async function environmentChanged() {
    secrets = {}; secretNames = []; envDraft = null;
    if (!environment) return;
    const wasLoading = loading; loading = true;
    try { const view = await api.environment(environment); secretNames = (view.document.variables ?? []).filter((row: { secret?: boolean; disabled?: boolean }) => row.secret && !row.disabled).map((row: { name: string }) => row.name); }
    catch (e) { error = String(e); }
    finally { loading = wasLoading; }
  }
  async function manageEnvironment() {
    if (busy || loading) return;
    loading = true; error = '';
    try { envDraft = environment ? draftFrom(await api.environment(environment)) : null; environmentOpen = true; }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function createEnvironment() {
    loading = true; error = '';
    try { const view = await api.createEnvironment(newEnvironment); envDraft = draftFrom(view); environment = view.name; collection!.environments = [...collection!.environments, view.name].sort(); newEnvironment = ''; secrets = {}; secretNames = []; }
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
  function closeEnvironment() { if (envDraft?.edits.length) { error = 'Save or Discard your environment changes before closing.'; return; } environmentOpen = false; error = ''; }
  async function discardEnvironment() { if (!environment) { envDraft = null; return; } envDraft = draftFrom(await api.environment(environment)); error = ''; }
  async function clearHistory() { try { await api.clearHistory(); await refreshHistory(); } catch (e) { history.warning = String(e); } }

  function beginCreate(kind: 'workspace' | 'collection' | 'folder' | 'request', folder = '') {
    if (busy || loading || !canLeave() || (kind !== 'collection' && kind !== 'workspace' && (!collection || collection.readOnly))) return;
    creationReturnFocus = kind === 'workspace' ? workspaceMenuTrigger : document.activeElement instanceof HTMLElement ? (document.activeElement.closest('[role="menu"]') ? menuInvoker : document.activeElement) : null;
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
  function treeMenuTarget(event: MouseEvent | PointerEvent) {
    const row = event.target instanceof Element ? event.target.closest<HTMLElement>('[data-tree-path]') : null;
    if (!row) { event.preventDefault(); return; }
    menuInvoker = event.target instanceof Element ? event.target.closest<HTMLElement>('button') ?? row.querySelector('button') : null;
    const owner = workspace?.collections.find(collection => collection.root === row.dataset.treeRoot);
    if (!owner || ((owner.readOnly || owner.warning) && row.dataset.treeCollection !== 'true')) { event.preventDefault(); return; }
    menuRoot = owner.root; menuPath = row.dataset.treePath!; menuFolder = row.dataset.treeFolder === 'true'; menuCollection = row.dataset.treeCollection === 'true';
  }
  function showTreeMenu(event: MouseEvent) {
    event.preventDefault(); event.stopPropagation();
    const button = event.currentTarget as HTMLElement;
    const rect = button.getBoundingClientRect();
    button.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, cancelable: true, clientX: rect.right, clientY: rect.bottom }));
  }
  function creationNameChanged(name: string) {
    creationName = name;
    if (creation === 'collection' || creation === 'folder') creationFile = name.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 64);
  }
  function updateCollection(result: Collection) {
    collection = result;
    if (workspace) {
      workspace.collections = workspace.collections.some(item => item.root === result.root) ? workspace.collections.map(item => item.root === result.root ? result : item) : [...workspace.collections, result];
      workspace.activeCollection = result.root;
    }
  }
  function goHome() {
    if (busy || loading || !canLeave()) return;
    manageWorkspacesOpen = false; home = true; error = ''; notice = '';
  }
  async function applyWorkspace(result: WorkspaceView) {
    requestSummaries = {};
    requestTabs = []; activeRequestTab = 0;
    workspace = result; collection = null; selected = ''; draft = null; envDraft = null; environmentOpen = false; environment = ''; secretNames = []; secrets = {}; response = null; body = ''; baseUrl = ''; notice = '';
    const active = result.collections.find(item => item.root === result.activeCollection && !item.warning);
    if (active) await useCollection(active);
  }
  async function restoreWorkspace() {
    loading = true;
    try { await applyWorkspace(await api.workspace()); }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function changeWorkspace(id: number) {
    if (id === workspace?.activeWorkspaceId || busy || loading || !canLeave()) return;
    loading = true; error = '';
    try { await applyWorkspace(await api.selectWorkspace(id)); manageWorkspacesOpen = false; }
    catch (e) { error = String(e); }
    finally { loading = false; }
  }
  async function openWorkspaceManager() {
    if (busy || loading) return;
    error = ''; manageWorkspacesOpen = true;
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
    if (busy || loading) return false;
    if (collection?.root === root) { home = false; manageWorkspacesOpen = false; if (path) await select(path); return true; }
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
      (menuInvoker?.isConnected ? menuInvoker : document.querySelector<HTMLElement>('[aria-label="New collection"]'))?.focus();
    }
  }
  async function useCollection(result: Collection, example = false, path?: string) {
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
        const result = await api.createRequest({ name: creationName.trim(), folder: creationFolder, method, url, edits: draft.edits });
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
        creationReturnFocus = Array.from(document.querySelectorAll<HTMLElement>('.tree-row')).find(row => row.dataset.treeRoot === collection?.root && row.dataset.treePath === destination)?.querySelector('button') ?? null;
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
    if (!draft || busy || loading || chunkLoading || environmentOpen || creation || deletingWorkspace) return;
    busy = true; error = ''; response = null; body = ''; offset = 0; decoder = new TextDecoder();
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
  function shortcut(e: KeyboardEvent) { if (creation || deletingWorkspace || manageWorkspacesOpen || closingRequestTab !== null) return; if ((e.metaKey || e.ctrlKey) && e.key === 'Enter') { e.preventDefault(); void send(); } if ((e.metaKey || e.ctrlKey) && e.key === 's') { e.preventDefault(); if (environmentOpen) void saveEnvironment(); else void saveRequest(); } }
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
</script>

<svelte:window onkeydown={shortcut} bind:innerWidth={viewportWidth} />

{#snippet emptyRequest(owner: Collection, folder: string)}
  <div class="sidebar-create"><Button variant="ghost" size="sm" class="h-7 justify-start px-1 text-[12px] leading-[18px] font-normal text-[var(--brand-text)]" aria-label={`New request in ${owner.name}${folder ? ` / ${folder}` : ''}`} onclick={() => createIn(owner.root, 'request', folder)} disabled={busy || loading || owner.readOnly}><Plus aria-hidden="true" />Request</Button></div>
{/snippet}

{#snippet branch(nodes: TreeNode[], owner: Collection)}
  <ul class="tree">
    {#each nodes as node (node.name)}
      <li>
        {#if node.path}
          {@const summary = !home && collection?.root === owner.root && selected === node.path && draft ? { name: requestName, method: method.toUpperCase() } : requestSummaries[owner.root]?.[node.path]}
          <div class="tree-row" data-tree-root={owner.root} data-tree-path={node.path} data-tree-folder="false">
          <Button variant="ghost" class={`file ${!home && collection?.root === owner.root && selected === node.path ? "active" : ""}`} disabled={busy || loading} onclick={() => chooseCollection(owner.root, node.path!)} title={node.path}>
            <span class="request-method http-method" data-method={summary?.method} title={summary?.method}>{summary?.method === 'DELETE' ? 'DEL' : summary?.method === 'OPTIONS' ? 'OPT' : summary?.method ?? 'HTTP'}</span><span class="tree-name">{summary?.name ?? node.name.replace(/\.ya?ml$/, '')}</span>
          </Button>
          <Button variant="ghost" class="tree-action" aria-label={`Actions for ${node.name}`} title="Request actions" disabled={busy || loading || owner.readOnly} onclick={showTreeMenu}><Ellipsis size={14} aria-hidden="true" /></Button>
          </div>
        {:else}
          <details open><summary><ChevronRight size={12} class="tree-chevron" aria-hidden="true" /><span class="tree-row" data-tree-root={owner.root} data-tree-path={node.folderPath} data-tree-folder="true"><span class="folder-label"><Folder size={14} class="folder-symbol" aria-hidden="true" /><span class="tree-name">{node.name}</span></span><Button variant="ghost" class="tree-action" aria-label={`Actions for folder ${node.name}`} title="Folder actions" disabled={busy || loading || owner.readOnly} onclick={showTreeMenu}><Ellipsis size={14} aria-hidden="true" /></Button></span></summary>{#if !node.children.length}{@render emptyRequest(owner, node.folderPath!)}{:else}{@render branch(node.children, owner)}{/if}</details>
        {/if}
      </li>
    {/each}
  </ul>
{/snippet}

<div class="app-shell">
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
  <div class="workspace" class:resizing-sidebar={resizingSidebar} style:--sidebar-width={`${visibleSidebarWidth}px`}>
    <aside id="collection-sidebar">
      <div class="aside-heading"><span class="aside-title"><Folder size={14} aria-hidden="true" />Collections</span><div class="collection-actions"><Button variant="ghost" size="icon-sm" aria-label="New collection" title="New collection" onclick={() => beginCreate('collection')} disabled={busy || loading}><FolderPlus aria-hidden="true" /></Button><Button variant="ghost" size="icon-sm" aria-label="Open collection" title="Open collection" onclick={() => open()} disabled={busy || loading}><FolderOpen aria-hidden="true" /></Button></div></div>
      {#if workspace?.warning}<p class="workspace-warning" role="alert">{workspace.warning}</p>{/if}
      {#if workspace?.collections.length}
        <ContextMenu.Root>
          <ContextMenu.Trigger class="tree-scroll" disabled={busy || loading} oncontextmenu={treeMenuTarget}>
            {#each workspace.collections as owner (owner.root)}
              {@const nodes = fileTree(owner.requests, owner.folders)}
              <details class="workspace-collection" open>
                <summary data-tree-root={owner.root} data-tree-path="" data-tree-collection="true"><ChevronRight size={12} class="tree-chevron" aria-hidden="true" /><Button variant="ghost" class={`collection-select ${!home && collection?.root === owner.root ? "current" : ""}`} title={owner.root} disabled={busy || loading || !!owner.warning} onclick={(event) => { event.preventDefault(); void chooseCollection(owner.root); }}><Folder size={14} aria-hidden="true" /><span>{owner.name}</span>{#if owner.readOnly}<span class="readonly-tag">RO</span>{/if}</Button><Button variant="ghost" class="tree-action" title="Collection actions" aria-label={`Actions for collection ${owner.name}`} aria-haspopup="menu" disabled={busy || loading} onclick={showTreeMenu}><Ellipsis size={14} aria-hidden="true" /></Button></summary>
                {#if owner.warning}<p class="workspace-warning" role="alert">{owner.warning}</p>{:else}
                  {#if !nodes.length}{@render emptyRequest(owner, '')}{:else}{@render branch(nodes, owner)}{/if}
                {/if}
              </details>
            {/each}
          </ContextMenu.Trigger>
          <ContextMenu.Content class="tree-menu" onCloseAutoFocus={(event) => { event.preventDefault(); if (!creation) menuInvoker?.focus(); }}>
            {#if menuCollection || menuFolder}
              <ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => createIn(menuRoot, 'request', menuPath)}><Plus size={14} aria-hidden="true" />New request</ContextMenu.Item>
              <ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => createIn(menuRoot, 'folder', menuPath)}><FolderPlus size={14} aria-hidden="true" />New folder</ContextMenu.Item>
              {#if menuFolder}<ContextMenu.Item class="tree-menu-item" disabled={menuCannotCreate} onSelect={() => beginFolderRename(menuRoot, menuPath)}><Pencil size={14} aria-hidden="true" />Rename</ContextMenu.Item>{/if}
              {#if menuCollection}
                <ContextMenu.Separator class="tree-menu-separator" />
                <ContextMenu.Item class="tree-menu-item" onSelect={() => menuInvoker?.closest('details')?.removeAttribute('open')}><Folder size={14} aria-hidden="true" />Collapse</ContextMenu.Item>
                <ContextMenu.Separator class="tree-menu-separator" />
                <ContextMenu.Item class="tree-menu-item" title="Remove from workspace · files are kept" onSelect={() => removeCollection(menuRoot)}><X size={14} aria-hidden="true" />Remove from workspace</ContextMenu.Item>
              {/if}
            {:else}
              <ContextMenu.Item class="tree-menu-item" onSelect={() => beginRequestAction('rename', menuPath, menuRoot)}><Pencil size={14} aria-hidden="true" />Rename</ContextMenu.Item>
              <ContextMenu.Item class="tree-menu-item" onSelect={() => beginRequestAction('duplicate', menuPath, menuRoot)}><Copy size={14} aria-hidden="true" />Duplicate</ContextMenu.Item>
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
    <main class:request-workbench={!home && !manageWorkspacesOpen}>
      {#if manageWorkspacesOpen}
        <WorkspaceManager {workspace} {error} disabled={busy || loading} onback={closeWorkspaceManager} onrename={(id) => beginWorkspaceAction('rename', id)} ondelete={(id) => beginWorkspaceAction('delete', id)} />
      {/if}
      <div class="request-view" hidden={manageWorkspacesOpen}>
      {#if !home}
        <div class="request-toolbar">
          <RequestTabs tabs={requestTabs} activeId={activeRequestTab} {draft} disabled={busy || loading || chunkLoading} visible={!manageWorkspacesOpen} onactivate={activateRequestTab} onclose={closeRequestTab} onnew={newUntitled} />
          {#if collection}<div class="request-environment"><Select.Root type="single" value={String(((collection?.environments ?? []).indexOf(environment) + 1))} items={environmentOptions} allowDeselect={false} disabled={busy || loading} onValueChange={(value) => { environment = collection?.environments[Number(value) - 1] ?? ''; void environmentChanged(); }}>
            <Select.Trigger class="environment-select" size="sm" aria-label="Environment"><Select.Value class="min-w-0 truncate" placeholder="No environment" /></Select.Trigger>
            <Select.Content align="end" class="p-1 motion-reduce:animate-none">{#each environmentOptions as item (item.value)}<Select.Item value={item.value} label={item.label}>{item.label}</Select.Item>{/each}</Select.Content>
          </Select.Root><Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label="Edit environments" title="Manage environments and baseUrl override" disabled={busy || loading} onclick={manageEnvironment}><Settings2 aria-hidden="true" /></Button></div>{/if}
        </div>
      {/if}
      {#if workbenchOpen}
        <div class="request-content" bind:clientHeight={responseAreaHeight} class:resizing-response={resizingResponse}>
        <div class="request-inputs">
        <div class="request-heading">
          <Breadcrumb.Root class="request-breadcrumb" aria-label="Request path"><Breadcrumb.List class="flex-nowrap">{#if !untitled && collection}<Breadcrumb.Item><span title={collection.name}>{collection.name}</span></Breadcrumb.Item>{#each selected.split('/').slice(0, -1) as folder}<Breadcrumb.Separator /><Breadcrumb.Item><span title={folder}>{folder}</span></Breadcrumb.Item>{/each}{:else}<Breadcrumb.Item><span>Unsaved request</span></Breadcrumb.Item>{/if}<Breadcrumb.Separator /><Breadcrumb.Item aria-current="page">{#if requestNameEdit !== null}<Input class="request-name-input" aria-label="Edit request name" value={requestNameEdit} maxlength={128} oninput={(event) => requestNameEdit = event.currentTarget.value} onblur={finishRequestName} onkeydown={requestNameKey} />{:else}<h1 aria-label={requestName || 'Untitled'}><Button variant="ghost" class="request-name-trigger" title={`${requestName || 'Untitled'} · Double-click to rename`} aria-label="Edit request name" disabled={busy || loading || (!untitled && collection?.readOnly)} ondblclick={editRequestName} onkeydown={(event) => { if (event.key === 'Enter' || event.key === 'F2') { event.preventDefault(); void editRequestName(); } }}>{requestName || 'Untitled'}</Button></h1>{/if}</Breadcrumb.Item></Breadcrumb.List></Breadcrumb.Root>
          <div class="save-actions">{#if notice}<span class="request-save-notice" class:sr-only={iconNotice} role="status" title={notice}>{notice}</span>{/if}{#if !untitled && collection?.readOnly}<span class="request-readonly">Read-only</span>{/if}<Button variant="ghost" size="sm" class="text-[12px] font-normal" title="Save request (⌘ / Ctrl + S)" disabled={busy || loading || !draft || (!untitled && ((!isEdited && !requestNameChanged) || collection?.readOnly))} onclick={saveRequest}>{#if requestSaved}<Check class="action-confirmation" aria-hidden="true" />{:else}<Save aria-hidden="true" />{/if}Save</Button>{#if isEdited || requestNameChanged || requestReloaded}<Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label="Discard changes" title="Discard changes" disabled={busy || loading || requestReloaded} onclick={discard}>{#if requestReloaded}<Check class="action-confirmation" aria-hidden="true" />{:else}<RotateCcw aria-hidden="true" />{/if}</Button>{/if}</div>
        </div>
        <form class="request-bar" onsubmit={(e) => { e.preventDefault(); void send(); }}>
          <div class="request-target">
          <Select.Root type="single" value={method} items={methodOptions} allowDeselect={false} disabled={busy || loading} onValueChange={(value) => change(['http', 'method'], value)}><Select.Trigger class="request-method-select" aria-label="HTTP method" data-method={method.toUpperCase()}><Select.Value class="http-method" data-method={method.toUpperCase()} /></Select.Trigger><Select.Content align="start">{#each methodOptions as option}<Select.Item value={option.value} label={option.label}><span class="http-method" data-method={option.value.toUpperCase()}>{option.label}</span></Select.Item>{/each}</Select.Content></Select.Root>
          <Input class="h-full rounded-none" aria-label="Request URL" value={url} oninput={(e) => change(['http', 'url'], e.currentTarget.value)} placeholder="https://api.example.com" disabled={busy || loading} required />
          </div>
          {#if busy}<Button type="button" variant="secondary" size="lg" onclick={cancel}><LoaderCircle class="animate-spin" aria-hidden="true" />Cancel</Button>{:else}<Button type="submit" size="lg" disabled={loading || chunkLoading}><Send aria-hidden="true" />Send</Button>{/if}
        </form>
        {#if error}<div class="error" role="alert">{error}</div>{/if}
        {#if draft?.diagnostics.length}<div class="error">{draft.diagnostics.join('\n')} · Incompatible fields are retained; execution may be blocked.</div>{/if}
        {#if draft}<RequestEditor value={draft.value} disabled={busy || loading} onedit={change} />{/if}
        {#if secretNames.length}<Collapsible.Root class="secrets-panel"><Collapsible.Trigger class="secrets-trigger">Secrets · memory only ({secretNames.length})</Collapsible.Trigger><Collapsible.Content><p>These values are used at Send. They are never saved to YAML or history.</p>{#each secretNames as name}<label>{name}<Input type="password" aria-label={`Secret ${name}`} bind:value={secrets[name]} disabled={busy || loading} autocomplete="off" /></label>{/each}</Collapsible.Content></Collapsible.Root>{/if}
        {#if response?.historyWarning}<div class="error">HTTP completed; history could not be saved: {response.historyWarning}</div>{/if}
        </div>
        {#if response}
          <div id="response-pane" class="response-pane" style:height={`${visibleResponseHeight}px`}>
            <Separator class="response-resizer" decorative={false} orientation="horizontal" tabindex={0} aria-label="Resize response panel" aria-orientation="horizontal" aria-controls="response-pane" aria-valuemin={responseMinHeight(responseAreaHeight)} aria-valuemax={responseMaxHeight(responseAreaHeight)} aria-valuenow={visibleResponseHeight} aria-valuetext={`${visibleResponseHeight} pixels`} title="Drag to resize · Double-click to reset" onpointerdown={startResponseResize} onpointermove={resizeResponse} onpointerup={finishResponseResize} onpointercancel={finishResponseResize} onlostpointercapture={finishResponseResize} onkeydown={responseResizeKey} ondblclick={resetResponseHeight} />
            {#key response.id}<ResponsePanel {response} {body} {offset} {chunkLoading} onloadMore={loadMore} />{/key}
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
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) cancelCreation(); }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label={titles[creation]} onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); if (manageWorkspacesOpen) restoreWorkspaceManagerFocus(); else if (creationReturnFocus?.isConnected) creationReturnFocus.focus(); }}>
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
  <Dialog.Root open={true} onOpenChange={(open) => { if (!open && !loading) closingRequestTab = null; }}><Dialog.Content class="modal creation-modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="Save changes before closing?" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { if (loading) event.preventDefault(); }} onOpenAutoFocus={(event) => { event.preventDefault(); document.querySelector<HTMLElement>('.shadcn-modal [autofocus]')?.focus(); }} onCloseAutoFocus={(event) => { event.preventDefault(); if (manageWorkspacesOpen) restoreWorkspaceManagerFocus(); else if (creationReturnFocus?.isConnected) creationReturnFocus.focus(); }}>
    <div class="modal-heading"><Dialog.Title>Save changes before closing?</Dialog.Title><Button variant="ghost" size="icon-sm" aria-label="Close" onclick={() => closingRequestTab = null} disabled={loading}><X aria-hidden="true" /></Button></div>
    <Dialog.Description>Save “{closingTab.draft.value.info?.name ?? 'Untitled'}” before closing its tab?{!closingTab.path ? ' Choose a name and location in the next step.' : ' Your changes will be saved to its current file.'}</Dialog.Description>
    {#if closeTabError}<div class="error" role="alert">{closeTabError}</div>{/if}
    <div class="creation-actions"><Button autofocus variant="ghost" onclick={() => closingRequestTab = null} disabled={loading}>Cancel</Button><Button variant="outline" onclick={discardClosingTab} disabled={loading}>Close without saving</Button><Button onclick={saveClosingTab} disabled={loading || closingTabReadOnly}>{#if loading}<LoaderCircle class="animate-spin" aria-hidden="true" />{/if}Save</Button></div>
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

{#if environmentOpen}<Dialog.Root open={true}><Dialog.Content class="modal shadcn-modal motion-reduce:animate-none" showCloseButton={false} aria-label="Environment editor" onInteractOutside={(event) => event.preventDefault()} onEscapeKeydown={(event) => { event.preventDefault(); closeEnvironment(); }}><div class="modal-heading"><Dialog.Title>Environments</Dialog.Title><Button variant="outline" size="sm" onclick={closeEnvironment} disabled={loading}><X aria-hidden="true" />Close</Button></div>
  <label>baseUrl override<Input aria-label="baseUrl override" bind:value={baseUrl} placeholder="Use the environment value" disabled={loading} /></label>
  <Dialog.Description>Public values are saved to YAML. Secret declarations have no value; supply their values in memory at Send.</Dialog.Description>
  {#if error}<div class="error" role="alert">{error}</div>{/if}
  {#if envDraft}<label>Environment name <Input aria-label="Environment name" value={envDraft.value.name} disabled={loading || collection?.readOnly} oninput={(e) => editDraft(envDraft!, ['name'], e.currentTarget.value)} /></label><Rows rows={envDraft.value.variables ?? []} path={['variables']} mode="variables" environment disabled={loading || collection?.readOnly} onedit={(path, value) => editDraft(envDraft!, path, value)} /><div class="save-actions"><Button variant="outline" size="sm" onclick={saveEnvironment} disabled={loading || !envDraft.edits.length || collection?.readOnly}>{#if environmentSaved && !envDraft.edits.length}<Check class="action-confirmation" aria-hidden="true" />{:else}<Save aria-hidden="true" />{/if}Save environment</Button><Button variant="outline" size="sm" onclick={() => { void discardEnvironment().catch(e => error = String(e)); }} disabled={loading || !envDraft.edits.length}>Discard environment changes</Button></div>{/if}
  <div class="new-environment"><Input aria-label="New environment name" bind:value={newEnvironment} placeholder="New environment name" disabled={loading || collection?.readOnly || !!envDraft?.edits.length} /><Button variant="outline" onclick={createEnvironment} disabled={loading || !newEnvironment || collection?.readOnly || !!envDraft?.edits.length}><Plus aria-hidden="true" />Create environment</Button></div>
</Dialog.Content></Dialog.Root>{/if}

{#if historyOpen}<dialog use:modal class="modal" aria-label="Execution history" oncancel={() => historyOpen = false}><div class="modal-heading"><h2>History · metadata only</h2><div class="save-actions"><Button variant="outline" size="sm" onclick={clearHistory} disabled={!history.entries.length}>Clear history</Button><Button variant="outline" size="sm" onclick={() => historyOpen = false}><X aria-hidden="true" />Close</Button></div></div>
  <p>Last 200 executions. No URLs, credentials, variable values, headers or bodies are stored. Clearing removes all retained records.</p>{#if history.warning}<div class="error">{history.warning}</div>{/if}
  <table><thead><tr><th>When / request</th><th>Method</th><th>Result</th><th>Duration</th><th>Size</th></tr></thead><tbody>{#each history.entries as entry}<tr><td>{new Date(entry.timestamp * 1000).toLocaleString()}<br />{entry.requestPath}</td><td><span class="http-method" data-method={entry.method.toUpperCase()}>{entry.method}</span></td><td>{entry.status ?? entry.outcome}</td><td>{entry.elapsedMs} ms</td><td>{size(entry.bodyBytes)}</td></tr>{/each}</tbody></table>{#if !history.entries.length}<p>No executions recorded yet.</p>{/if}
</dialog>{/if}
