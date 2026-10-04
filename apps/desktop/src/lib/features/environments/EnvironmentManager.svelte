<script lang="ts">
  import { ArrowLeft, Check, Plus, Save } from '@lucide/svelte';
  import Rows from '$lib/components/Rows.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import * as Select from '$lib/components/ui/select';
  import type { Collection } from '$lib/desktop/api';
  import type { Draft } from '$lib/features/requests/draft';

  let { collection, environment, draft, baseUrl, disabled = false, saved = false, error = '', onback, onselect, oncreate, onsave, ondiscard, onedit, onbaseurlchange }: {
    collection: Collection; environment: string; draft: Draft | null; baseUrl: string;
    disabled?: boolean; saved?: boolean; error?: string;
    onback: () => Promise<void>; onselect: (name: string) => Promise<void>; oncreate: () => void;
    onsave: () => Promise<void>; ondiscard: () => void; onedit: (path: string[], value: unknown) => void;
    onbaseurlchange: (value: string) => void;
  } = $props();
  const options = $derived([{ value: '0', label: 'No environment' }, ...collection.environments.map((name, index) => ({ value: String(index + 1), label: name }))]);
  const readOnly = $derived(disabled || collection.readOnly);
  let selection = $state('0');
  $effect(() => { selection = String(collection.environments.indexOf(environment) + 1); });
</script>

<section class="workspace-manager environment-manager" aria-labelledby="environment-manager-heading">
  <Button variant="ghost" size="sm" class="workspace-manager-back" onclick={onback} {disabled}><ArrowLeft aria-hidden="true" />Back to request</Button>
  <div class="workspace-manager-heading">
    <h1 id="environment-manager-heading" tabindex="-1">Manage environments</h1>
    <p>{collection.name} · Edit variables used by your requests.</p>
  </div>
  <div class="environment-manager-toolbar">
    <Select.Root type="single" bind:value={selection} items={options} allowDeselect={false} {disabled} onValueChange={async (value) => { await onselect(collection.environments[Number(value) - 1] ?? ''); selection = String(collection.environments.indexOf(environment) + 1); }}>
      <Select.Trigger aria-label="Environment to edit"><Select.Value /></Select.Trigger>
      <Select.Content>{#each options as item (item.value)}<Select.Item value={item.value} label={item.label}>{item.label}</Select.Item>{/each}</Select.Content>
    </Select.Root>
    <Button id="create-managed-environment" variant="outline" size="sm" disabled={readOnly} onclick={oncreate}><Plus aria-hidden="true" />Create environment</Button>
  </div>
  {#if error}<div class="error" role="alert">{error}</div>{/if}
  {#if draft}
    <div class="environment-manager-fields"><label>Name<Input aria-label="Environment name" value={draft.value.name} disabled={readOnly} oninput={(event) => onedit(['name'], event.currentTarget.value)} /></label></div>
    <section class="config-editor" aria-label="Environment variables">
      <p class="config-section-label">Variables</p>
      <Rows rows={draft.value.variables ?? []} path={['variables']} mode="variables" environment disabled={readOnly} {onedit} />
    </section>
    <div class="save-actions">
      <Button variant="outline" size="sm" onclick={onsave} disabled={readOnly || !draft.edits.length}>{#if saved && !draft.edits.length}<Check class="action-confirmation" aria-hidden="true" />{:else}<Save aria-hidden="true" />{/if}Save environment</Button>
      <Button variant="outline" size="sm" onclick={ondiscard} disabled={disabled || !draft.edits.length}>Discard environment changes</Button>
    </div>
    <p class="environment-manager-note">Secret values are supplied when sending a request and are never saved here.</p>
  {:else}
    <p class="environment-manager-note">{collection.environments.length ? 'Choose an environment to edit its variables.' : 'Create your first environment to add variables.'}</p>
  {/if}
  <div class="environment-manager-fields"><label>baseUrl override<Input aria-label="baseUrl override" value={baseUrl} placeholder="Use the environment value" {disabled} oninput={(event) => onbaseurlchange(event.currentTarget.value)} /></label><p class="environment-manager-note">Applies to the current request in memory.</p></div>
</section>
