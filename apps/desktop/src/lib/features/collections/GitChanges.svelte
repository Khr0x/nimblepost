<script lang="ts">
  import { onMount } from 'svelte';
  import { api, nextGitOperationId, type Collection, type GitAction, type GitEntry, type GitReply, type GitSide, type GitStatus } from '$lib/desktop/api';
  import { Button } from '$lib/components/ui/button';

  let { collection, filesVersion, hasDraft, onclose }: {
    collection: Collection;
    filesVersion: number;
    hasDraft: (paths: string[]) => boolean;
    onclose: () => void;
  } = $props();

  let status = $state<GitStatus | null>(null);
  let preview = $state<({ path: string; side: GitSide } & Extract<GitReply, { kind: 'diff' }>) | null>(null);
  let chosen = $state<string[]>([]);
  let message = $state('');
  let error = $state('');
  let notice = $state('');
  let operation = $state<number | null>(null);
  let cancelling = $state(false);
  let observedVersion = $state(-1);
  let confirmation = $state<{ action: GitAction; text: string } | null>(null);
  let disposed = false;

  const stale = $derived(!!status && filesVersion !== observedVersion);
  const staged = $derived(status?.entries.filter(entry => entry.index !== '.' && !entry.conflict) ?? []);
  const current = $derived(status?.entries.find(entry => entry.path === preview?.path));
  const canChange = $derived(!!status && !stale && !status.mutationBlock && operation === null && !confirmation);
  const canCommit = $derived(canChange && !status?.commitBlock && status?.outsideStagedCount === 0
    && staged.length > 0 && staged.length === chosen.length && !!message.trim());

  const labels: Record<string, string> = { '.': '—', '?': 'Untracked', A: 'Added', M: 'Modified', D: 'Deleted', T: 'Type changed', U: 'Conflict' };
  const label = (code: string) => labels[code] ?? code;

  onMount(() => {
    void perform({ kind: 'status' });
    return () => {
      disposed = true;
      if (operation !== null) void api.cancelGit(operation).catch(() => {});
    };
  });

  function display(node: HTMLDialogElement) {
    node.showModal();
    node.querySelector('h2')?.focus();
    return { destroy: () => node.close() };
  }

  function updateStatus(next: GitStatus | null, version: number) {
    status = next;
    observedVersion = version;
    preview = null;
    chosen = [];
    confirmation = null;
  }

  async function perform(action: GitAction) {
    if (operation !== null) return;
    const version = filesVersion;
    const id = nextGitOperationId();
    operation = id;
    confirmation = null;
    error = '';
    notice = '';
    if (action.kind === 'status' || action.kind === 'diff') preview = null;
    try {
      const result = await api.git(collection.root, id, action);
      if (disposed) return;
      if (result.kind === 'status') updateStatus(result.status, version);
      if (result.kind === 'diff' && action.kind === 'diff' && version === filesVersion) {
        preview = { ...result, path: action.path, side: action.side };
      }
      if (result.kind === 'mutation') {
        updateStatus(result.effect === 'unknown' ? null : result.status, version);
        if (result.effect === 'applied') {
          if (result.oid) {
            message = '';
            notice = `Committed ${result.oid.slice(0, 12)} on ${result.branch}.`;
          } else {
            notice = action.kind === 'stage' ? 'File staged.' : 'File unstaged.';
          }
          if (!result.status) notice += ' Refresh to see the current status.';
        } else {
          error = result.issue?.message ?? 'Could not complete the Git operation.';
          if (result.effect === 'unknown') error += ' The result is uncertain. Refresh before trying again.';
        }
      }
    } catch (issue) {
      if (disposed) return;
      if (action.kind === 'status' || (typeof issue === 'object' && issue && 'code' in issue && issue.code === 'stale-state')) {
        updateStatus(null, version);
      }
      error = issue && typeof issue === 'object' && 'message' in issue && typeof issue.message === 'string'
        ? issue.message : 'Git changes could not be loaded. Try again.';
    } finally {
      operation = null;
      cancelling = false;
    }
  }

  async function cancel() {
    if (operation === null || cancelling) return;
    cancelling = true;
    try { await api.cancelGit(operation); }
    catch { cancelling = false; error = 'Cancellation could not be requested. Try again.'; }
  }

  function close() { if (operation === null) onclose(); }
  export function requestClose() { if (operation === null) close(); else void cancel(); }

  function read(entry: GitEntry, side: GitSide) {
    if (status && !stale) void perform({ kind: 'diff', path: entry.path, side, revision: status.revision });
  }

  function stage(entry: GitEntry) {
    if (!status || !preview?.fileRevision || !canChange) return;
    const action: GitAction = {
      kind: 'stage', path: entry.path, revision: status.revision,
      fileRevision: preview.fileRevision, replacePartial: entry.index !== '.',
    };
    const draft = hasDraft([entry.path]);
    const partial = entry.index !== '.';
    if (draft || partial) {
      confirmation = { action, text: [
        draft ? 'This file has an unsaved draft. Close Git changes to save it in the editor, or stage the version on disk and keep your draft.' : '',
        partial ? 'This will replace the staged version with the full version on disk.' : '',
      ].filter(Boolean).join(' ') };
    } else void perform(action);
  }

  function commit() {
    if (!status || !canCommit) return;
    const action: GitAction = { kind: 'commit', paths: chosen, message, revision: status.revision };
    if (hasDraft(chosen)) {
      confirmation = { action, text: 'Some selected files have unsaved drafts. Close Git changes to save them in the editor, or commit the versions in the index and keep your drafts.' };
    } else void perform(action);
  }
</script>

<dialog class="modal git-changes" aria-labelledby="git-changes-title" use:display
  oncancel={(event) => { event.preventDefault(); close(); }}>
  <header class="modal-heading">
    <h2 id="git-changes-title" tabindex="-1">Git changes · {collection.name}</h2>
    <div class="save-actions">
      {#if operation !== null}
        <Button variant="outline" size="sm" onclick={cancel} disabled={cancelling}>{cancelling ? 'Cancelling…' : 'Cancel operation'}</Button>
      {:else}
        <Button variant="outline" size="sm" onclick={() => perform({ kind: 'status' })}>Refresh</Button>
      {/if}
      <Button variant="outline" size="sm" onclick={close} disabled={operation !== null}>Close</Button>
    </div>
  </header>
  {#if error}<p role="alert" class="error">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  {#if !status && operation !== null}<p role="status">Reading Git status…</p>{/if}
  {#if status}
    <div class="git-location">
      <span><strong>{status.branch ?? 'Detached HEAD'}</strong>{#if status.branch && !status.headOid} · No commits yet{/if}</span>
      <code title={status.repoRoot}>{status.repoRoot}</code>
    </div>
    {#if stale}<p role="status">Files changed outside this panel. Refresh to review the new state.</p>{/if}
    {#if status.mutationBlock || status.commitBlock}<p>{(status.mutationBlock ?? status.commitBlock)?.message}</p>{/if}
    {#if status.outsideStagedCount}
      <p>{status.outsideStagedCount} staged {status.outsideStagedCount === 1 ? 'file is' : 'files are'} outside this collection. Resolve them in Git before committing here.</p>
    {/if}
    {#if status.entries.length}
      <div class="git-files">
        <table>
          <thead><tr><th scope="col">Commit</th><th scope="col">File</th><th scope="col">Index</th><th scope="col">Working tree</th></tr></thead>
          <tbody>
          {#each status.entries as entry (entry.path)}
            <tr>
              <td>
                {#if entry.index !== '.' && entry.supported && !entry.conflict}
                  <input type="checkbox" aria-label={`Include ${entry.path} in commit`} value={entry.path} bind:group={chosen} disabled={!canChange} />
                {/if}
              </td>
              <th scope="row"><code>{entry.path}</code>{#if !entry.supported} <small>Unsupported in Git panel</small>{/if}</th>
              <td>
                {#if entry.index !== '.' && entry.supported && !entry.conflict}
                  <Button size="sm" variant="ghost" aria-label={`View staged diff for ${entry.path}`} disabled={operation !== null || stale || !!confirmation} onclick={() => read(entry, 'index')}>{label(entry.index)}</Button>
                {:else}{entry.conflict ? 'Conflict' : label(entry.index)}{/if}
              </td>
              <td>
                {#if entry.worktree !== '.' && entry.supported && !entry.conflict}
                  <Button size="sm" variant="ghost" aria-label={`View working tree diff for ${entry.path}`} disabled={operation !== null || stale || !!confirmation} onclick={() => read(entry, 'worktree')}>{label(entry.worktree)}</Button>
                {:else}{entry.conflict ? 'Conflict' : label(entry.worktree)}{/if}
              </td>
            </tr>
          {/each}
          </tbody>
        </table>
      </div>
    {:else}<p>No changes in this collection.</p>{/if}
    {#if preview && current && !stale}
      <section class="git-preview" aria-label="File diff">
        <div class="modal-heading">
          <h3><code>{preview.path}</code> · {preview.side === 'index' ? 'Index diff' : 'Working tree diff'}</h3>
          {#if preview.side === 'index'}
            <Button variant="outline" size="sm" disabled={!canChange} onclick={() => status && current && perform({ kind: 'unstage', path: current.path, revision: status.revision })}>Unstage file</Button>
          {:else}
            <Button variant="outline" size="sm" disabled={!canChange} onclick={() => current && stage(current)}>Stage file</Button>
          {/if}
        </div>
        {#if preview.binary}<p>Binary file. Text diff unavailable.</p>
        {:else}
          <!-- svelte-ignore a11y_no_noninteractive_tabindex (keyboard access to the scrollable patch) -->
          <pre tabindex="0" aria-label="Diff contents">{preview.text || 'No textual changes.'}</pre>
        {/if}
      </section>
    {/if}
    {#if confirmation}
      <div class="git-confirmation">
        <p role="alert">{confirmation.text}</p>
        <div class="save-actions">
          <Button size="sm" onclick={() => confirmation && perform(confirmation.action)} disabled={stale}>{confirmation.action.kind === 'commit' ? 'Commit staged versions' : 'Stage disk version'}</Button>
          <Button size="sm" variant="outline" onclick={() => confirmation = null}>Go back</Button>
        </div>
      </div>
    {/if}
    {#if staged.length && status.branch}
      <form class="git-commit" onsubmit={(event) => { event.preventDefault(); commit(); }}>
        <label for="git-commit-message">Commit message</label>
        <textarea id="git-commit-message" rows="2" bind:value={message} disabled={operation !== null || !!confirmation}></textarea>
        <div class="save-actions">
          <Button type="submit" size="sm" disabled={!canCommit}>Commit to {status.branch}</Button>
          <span>{chosen.length} of {staged.length} staged files selected</span>
        </div>
      </form>
    {/if}
  {/if}
</dialog>

<style>
  .git-changes { display: grid; gap: 12px; }
  .git-changes:not([open]) { display: none; }
  h2, h3, p { margin: 0; }
  .git-location { display: flex; gap: 6px 16px; flex-wrap: wrap; }
  .git-location code, th code, h3 code { overflow-wrap: anywhere; white-space: pre-wrap; }
  .git-files { max-height: 26vh; overflow: auto; border: 1px solid var(--border); border-radius: 6px; }
  table { border-collapse: collapse; width: 100%; text-align: left; }
  th, td { padding: 3px 8px; border-bottom: 1px solid var(--border); }
  td:first-child { text-align: center; }
  tbody th { width: 100%; font-weight: normal; }
  thead th { position: sticky; top: 0; background: var(--surface); white-space: nowrap; }
  .git-preview, .git-commit, .git-confirmation { display: grid; gap: 8px; }
  pre, textarea { padding: 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--surface); }
  pre { margin: 0; overflow: auto; max-height: 24vh; }
  textarea { color: inherit; resize: vertical; }
  tbody small { display: block; }
</style>
