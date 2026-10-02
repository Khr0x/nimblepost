<script lang="ts">
  import Rows from '$lib/components/Rows.svelte';
  import type { Config } from './draft';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import * as Select from '$lib/components/ui/select';
  import * as Tabs from '$lib/components/ui/tabs';
  let { value, disabled = false, onedit }: { value: Config; disabled?: boolean; onedit: (path: string[], value: unknown) => void } = $props();
  let tab = $state('Params');
  const auth = $derived(value.http?.auth);
  const authType = $derived(typeof auth === 'string' || !auth ? 'inherit' : auth.type);
  const bodyType = $derived(value.http?.body?.type ?? 'none');
  const authTypes = ['inherit', 'basic', 'bearer', 'apikey'];
  const bodyTypes = ['none', 'json', 'text', 'xml'];
  const authOptions = $derived([...(!authTypes.includes(authType) ? [{ value: authType, label: `${authType} (retained)` }] : []), ...authTypes.map((value, index) => ({ value, label: ['Inherit', 'Basic', 'Bearer', 'API Key'][index] }))]);
  const bodyOptions = $derived([...(!bodyTypes.includes(bodyType) ? [{ value: bodyType, label: `${bodyType} (retained)` }] : []), ...bodyTypes.map(value => ({ value, label: value }))]);
  const placements = [{ value: 'header', label: 'Header' }, { value: 'query', label: 'Query' }];
  const sections = $derived([
    { name: 'Params', count: value.http?.params?.length ?? 0 },
    { name: 'Auth', count: 0 },
    { name: 'Headers', count: value.http?.headers?.length ?? 0 },
    { name: 'Body', count: 0 },
    { name: 'Variables', count: value.runtime?.variables?.length ?? 0 },
  ]);
  function setAuth(type: string) {
    const next = type === 'inherit' ? 'inherit' : type === 'basic' ? { type, username: '', password: '{{password}}' } : type === 'bearer' ? { type, token: '{{token}}' } : { type, key: 'X-API-Key', value: '{{apiKey}}', placement: 'header' };
    onedit(['http', 'auth'], next);
  }
  function setBody(type: string) {
    if (type === 'none') onedit(['http', 'body'], null);
    else if (['json', 'text', 'xml'].includes(bodyType)) onedit(['http', 'body', 'type'], type);
    else onedit(['http', 'body'], { type, data: '' });
  }
</script>

<section class="config-editor" aria-label="Request configuration">
  <Tabs.Root bind:value={tab} class="gap-0"><Tabs.List variant="line" aria-label="Configuration sections" class="config-tabs w-full justify-start border-b border-border">{#each sections as section (section.name)}<Tabs.Trigger class="config-tab flex-none" value={section.name}>{section.name}{#if section.count}<span class="config-count">{section.count}</span>{/if}</Tabs.Trigger>{/each}</Tabs.List>
  <Tabs.Content value="Params"><p class="config-section-label">Query Params</p><Rows rows={value.http?.params ?? []} path={['http', 'params']} mode="params" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Headers"><p class="config-section-label">Request Headers</p><Rows rows={value.http?.headers ?? []} path={['http', 'headers']} mode="headers" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Variables"><p class="config-section-label">Request Variables</p><Rows rows={value.runtime?.variables ?? []} path={['runtime', 'variables']} mode="variables" {disabled} {onedit} />
</Tabs.Content>
  <Tabs.Content value="Body">
    <div class="editor-fields"><div class="editor-select"><label for="request-body-type">Body type</label><Select.Root type="single" value={bodyType} items={bodyOptions} allowDeselect={false} {disabled} onValueChange={setBody}><Select.Trigger id="request-body-type" aria-label="Body type"><Select.Value /></Select.Trigger><Select.Content>{#each bodyOptions as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>
    {#if ['json', 'text', 'xml'].includes(bodyType)}<Textarea class="min-h-32 font-mono" aria-label="Request body" value={value.http.body.data} {disabled} spellcheck="false" oninput={(e) => onedit(['http', 'body', 'data'], e.currentTarget.value)}></Textarea>{:else if bodyType !== 'none'}<p>This body is retained. Select a supported type to replace it.</p>{/if}</div>
</Tabs.Content>
  <Tabs.Content value="Auth">
    <div class="editor-fields"><div class="editor-select"><label for="request-auth-type">Authentication</label><Select.Root type="single" value={authType} items={authOptions} allowDeselect={false} {disabled} onValueChange={setAuth}><Select.Trigger id="request-auth-type" aria-label="Authentication type"><Select.Value /></Select.Trigger><Select.Content>{#each authOptions as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>
      {#each (authType === 'basic' ? ['username', 'password'] : authType === 'bearer' ? ['token'] : authType === 'apikey' ? ['key', 'value'] : []) as field}<label>{field}<Input aria-label={`Auth ${field}`} type={['password', 'token', 'value'].includes(field) ? 'password' : 'text'} value={auth?.[field] ?? ''} {disabled} oninput={(e) => onedit(['http', 'auth', field], e.currentTarget.value)} placeholder={'Value or {{secret}}'} autocomplete="off" /></label>{/each}
      {#if authType === 'apikey'}<div class="editor-select"><label for="request-auth-placement">Placement</label><Select.Root type="single" value={auth.placement} items={placements} allowDeselect={false} {disabled} onValueChange={(value) => onedit(['http', 'auth', 'placement'], value)}><Select.Trigger id="request-auth-placement" aria-label="API key placement"><Select.Value /></Select.Trigger><Select.Content>{#each placements as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>{/if}
      <p>Save writes configuration values to YAML. Use placeholders for credentials and supply secrets in memory below.</p>
    </div>
  </Tabs.Content></Tabs.Root>
</section>
