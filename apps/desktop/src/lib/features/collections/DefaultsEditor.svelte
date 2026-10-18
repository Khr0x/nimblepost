<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { api, type Collection, type DefaultsView } from '$lib/desktop/api';
  import { draftFrom, editDraft, type Draft } from '../requests/draft';
  import { defaultsProvenance, editableDefaults } from './defaults';
  import Rows from '$lib/components/Rows.svelte';
  import AuthFields from '$lib/components/AuthFields.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Dialog from '$lib/components/ui/dialog';
  import * as Tabs from '$lib/components/ui/tabs';
  let { collection, folder, onclose, onsaved }: {
    collection: Collection; folder: string; onclose: () => void; onsaved: () => void;
  } = $props();
  let view = $state<DefaultsView | null>(null), draft = $state<Draft | null>(null);
  let pending = $state(true), error = $state(''), notice = $state(''), closing = $state(false), tab = $state('Headers');
  const readOnly = $derived(pending || collection.readOnly || (!!draft && !editableDefaults(draft.value)));
  const dirty = $derived(!!draft?.edits.length);
  const rowsEditable = (rows: unknown) => rows === undefined || (Array.isArray(rows) && rows.every(row => row && !Array.isArray(row) && typeof row === 'object' && (row.name === undefined || typeof row.name === 'string') && (row.value === undefined || typeof row.value === 'string')));
  const variablesEditable = (rows: unknown) => rows === undefined || (Array.isArray(rows) && rows.every(row => row && typeof row === 'object' && !Array.isArray(row) && (row.name === undefined || typeof row.name === 'string') && (row.value?.type !== 'string' || typeof row.value.data === 'string')));
  const provenance = $derived(view && draft ? defaultsProvenance(view.ancestors, view.path, draft.value) : []);
  onMount(() => { void reload(); });
  async function restoreFocus(focused: Element | null) {
    await tick();
    if (focused instanceof HTMLElement && focused.isConnected && !focused.matches(':disabled')) focused.focus();
    else document.getElementById('defaults-close')?.focus();
  }
  async function reload() {
    const focused = document.activeElement;
    pending = true; error = ''; notice = '';
    try { view = await api.defaults(folder); draft = draftFrom(view.document); closing = false; }
    catch (e) { error = String(e); }
    finally { pending = false; await restoreFocus(focused); }
  }
  function edit(path: string[], value: unknown) { if (draft && !readOnly) { editDraft(draft, path, value); notice = ''; } }
  export async function requestClose() {
    if (pending) return;
    if (dirty) { closing = true; await tick(); document.getElementById('defaults-keep-editing')?.focus(); }
    else onclose();
  }
  async function save(close = false) {
    if (!view || !draft || readOnly || !dirty) return;
    const focused = document.activeElement;
    let closed = false;
    pending = true; error = ''; notice = '';
    try {
      const saved = view.missing ? await api.createFolderDefaults(folder, draft.edits) : await api.save(draft.revision, draft.edits);
      draft = draftFrom(saved); view = { ...view, document: saved, missing: false };
      notice = 'Defaults saved.'; closing = false; onsaved();
      if (close) { closed = true; onclose(); }
    } catch (e) { error = `${String(e)}. Your changes are preserved. Reload from disk to discard them and read the current file.`; }
    finally {
      pending = false;
      if (!closed) await restoreFocus(focused);
    }
  }
  function shortcut(event: KeyboardEvent) {
    if ((event.metaKey || event.ctrlKey) && event.key === 's') { event.preventDefault(); void save(); }
  }
</script>

<svelte:window onkeydown={shortcut} />
<Dialog.Root open={true}><Dialog.Content class="defaults-modal sm:max-w-4xl motion-reduce:animate-none" showCloseButton={false}
  aria-label="Edit defaults" onInteractOutside={(event) => event.preventDefault()}
  onEscapeKeydown={(event) => { event.preventDefault(); requestClose(); }} onCloseAutoFocus={(event) => event.preventDefault()}>
  <Dialog.Header><Dialog.Title>{folder ? 'Folder defaults' : 'Collection defaults'}</Dialog.Title>
    <Dialog.Description>{collection.name} / {folder ? `${folder}/folder.yml` : 'opencollection.yml'}. Settings apply to requests below this scope.</Dialog.Description></Dialog.Header>
  {#if error}<p class="error" role="alert">{error}</p>{/if}
  {#if pending}<p role="status">Loading…</p>{/if}
  {#if draft && view}
    {#if view.missing}<p>There is no folder.yml here. Save creates it; opening or discarding does not write a file.</p>{/if}
    {#if collection.readOnly}<p role="status">This collection is read-only.</p>{/if}
    {#if !editableDefaults(draft.value)}<p role="alert">The defaults structure is retained but cannot be edited here. Correct it in YAML and reload.</p>{/if}
    {#if draft.diagnostics.length}<p role="alert">{draft.diagnostics.join(' · ')}</p>{/if}
    <div class="config-editor"><Tabs.Root bind:value={tab} class="gap-0">
      <Tabs.List variant="line" aria-label="Defaults sections"><Tabs.Trigger value="Headers">Headers</Tabs.Trigger><Tabs.Trigger value="Auth">Auth</Tabs.Trigger><Tabs.Trigger value="Variables">Variables</Tabs.Trigger><Tabs.Trigger value="Sources">Sources</Tabs.Trigger></Tabs.List>
      <Tabs.Content value="Headers">{#if rowsEditable(draft.value.request?.headers)}<Rows rows={draft.value.request?.headers ?? []} path={['request', 'headers']} mode="headers" disabled={readOnly} onedit={edit} />{:else}<p>These headers are retained but cannot be edited here. Correct their structure in YAML and reload.</p>{/if}</Tabs.Content>
      <Tabs.Content value="Auth"><AuthFields auth={draft.value.request?.auth} path={['request', 'auth']} idPrefix="defaults-auth" inDialog disabled={readOnly} onedit={edit} /></Tabs.Content>
      <Tabs.Content value="Variables">{#if variablesEditable(draft.value.request?.variables)}<Rows rows={draft.value.request?.variables ?? []} path={['request', 'variables']} mode="variables" disabled={readOnly} onedit={edit} />{:else}<p>These variables are retained but cannot be edited here. Correct their structure in YAML and reload.</p>{/if}</Tabs.Content>
      <Tabs.Content value="Sources"><p>Sources run from collection to nearest folder. Local fields override inherited ones. Values are not resolved here; environment and request scopes can override these defaults.</p>
        <ul class="defaults-source-files">{#each [...view.ancestors.map(source => source.path), view.path] as path}<li>{path}{path === view.path ? ' (editing)' : ''}</li>{/each}</ul>
        {#if provenance.length}<table class="defaults-sources"><thead><tr><th>Section</th><th>Name</th><th>File</th><th>State</th></tr></thead><tbody>{#each provenance as row}<tr><td>{row.section}</td><td>{row.name}</td><td>{row.path}</td><td>{row.state}</td></tr>{/each}</tbody></table>{:else}<p>No headers, authentication or variables are configured in these scopes.</p>{/if}
        <p>A disabled local header group suppresses inheritance. A disabled variable allows the inherited variable to remain active. Reload rereads ancestor files.</p>
      </Tabs.Content>
    </Tabs.Root></div>
    <p role="status">{notice || (dirty ? 'Unsaved changes' : 'No unsaved changes')}</p>
    {#if closing}<div class="defaults-close-confirm" role="group" aria-label="Unsaved defaults"><p>Save or discard your defaults changes before closing.</p><Button id="defaults-keep-editing" variant="ghost" onclick={async () => { closing = false; await restoreFocus(null); }} disabled={pending}>Keep editing</Button><Button variant="outline" onclick={onclose} disabled={pending}>Discard and close</Button><Button onclick={() => save(true)} disabled={readOnly}>Save and close</Button></div>
    {:else}<Dialog.Footer><Button id="defaults-close" variant="ghost" onclick={requestClose} disabled={pending}>Close</Button><Button variant="outline" onclick={reload} disabled={pending}>{dirty ? 'Discard and reload' : 'Reload from disk'}</Button><Button aria-label="Save defaults" onclick={() => save()} disabled={readOnly || !dirty}>Save</Button></Dialog.Footer>{/if}
  {:else if !pending}<Dialog.Footer><Button id="defaults-close" onclick={onclose}>Close</Button><Button variant="outline" onclick={reload}>Retry</Button></Dialog.Footer>{/if}
</Dialog.Content></Dialog.Root>
