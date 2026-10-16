<script lang="ts">
  import { tick } from 'svelte';
  import Rows from '$lib/components/Rows.svelte';
  import type { VariablePreview } from '$lib/desktop/api';
  import type { Config } from './draft';
  import AuthFields from '$lib/components/AuthFields.svelte';
  import { Textarea } from '$lib/components/ui/textarea';
  import { Button } from '$lib/components/ui/button';
  import * as Select from '$lib/components/ui/select';
  import * as Tabs from '$lib/components/ui/tabs';
  let { value, requestId, bodyEditorReset = 0, disabled = false, onedit, variables = [], variableIssue = '', variablesLoading = false,
    providedSecrets = [], onvariableedit }: {
    value: Config; requestId: number; bodyEditorReset?: number; disabled?: boolean;
    onedit: (path: string[], value: unknown) => void; variables?: VariablePreview[]; variableIssue?: string; variablesLoading?: boolean; providedSecrets?: string[];
    onvariableedit: (name: string, value: string) => void;
  } = $props();
  let tab = $state('Params');
  let editorAttempt = $state(0);
  async function addVariable(name: string) {
    if (disabled) return;
    const rows = Array.isArray(value.runtime?.variables) ? value.runtime.variables : [];
    if (name) onedit(['runtime', 'variables'], [...rows, { name, value: '' }]);
    tab = 'Variables';
    await tick();
    document.querySelector<HTMLInputElement>(`input[aria-label="variables ${name ? 'value' : 'name'} ${rows.length + 1}"]`)?.focus();
  }
  const bodyType = $derived(value.http?.body?.type ?? 'none');
  const bodyData = $derived(typeof value.http?.body?.data === 'string' ? value.http.body.data : '');
  const bodyTypes = ['none', 'json', 'text', 'xml'];
  const bodyOptions = $derived([...(!bodyTypes.includes(bodyType) ? [{ value: bodyType, label: `${bodyType} (retained)` }] : []), ...bodyTypes.map(value => ({ value, label: value }))]);
  const sections = $derived([
    { name: 'Params', count: value.http?.params?.length ?? 0 },
    { name: 'Auth', count: 0 },
    { name: 'Headers', count: value.http?.headers?.length ?? 0 },
    { name: 'Body', count: 0 },
    { name: 'Variables', count: value.runtime?.variables?.length ?? 0 },
  ]);
  function setBody(type: string) {
    if (type === 'none') onedit(['http', 'body'], null);
    else if (['json', 'text', 'xml'].includes(bodyType)) onedit(['http', 'body', 'type'], type);
    else onedit(['http', 'body'], { type, data: '' });
  }
</script>

<section class="config-editor" aria-label="Request configuration">
  <Tabs.Root bind:value={tab} class="gap-0"><Tabs.List variant="line" aria-label="Configuration sections" class="config-tabs w-full justify-start border-b border-border">{#each sections as section (section.name)}<Tabs.Trigger class="config-tab flex-none" value={section.name}>{section.name}{#if section.count}<span class="config-count">{section.count}</span>{/if}</Tabs.Trigger>{/each}</Tabs.List>
  <Tabs.Content value="Params"><p class="config-section-label">Query Params</p><Rows {variables} {onvariableedit} rows={value.http?.params ?? []} path={['http', 'params']} mode="params" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Headers"><p class="config-section-label">Request Headers</p><Rows {variables} {onvariableedit} rows={value.http?.headers ?? []} path={['http', 'headers']} mode="headers" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Variables"><p class="config-section-label">Request Variables</p><Rows {variables} {onvariableedit} rows={value.runtime?.variables ?? []} path={['runtime', 'variables']} mode="variables" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Body">
    <div class="editor-fields"><div class="editor-select"><label for="request-body-type">Body type</label><Select.Root type="single" value={bodyType} items={bodyOptions} allowDeselect={false} {disabled} onValueChange={setBody}><Select.Trigger id="request-body-type" aria-label="Body type"><Select.Value /></Select.Trigger><Select.Content>{#each bodyOptions as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>
    {#if tab === 'Body' && ['json', 'text', 'xml'].includes(bodyType)}
      {#key `${requestId}:${bodyEditorReset}:${editorAttempt}`}
        {#await import('./BodyEditor.svelte')}
          <Textarea class="min-h-32 font-mono" aria-label="Request body" value={bodyData} {disabled} spellcheck="false" oninput={(e) => onedit(['http', 'body', 'data'], e.currentTarget.value)} />
        {:then editor}
          <editor.default value={bodyData} {bodyType} {disabled} {variables} {providedSecrets} {variableIssue} {variablesLoading}
            {onvariableedit} onaddvariable={addVariable} onchange={(text) => onedit(['http', 'body', 'data'], text)} />
        {:catch}
          <p role="alert">Could not load the body editor. <Button variant="outline" size="sm" onclick={() => editorAttempt++}>Retry editor</Button></p>
          <Textarea class="min-h-32 font-mono" aria-label="Request body" value={bodyData} {disabled} spellcheck="false" oninput={(e) => onedit(['http', 'body', 'data'], e.currentTarget.value)} />
        {/await}
      {/key}
      {#if variableIssue}<p role="status">{variableIssue}</p>{/if}
    {:else if bodyType !== 'none' && !['json', 'text', 'xml'].includes(bodyType)}<p>This body is retained. Select a supported type to replace it.</p>{/if}</div>
</Tabs.Content>
  <Tabs.Content value="Auth">
    <AuthFields auth={value.http?.auth} path={['http', 'auth']} {disabled} {variables} {onvariableedit} {onedit} />
  </Tabs.Content></Tabs.Root>
</section>
