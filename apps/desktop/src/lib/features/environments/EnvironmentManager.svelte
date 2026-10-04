<script lang="ts">
  import { ArrowLeft, Check, Database, Pencil, Plus, Search } from '@lucide/svelte';
  import Rows from '$lib/components/Rows.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import type { Collection } from '$lib/desktop/api';
  import type { Draft } from '$lib/features/requests/draft';

  let { collection, environment, draft, disabled = false, saved = false, error = '', onback, onselect, oncreate, onsave, ondiscard, onedit }: {
    collection: Collection; environment: string; draft: Draft | null;
    disabled?: boolean; saved?: boolean; error?: string;
    onback: () => Promise<void>; onselect: (name: string) => Promise<void>; oncreate: () => void;
    onsave: () => Promise<void>; ondiscard: () => void; onedit: (path: string[], value: unknown) => void;
  } = $props();
  let query = $state('');
  const environments = $derived(collection.environments.filter(name => name.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())));
  const readOnly = $derived(disabled || collection.readOnly);
</script>

<section class="environment-manager" aria-labelledby="environment-manager-heading">
  <header class="environment-manager-heading">
    <h1 id="environment-manager-heading" tabindex="-1"><Database size={14} aria-hidden="true" />Environments</h1>
    <span class="environment-collection" title={collection.root}>{collection.name}</span>
    <Button variant="ghost" size="sm" onclick={onback} {disabled}><ArrowLeft aria-hidden="true" />Back to request</Button>
  </header>
  <nav class="environment-sidebar" aria-label={`Environments in ${collection.name}`}>
    <div class="environment-list-heading"><span>Environments</span><Button id="create-managed-environment" variant="ghost" size="icon-sm" aria-label="Create environment" title="Create environment" disabled={readOnly} onclick={oncreate}><Plus aria-hidden="true" /></Button></div>
    <div class="environment-search"><Search size={14} aria-hidden="true" /><Input type="search" aria-label="Search environments" placeholder="Search environments…" bind:value={query} /></div>
    <ul class="environment-list">
      {#each environments as name (name)}
        <li><Button variant="ghost" class="environment-list-item" aria-current={name === environment ? 'page' : undefined} title={name} {disabled} onclick={() => onselect(name)}><span>{name}</span>{#if name === environment}<Check size={14} aria-label="Selected environment" />{/if}</Button></li>
      {/each}
    </ul>
    {#if !environments.length}<p class="environment-list-empty">{collection.environments.length ? 'No environments match your search.' : 'No environments in this collection.'}</p>{/if}
  </nav>
  <div class="environment-details">
    {#if error}<div class="error" role="alert">{error}</div>{/if}
    {#if draft}
      <div class="environment-editor-heading"><Database size={14} aria-hidden="true" /><Input class="environment-name" aria-label="Environment name" value={draft.value.name} disabled={readOnly} oninput={(event) => onedit(['name'], event.currentTarget.value)} /><Pencil size={13} aria-hidden="true" />{#if draft.edits.length}<span class="environment-manager-note">Unsaved changes</span>{/if}</div>
      <section class="config-editor" aria-label="Environment variables">
        <div class="environment-variables-heading">Variables <span>{draft.value.variables?.length ?? 0}</span></div>
        <Rows rows={draft.value.variables ?? []} path={['variables']} mode="variables" environment disabled={readOnly} {onedit} />
      </section>
      <div class="save-actions">
        <Button size="sm" aria-label="Save environment" onclick={onsave} disabled={readOnly || !draft.edits.length}>{#if saved && !draft.edits.length}<Check class="action-confirmation" aria-hidden="true" />{/if}Save</Button>
        <Button variant="ghost" size="sm" aria-label="Discard environment changes" onclick={ondiscard} disabled={disabled || !draft.edits.length}>Reset</Button>
      </div>
      <p class="environment-manager-note">{collection.readOnly ? 'This collection is read-only. ' : ''}Secret values are supplied when sending a request and are never saved here.</p>
    {:else}
      <div class="environment-editor-empty"><Database size={24} aria-hidden="true" /><h2>{collection.environments.length ? 'Choose an environment' : 'Create your first environment'}</h2><p>Manage variables for {collection.name}.</p>{#if !collection.environments.length}<Button variant="outline" size="sm" disabled={readOnly} onclick={oncreate}><Plus aria-hidden="true" />Create environment</Button>{/if}</div>
    {/if}
  </div>
</section>
