<script lang="ts">
  import { onMount } from 'svelte';
  import { Tooltip } from 'bits-ui';
  import { FileText, GitBranch, GitCompareArrows, Minus, Pencil, Plus, RefreshCw, Square, TriangleAlert } from '@lucide/svelte';
  import { api, nextGitOperationId, type Collection, type GitAction, type GitEntry, type GitReply, type GitSide, type GitStatus } from '$lib/desktop/api';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Separator } from '$lib/components/ui/separator';
  import { Textarea } from '$lib/components/ui/textarea';
  import GitDiff from './GitDiff.svelte';
  import { indexGitDiff } from './git-diff';
  import { TREE_ROW_HEIGHT } from './tree';

  let { id, collection, filesVersion, hasDraft, hidden, blocked, operating = $bindable(false) }: {
    id: string;
    collection: Collection;
    filesVersion: number;
    hasDraft: (paths: string[]) => boolean;
    hidden: boolean;
    blocked: boolean;
    operating?: boolean;
  } = $props();
  const listId = $props.id();
  const checkboxTooltip = Tooltip.createTether<string>();

  type Pane = 'filesWidth' | 'filesHeight' | 'commitHeight';
  type PaneSize = { min: number; max: number; value: number };
  type DiffAction = Extract<GitAction, { kind: 'diff' }>;
  const resizeHandleSize = 8;
  const commitMinimum = 74;
  let bodyWidth = $state(0);
  let bodyHeight = $state(0);
  let confirmationHeight = $state(0);
  let preferred = $state<Partial<Record<Pane, number>>>({});
  let drag: { pane: Pane; handle: HTMLElement; pointerId: number; start: number; size: number } | null = null;
  let status = $state<GitStatus | null>(null);
  let preview = $state<{ path: string; side: GitSide; result?: Extract<GitReply, { kind: 'diff' }> } | null>(null);
  let pending = $state<{ paths: Set<string>; staged: boolean } | null>(null);
  let message = $state('');
  let error = $state('');
  let notice = $state('');
  let operation = $state<number | null>(null);
  let readingDiff = $state<DiffAction | null>(null);
  let queuedDiff = $state<DiffAction | null>(null);
  let cancelling = $state(false);
  let observedVersion = $state(-1);
  let confirmation = $state<{ action: GitAction; text: string } | null>(null);
  let disposed = false;

  const compact = $derived(bodyWidth > 0 && bodyWidth <= 630);
  const filesPane = $derived(compact ? 'filesHeight' : 'filesWidth');
  const commitDefault = $derived(commitMinimum + (confirmation ? confirmationHeight + 10 : 0));
  const commitSize = $derived(paneSize(bodyHeight, preferred.commitHeight ?? commitDefault,
    commitDefault, compact ? 128 : 104));
  const reviewHeight = $derived(status?.entries.length ? bodyHeight - commitSize.value - resizeHandleSize : bodyHeight);
  const filesSize = $derived(compact
    ? paneSize(reviewHeight, preferred.filesHeight ?? reviewHeight * .42, 54, 54)
    : paneSize(bodyWidth, preferred.filesWidth ?? Math.max(210, bodyWidth * .30), 160, 240));

  const stale = $derived(!!status && filesVersion !== observedVersion);
  const stagingBlock = $derived(stale ? 'Files changed outside this panel. Refresh Git status.' : status?.mutationBlock?.message ?? null);
  const entries = $derived(status?.entries.slice().sort((a, b) => a.path === b.path ? 0 : a.path < b.path ? -1 : 1) ?? []);
  const selectable = $derived(entries.filter(entry => !entry.blockedReason && !entry.conflict));
  const allBlocked = $derived(stagingBlock ?? (selectable.length ? null : 'There are no eligible changes to stage.'));
  const checkedCount = $derived(selectable.filter(isChecked).length);
  const allChecked = $derived(checkedCount > 0 && checkedCount === selectable.length);
  const staged = $derived(entries.filter(entry => entry.index !== '.' && !entry.conflict));
  const unsupportedStaged = $derived(staged.some(entry => !!entry.blockedReason));
  const current = $derived(status?.entries.find(entry => entry.path === preview?.path));
  const patch = $derived(preview?.result?.text);
  const diff = $derived(patch ? indexGitDiff(patch) : null);
  const available = $derived(!!status && !stale && !blocked && !confirmation);
  const canReview = $derived(available && (operation === null || (!!readingDiff && !cancelling)));
  const canChange = $derived(available && operation === null && !stagingBlock);
  const commitReady = $derived(available && !stagingBlock && !status?.commitBlock && status?.outsideStagedCount === 0
    && staged.length > 0 && !unsupportedStaged && !!message.trim());
  const canCommit = $derived(commitReady && operation === null);

  function paneSize(available: number, requested: number, minimum: number, reserve: number): PaneSize {
    const space = Math.max(0, available - resizeHandleSize);
    const max = space - Math.min(reserve, Math.floor(space / 2));
    const min = Math.min(minimum, max);
    return { min, max, value: Math.min(max, Math.max(min, Math.round(requested))) };
  }

  function pointerPosition(event: PointerEvent, pane: Pane) {
    return pane === 'filesWidth' ? event.clientX : event.clientY;
  }

  function resizePane(pane: Pane, value: number) {
    const { min, max } = pane === 'commitHeight' ? commitSize : filesSize;
    preferred[pane] = Math.min(max, Math.max(min, Math.round(value)));
  }

  function startResize(event: PointerEvent, pane: Pane, size: PaneSize) {
    if (event.button !== 0 || drag?.handle.hasPointerCapture(drag.pointerId)) return;
    event.preventDefault();
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    drag = { pane, handle, pointerId: event.pointerId, start: pointerPosition(event, pane), size: size.value };
  }

  function moveResize(event: PointerEvent) {
    if (drag?.pointerId !== event.pointerId) return;
    const { pane, start, size } = drag;
    const position = pointerPosition(event, pane);
    resizePane(pane, size + (pane === 'commitHeight' ? start - position : position - start));
  }

  function finishResize(event: PointerEvent) {
    if (drag?.pointerId === event.pointerId) drag = null;
  }

  function resizeKey(event: KeyboardEvent, pane: Pane, size: PaneSize) {
    const grow = pane === 'filesWidth' ? 'ArrowRight' : pane === 'filesHeight' ? 'ArrowDown' : 'ArrowUp';
    const shrink = pane === 'filesWidth' ? 'ArrowLeft' : pane === 'filesHeight' ? 'ArrowUp' : 'ArrowDown';
    const sizes: Record<string, number> = {
      [grow]: size.value + 16, [shrink]: size.value - 16, Home: size.min, End: size.max,
    };
    if (!(event.key in sizes)) return;
    event.preventDefault();
    resizePane(pane, sizes[event.key]);
  }

  function isChecked(entry: GitEntry) {
    if (pending?.paths.has(entry.path)) return pending.staged;
    return entry.index !== '.' && !entry.conflict;
  }

  const fileStates = {
    untracked: { kind: 'new', label: 'New file · Untracked', icon: null },
    added: { kind: 'new', label: 'New file · Staged', icon: Plus },
    modified: { kind: 'modified', label: 'Modified file', icon: Pencil },
    deleted: { kind: 'deleted', label: 'Deleted file', icon: Minus },
    conflict: { kind: 'conflict', label: 'Conflict', icon: TriangleAlert },
  } as const;

  function fileState(entry: GitEntry) {
    if (entry.conflict) return fileStates.conflict;
    if (entry.worktree === '?') return fileStates.untracked;
    if (entry.index === 'D' || entry.worktree === 'D') return fileStates.deleted;
    if (entry.index === 'A' || entry.worktree === 'A') return fileStates.added;
    return fileStates.modified;
  }

  onMount(() => {
    void perform({ kind: 'status' });
    return () => {
      disposed = true;
      if (operation !== null) void api.cancelGit(operation).catch(() => {});
    };
  });

  function updateStatus(next: GitStatus | null, version: number) {
    status = next;
    observedVersion = version;
  }

  function refreshPreview(next: GitStatus): DiffAction | undefined {
    const review = preview;
    const entry = next.entries.find(item => item.path === review?.path);
    if (!review?.result || !entry || entry.blockedReason || entry.conflict) {
      preview = null;
      return;
    }
    const side = review.side === 'index' && entry.index === '.' ? 'worktree' :
      review.side === 'worktree' && entry.worktree === '.' ? 'index' : review.side;
    preview = { ...review, side };
    return { kind: 'diff', path: entry.path, side, revision: next.revision };
  }

  async function perform(action: GitAction, keepDiff = false) {
    if (operation !== null || blocked) return;
    const version = filesVersion;
    const id = nextGitOperationId();
    operation = id;
    readingDiff = action.kind === 'diff' ? action : null;
    operating = true;
    if (action.kind === 'stagePaths' || action.kind === 'unstage') {
      pending = { paths: new Set(action.paths), staged: action.kind === 'stagePaths' };
    }
    confirmation = null;
    error = '';
    if (action.kind !== 'diff') notice = '';
    if (action.kind === 'diff' && !keepDiff) preview = { path: action.path, side: action.side };
    let reloadDiff: DiffAction | undefined;
    try {
      const result = await api.git(collection.root, id, action);
      if (disposed) return;
      if (result.kind === 'status') {
        updateStatus(result.status, version);
        reloadDiff = refreshPreview(result.status);
      }
      if (result.kind === 'diff' && action.kind === 'diff' && version === filesVersion && !queuedDiff) {
        preview = { path: action.path, side: action.side, result };
      }
      if (result.kind === 'mutation') {
        const unchanged = result.effect === 'notApplied' && result.status?.revision === status?.revision;
        const keepReview = version === filesVersion && (result.selectionStable || unchanged);
        updateStatus(result.effect === 'unknown' ? null : result.status, version);
        const review = preview;
        const reviewed = status?.entries.find(entry => entry.path === review?.path);
        if (!keepReview || result.issue?.code === 'stale-state' || !status || !review?.result || !reviewed || reviewed.blockedReason || reviewed.conflict) {
          preview = null;
        } else {
          const affected = action.kind === 'stagePaths' || action.kind === 'unstage'
            ? action.paths.includes(review.path) : action.kind === 'stage' && action.path === review.path;
          if (affected && result.effect === 'applied') reloadDiff = refreshPreview(status);
        }
        if (result.effect === 'applied') {
          if (result.oid) {
            message = '';
            notice = `Committed ${result.oid.slice(0, 12)} on ${result.branch}.`;
          }
          if (!result.status) notice = (notice ? notice + ' ' : '') + 'Refresh to see the current status.';
        } else {
          error = result.issue?.message ?? 'Could not complete the Git operation.';
          if (result.effect === 'unknown') error += ' The result is uncertain. Refresh before trying again.';
        }
      }
    } catch (issue) {
      if (disposed) return;
      if (action.kind === 'status' || (typeof issue === 'object' && issue && 'code' in issue && issue.code === 'stale-state')) {
        updateStatus(null, version);
        preview = null;
      }
      if (keepDiff) preview = null;
      error = issue && typeof issue === 'object' && 'message' in issue && typeof issue.message === 'string'
        ? issue.message : 'Git changes could not be loaded. Try again.';
    } finally {
      const next = queuedDiff?.revision === status?.revision ? queuedDiff : reloadDiff;
      queuedDiff = null;
      readingDiff = null;
      operation = null;
      operating = false;
      cancelling = false;
      pending = null;
      if (next && !disposed && !stale) void perform(next, next === reloadDiff);
    }
  }

  export async function cancel() {
    if (operation === null || cancelling) return;
    queuedDiff = null;
    cancelling = true;
    try { await api.cancelGit(operation); }
    catch { cancelling = false; error = 'Cancellation could not be requested. Try again.'; }
  }

  function read(entry: GitEntry, side: GitSide) {
    if (!status || !canReview || entry.blockedReason || entry.conflict) return;
    const action: DiffAction = { kind: 'diff', path: entry.path, side, revision: status.revision };
    if (readingDiff) {
      queuedDiff = readingDiff.path === entry.path && readingDiff.side === side ? null : action;
    } else if (preview?.path !== entry.path || preview.side !== side || !preview.result) {
      void perform(action);
    }
  }

  function setStaged(files: GitEntry[], checked: boolean) {
    if (!status || !canChange) return;
    const paths = files.filter(entry => !entry.blockedReason && !entry.conflict && isChecked(entry) !== checked)
      .map(entry => entry.path);
    if (!paths.length) return;
    const action: GitAction = { kind: checked ? 'stagePaths' : 'unstage', paths, revision: status.revision };
    if (checked && hasDraft(paths)) {
      confirmation = { action, text: 'Some files to stage have unsaved drafts. Switch to the editor to save them, or stage the saved versions and keep your drafts.' };
    } else void perform(action);
  }

  function stage(entry: GitEntry) {
    if (!status || !canChange || preview?.path !== entry.path || preview.side !== 'worktree' || !preview.result?.fileRevision) return;
    const action: GitAction = {
      kind: 'stage', path: entry.path, revision: status.revision,
      fileRevision: preview.result.fileRevision, replacePartial: entry.index !== '.',
    };
    const draft = hasDraft([entry.path]);
    const partial = entry.index !== '.';
    if (draft || partial) {
      confirmation = { action, text: [
        draft ? 'This file has an unsaved draft. Switch to the editor to save it, or stage the version on disk and keep your draft.' : '',
        partial ? 'This will replace the staged version with the full version on disk.' : '',
      ].filter(Boolean).join(' ') };
    } else void perform(action);
  }

  function commit() {
    if (!status || !canCommit) return;
    const paths = staged.map(entry => entry.path);
    const action: GitAction = { kind: 'commit', paths, message, revision: status.revision };
    if (hasDraft(paths)) {
      confirmation = { action, text: 'Some staged files have unsaved drafts. Switch to the editor to save them, or commit the versions in the index and keep your drafts.' };
    } else void perform(action);
  }
</script>

{#snippet paneHandle(pane: Pane, label: string, controls: string, size: PaneSize)}
  <Separator class="git-pane-handle" decorative={false}
    orientation={pane === 'filesWidth' ? 'vertical' : 'horizontal'}
    tabindex={0} aria-label={label} aria-controls={controls}
    aria-valuemin={size.min} aria-valuemax={size.max} aria-valuenow={size.value} aria-valuetext={size.value + ' pixels'}
    title="Drag to resize · Double-click to reset"
    onpointerdown={(event) => startResize(event, pane, size)}
    onpointermove={moveResize}
    onpointerup={finishResize}
    onpointercancel={finishResize}
    onlostpointercapture={finishResize}
    onkeydown={(event) => resizeKey(event, pane, size)}
    ondblclick={() => preferred[pane] = undefined} />
{/snippet}

{#snippet stageCheckbox(id: string, label: string, title: string, reason: string | null,
  checked: () => boolean, toggle: (value: boolean) => void, mixed: () => boolean)}
  <Tooltip.Trigger tether={checkboxTooltip} payload={reason ?? ''} disabled={!reason}>
    {#snippet child({ props })}
      <Checkbox {...props} disabled={false} class="git-stage-check after:-inset-x-1"
        aria-label={label}
        aria-disabled={reason ? true : undefined}
        aria-describedby={reason ? id : undefined}
        title={reason ? undefined : title}
        bind:checked={checked, toggle}
        bind:indeterminate={mixed, () => {}}
        readonly={!canChange || !!reason} />
    {/snippet}
  </Tooltip.Trigger>
  {#if reason}<span {id} hidden>{reason}</span>{/if}
{/snippet}

<section {id} {hidden} class="git-changes" aria-labelledby={listId + '-title'}
  style:--git-resize-size={resizeHandleSize + 'px'}>
  <Tooltip.Provider delayDuration={350}>
  <header class="git-header">
    <div class="git-title">
      <GitCompareArrows size={16} aria-hidden="true" />
      <div class="git-heading">
        <div class="git-heading-line">
          <h2 id={listId + '-title'} tabindex="-1">Git changes</h2>
          <span class="git-collection" title={collection.name}>{collection.name}</span>
        </div>
        {#if status}<code class="git-location" title={status.repoRoot}>{status.repoRoot}</code>{/if}
      </div>
    </div>
    <div class="git-tools">
      {#if status}
        <span class="git-branch" title={(status.branch ?? 'Detached HEAD') + (status.branch && !status.headOid ? ' · No commits yet' : '')}>
          <GitBranch size={13} aria-hidden="true" />
          <strong>{status.branch ?? 'Detached HEAD'}</strong>
        </span>
      {/if}
      {#if operation !== null && !readingDiff}
        <Button variant="ghost" size="icon-sm" aria-label={cancelling ? 'Cancelling Git operation' : 'Cancel Git operation'} title="Cancel Git operation" onclick={cancel} disabled={cancelling}>
          <Square aria-hidden="true" />
        </Button>
      {:else}
        <Button variant="ghost" size="icon-sm" aria-label="Refresh Git status" title="Refresh Git status" onclick={() => perform({ kind: 'status' })} disabled={blocked} aria-disabled={!!readingDiff || undefined}>
          <RefreshCw aria-hidden="true" />
        </Button>
      {/if}
    </div>
  </header>
  {#if error || notice || (!status && operation !== null) || stale || status?.mutationBlock || status?.commitBlock || status?.outsideStagedCount || unsupportedStaged}
    <div class="git-banners">
      {#if error}<p role="alert" class="error">{error}</p>{/if}
      {#if notice}<p role="status">{notice}</p>{/if}
      {#if !status && operation !== null}<p role="status">Reading Git status…</p>{/if}
      {#if stale}<p role="status">Files changed outside this panel. Refresh to review the new state.</p>{/if}
      {#if status?.mutationBlock || status?.commitBlock}<p>{(status.mutationBlock ?? status.commitBlock)?.message}</p>{/if}
      {#if status?.outsideStagedCount}
        <p>{status.outsideStagedCount} staged {status.outsideStagedCount === 1 ? 'file is' : 'files are'} outside this collection. Resolve them in Git before committing here.</p>
      {/if}
      {#if unsupportedStaged}<p>Some staged files are unsupported here. Review them in Git before committing.</p>{/if}
    </div>
  {/if}
  {#if status}
    <div class="git-body" bind:clientWidth={bodyWidth} bind:clientHeight={bodyHeight}>
    <div class="git-workbench" class:compact style:--git-file-size={bodyWidth ? filesSize.value + 'px' : '30%'}>
      <section id={listId + '-files'} class="git-files" aria-labelledby={listId + '-files-title'} style:--git-row-height={TREE_ROW_HEIGHT + 'px'}>
        <div class="git-pane-heading git-files-heading">
          {@render stageCheckbox(listId + '-all-blocked', 'Stage or unstage all eligible changes',
            allChecked ? 'Unstage all eligible changes' : 'Stage all eligible changes', allBlocked,
            () => allChecked, checked => setStaged(selectable, checked), () => !allChecked && checkedCount > 0)}
          <h3 id={listId + '-files-title'}>Changes</h3>
          <span class="git-count">{status.entries.length}</span>
        </div>
        {#if status.entries.length}
          <ul class="git-files-scroll" role="list">
            {#each entries as entry, index (entry.path)}
              {@const change = fileState(entry)}
              {@const stateId = listId + '-state-' + index}
              {@const operationLabel = isChecked(entry) ? 'Unstage' : 'Stage'}
              {@const blocked = entry.conflict ? 'Resolve this file’s Git conflict before staging it.' : entry.blockedReason ?? stagingBlock}
              {@const reviewed = entry.path === (queuedDiff?.path ?? preview?.path)}
              <li class="git-file-row" class:reviewed>
                {@render stageCheckbox(stateId + '-blocked', operationLabel + ' ' + entry.path, operationLabel + ' file',
                  blocked, () => isChecked(entry), checked => setStaged([entry], checked), () => false)}
                <button type="button" class="git-file-name" title={entry.path}
                  aria-label={'Review ' + entry.path}
                  aria-describedby={stateId}
                  aria-current={reviewed ? 'true' : undefined}
                  disabled={!!entry.blockedReason || entry.conflict}
                  aria-disabled={!canReview || undefined}
                  onclick={() => read(entry, entry.index === '.' ? 'worktree' : 'index')}>
                  <span class="git-file-status" data-kind={change.kind} title={change.label} aria-hidden="true">
                    {#if change.icon}<change.icon size={11} strokeWidth={2.5} />
                    {:else}?{/if}
                  </span>
                  <span class="git-file-path">{entry.path}</span>
                </button>
                <span id={stateId} hidden>{change.label}{entry.blockedReason ? ' · ' + entry.blockedReason : ''}</span>
              </li>
            {/each}
          </ul>
        {:else}<p class="git-pane-message">No changes in this collection.</p>{/if}
      </section>
      {@render paneHandle(filesPane, 'Resize files and diff', listId + '-files', filesSize)}
      <section class="git-preview" aria-label="File diff">
        <div class="git-pane-heading git-diff-heading">
          {#if preview && current}
            <div class="git-diff-name"><FileText size={14} aria-hidden="true" /><h3 title={preview.path}>{preview.path}</h3></div>
            {#if current.index !== '.' && current.worktree !== '.'}
              <div class="git-diff-source" role="group" aria-label="Diff source">
                <button type="button" aria-pressed={preview.side === 'index'} aria-disabled={!canReview}
                  onclick={() => current && read(current, 'index')}>Staged</button>
                <button type="button" aria-pressed={preview.side === 'worktree'} aria-disabled={!canReview}
                  onclick={() => current && read(current, 'worktree')}>Worktree</button>
              </div>
            {:else}<span class="git-side">{preview.side === 'index' ? 'Staged diff' : 'Working tree diff'}</span>{/if}
            {#if readingDiff}
              <Button variant="ghost" size="icon-sm" aria-label={cancelling ? 'Cancelling Git operation' : 'Cancel Git operation'} title="Cancel reading diff" onclick={cancel} disabled={cancelling}>
                <Square aria-hidden="true" />
              </Button>
            {:else if preview.side === 'index'}
              <Button variant="ghost" size="icon-sm" aria-label="Unstage file" title="Unstage file" disabled={!canChange || !preview.result} onclick={() => current && setStaged([current], false)}>
                <Minus aria-hidden="true" />
              </Button>
            {:else}
              <Button variant="ghost" size="icon-sm" aria-label="Stage file" title="Stage file" disabled={!canChange || !preview.result} onclick={() => current && stage(current)}>
                <Plus aria-hidden="true" />
              </Button>
            {/if}
          {:else}<h3>Diff</h3>{/if}
        </div>
        {#if preview && current}
          {#if preview.result?.binary}<p class="git-pane-message">Binary file. Text diff unavailable.</p>
          {:else if diff?.lines.length}<GitDiff {diff} />
          {:else if preview.result}<p class="git-pane-message">No textual changes.</p>
          {:else if operation !== null}<p role="status" class="git-pane-message">Reading diff…</p>{/if}
        {:else if status.entries.length}
          <p class="git-pane-message">Select a changed file to review its diff.</p>
        {/if}
      </section>
    </div>
    {#if status.entries.length}
      {@render paneHandle('commitHeight', 'Resize commit panel', listId + '-commit', commitSize)}
      <footer id={listId + '-commit'} class="git-footer" style:height={(bodyHeight ? commitSize.value : commitDefault) + 'px'}>
        {#if confirmation}
          <div class="git-confirmation" bind:offsetHeight={confirmationHeight}>
            <p role="alert">{confirmation.text}</p>
            <div class="save-actions">
              <Button size="sm" onclick={() => confirmation && perform(confirmation.action)} disabled={stale || blocked}>{confirmation.action.kind === 'commit' ? 'Commit staged versions' : 'Stage disk version'}</Button>
              <Button size="sm" variant="outline" onclick={() => confirmation = null}>Go back</Button>
            </div>
          </div>
        {/if}
        <form class="git-commit" onsubmit={(event) => { event.preventDefault(); commit(); }}>
          <label class="sr-only" for={listId + '-commit-message'}>Commit message</label>
          <Textarea id={listId + '-commit-message'} class="git-message" rows={2} placeholder="Commit message" bind:value={message} disabled={(operation !== null && !readingDiff) || !!confirmation} />
          <div class="git-commit-actions">
            <Button type="submit" disabled={!commitReady || (operation !== null && !readingDiff)} aria-disabled={!canCommit || undefined}>{status.branch ? 'Commit to ' + status.branch : 'Commit'}</Button>
            <span role="status">
              {#if pending}{pending.staged ? 'Staging files…' : 'Unstaging files…'}
              {:else if staged.length}{staged.length} {staged.length === 1 ? 'file staged' : 'files staged'}
              {:else}Check a file to stage it{/if}
            </span>
          </div>
        </form>
      </footer>
    {/if}
    </div>
  {/if}
  <Tooltip.Root tether={checkboxTooltip} disableCloseOnTriggerClick>
    {#snippet children({ payload })}
      <Tooltip.Content class="git-stage-tooltip" side="right" sideOffset={8} collisionPadding={8} hideWhenDetached>
        {payload}
      </Tooltip.Content>
    {/snippet}
  </Tooltip.Root>
  </Tooltip.Provider>
</section>

<style>
  .git-changes {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    padding: 0;
  }
  .git-changes[hidden] { display: none; }
  .git-header, .git-title, .git-heading-line, .git-tools, .git-branch, .git-pane-heading, .git-diff-name {
    display: flex;
    align-items: center;
  }
  .git-header {
    justify-content: space-between;
    gap: 12px;
    min-height: 51px;
    padding: 5px 12px 5px 14px;
    border-bottom: 1px solid var(--border);
    background: var(--workbench-tabs);
  }
  .git-title { min-width: 0; gap: 9px; }
  .git-title :global(svg) { flex: none; color: var(--brand-text); }
  .git-heading { min-width: 0; }
  .git-heading-line { gap: 8px; }
  .git-title h2 { flex: none; margin: 0; font-size: 13px; line-height: 1.4; }
  .git-title h2:focus { outline: none; }
  .git-collection, .git-location, .git-branch strong, .git-diff-name h3, .git-file-path {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .git-collection { padding-inline-start: 8px; border-inline-start: 1px solid var(--border); }
  .git-location { display: block; font-size: 11px; line-height: 1.4; }
  .git-collection, .git-location, .git-count, .git-side, .git-commit-actions span {
    color: var(--text-secondary);
  }
  .git-collection, .git-count, .git-side { font-size: 12px; }
  .git-tools { gap: 2px; flex: 0 0 auto; max-width: 48%; }
  .git-branch {
    gap: 5px;
    min-width: 0;
    margin-inline-end: 5px;
    padding: 3px 7px;
    border: 1px solid var(--border);
    border-radius: 5px;
    background: var(--workbench);
    color: var(--text-secondary);
    font-size: 11px;
  }
  .git-branch strong { font-weight: 600; }
  .git-branch :global(svg), .git-diff-name :global(svg) { flex: none; }
  .git-banners { padding: 4px 16px; border-bottom: 1px solid var(--border); }
  .git-changes p { margin: 4px 0; color: var(--text-secondary); font-size: 11px; line-height: 1.6; }
  .git-banners p.error { color: var(--danger); }
  .git-body { display: flex; flex-direction: column; flex: 1; min-width: 0; min-height: 0; }
  .git-workbench {
    display: grid;
    grid-template-columns: var(--git-file-size) var(--git-resize-size) minmax(0, 1fr);
    grid-template-rows: minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  .git-workbench.compact {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: var(--git-file-size) var(--git-resize-size) minmax(0, 1fr);
  }
  .git-body :global(.git-pane-handle) {
    display: grid;
    place-items: center;
    background: var(--surface);
    touch-action: none;
  }
  .git-body :global(.git-pane-handle[data-orientation="vertical"]) {
    width: var(--git-resize-size);
    cursor: col-resize;
  }
  .git-body :global(.git-pane-handle[data-orientation="horizontal"]) {
    height: var(--git-resize-size);
    cursor: row-resize;
  }
  .git-body :global(.git-pane-handle)::after {
    content: '';
    width: 24px;
    height: 2px;
    border-radius: 2px;
    background: var(--text-muted);
    opacity: .6;
    pointer-events: none;
  }
  .git-body :global(.git-pane-handle[data-orientation="vertical"])::after { width: 2px; height: 24px; }
  .git-body :global(.git-pane-handle:hover),
  .git-body :global(.git-pane-handle:active) { background: var(--hover); }
  .git-body :global(.git-pane-handle:focus-visible) { outline: 1px solid var(--text-muted); outline-offset: -1px; }
  .git-files, .git-preview { display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .git-files { background: var(--sidebar); }
  .git-preview { background: var(--workbench); }
  .git-pane-heading {
    flex: none;
    gap: 8px;
    min-height: 36px;
    padding: 0 10px;
    background: var(--surface);
    border-bottom: 1px solid var(--border);
  }
  .git-pane-heading h3 { margin: 0; font-size: 12px; font-weight: 600; }
  .git-files-heading { gap: 7px; padding-inline-start: 12px; }
  .git-count { margin-inline-start: auto; font-variant-numeric: tabular-nums; }
  .git-diff-name { flex: 1; min-width: 0; gap: 7px; }
  .git-diff-name :global(svg) { color: var(--text-muted); }
  .git-side { flex: none; white-space: nowrap; }
  .git-diff-source {
    display: flex;
    align-self: stretch;
    gap: 8px;
    flex: none;
  }
  .git-diff-source button {
    padding: 0 2px;
    border-bottom: 2px solid transparent;
    color: var(--text-secondary);
    font-size: 12px;
  }
  .git-diff-source button[aria-pressed="true"] {
    border-color: var(--brand);
    color: var(--text);
  }
  .git-diff-source button[aria-disabled="true"] { cursor: default; }
  .git-files-scroll { flex: 1; min-height: 0; margin: 0; padding: 4px 6px; overflow: auto; list-style: none; }
  .git-file-row {
    display: flex;
    align-items: center;
    min-height: var(--git-row-height);
    padding-inline: 6px;
    gap: 7px;
    border-radius: 5px;
  }
  .git-file-row:hover, .git-file-row.reviewed { background: var(--hover); }
  .git-files :global(.git-stage-check) {
    width: 16px;
    height: 16px;
    border-color: color-mix(in srgb, var(--text-muted) 65%, var(--border));
    background: var(--workbench);
  }
  .git-files :global(.git-stage-check[aria-checked="true"]) {
    border-color: var(--brand);
    background: var(--brand);
    color: var(--on-accent);
  }
  .git-files :global(.git-stage-check[aria-checked="mixed"]) { color: var(--text); }
  .git-files :global(.git-stage-check[aria-disabled="true"]) { opacity: .5; cursor: not-allowed; }
  .git-files :global(.git-stage-check:focus-visible) {
    box-shadow: none;
    outline: 1px solid var(--text-muted);
    outline-offset: 2px;
  }
  .git-file-name {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 7px;
    min-width: 0;
    min-height: var(--git-row-height);
    text-align: left;
    font-size: 12px;
  }
  .git-changes :global(.git-stage-tooltip) {
    max-width: min(260px, calc(100vw - 40px));
    padding: 6px 8px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--surface);
    color: var(--text);
    box-shadow: 0 4px 16px #0003;
    font-size: 12px;
    line-height: 1.4;
  }
  .git-file-name:disabled { color: var(--text-muted); }
  .git-file-name:focus-visible, .git-diff-source button:focus-visible {
    outline: 1px solid var(--text-muted);
    outline-offset: -1px;
    border-radius: 4px;
  }
  .git-file-status {
    display: inline-grid;
    flex: none;
    place-items: center;
    width: 15px;
    height: 15px;
    border-radius: 4px;
    color: var(--background);
    font-size: 11px;
    font-weight: 700;
    line-height: 1;
  }
  .git-file-status[data-kind="new"] { background: var(--method-patch); }
  .git-file-status[data-kind="modified"] { background: var(--method-put); }
  .git-file-status[data-kind="deleted"], .git-file-status[data-kind="conflict"] { background: var(--danger); }
  .git-changes .git-pane-message { width: auto; margin: 0; padding: 14px 12px; }
  .git-footer {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    flex: none;
    padding: 10px 12px;
    border: 0;
    overflow: auto;
    background: var(--surface);
    color: var(--text);
  }
  .git-confirmation { display: grid; flex: none; gap: 4px; }
  .git-confirmation + .git-commit { margin-top: 10px; }
  .git-commit { display: flex; flex: 1; min-height: 52px; gap: 12px; }
  .git-commit :global(.git-message) {
    flex: 1;
    min-width: 0;
    min-height: 52px;
    padding: 6px 9px;
    field-sizing: fixed;
    resize: none;
    background: var(--workbench);
    color: inherit;
    font-size: 12px;
  }
  .git-commit-actions { display: grid; align-self: end; justify-items: end; gap: 4px; flex: 0 0 auto; }
  .git-commit-actions span { font-size: 11px; }
</style>
