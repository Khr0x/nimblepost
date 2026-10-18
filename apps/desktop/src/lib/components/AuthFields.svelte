<script lang="ts">
  import type { VariablePreview } from '$lib/desktop/api';
  import type { Config } from '$lib/features/requests/draft';
  import VariableInput from './VariableInput.svelte';
  import * as Select from './ui/select';
  let { auth, path, idPrefix = 'request-auth', inDialog = false, disabled = false, variables = [], onvariableedit, onedit }: {
    auth: Config | string | undefined; path: string[]; idPrefix?: string; inDialog?: boolean; disabled?: boolean;
    variables?: VariablePreview[]; onvariableedit?: (name: string, value: string) => void;
    onedit: (path: string[], value: unknown) => void;
  } = $props();
  const authType = $derived(!auth || auth === 'inherit' ? 'inherit' : typeof auth === 'object' && typeof auth.type === 'string' ? auth.type : 'unsupported');
  const fields = $derived(auth && typeof auth === 'object' ? auth : {});
  const types = ['inherit', 'basic', 'bearer', 'apikey'];
  const options = $derived([...(!types.includes(authType) ? [{ value: authType, label: `${authType} (retained)` }] : []), ...types.map((value, index) => ({ value, label: ['Inherit', 'Basic', 'Bearer', 'API Key'][index] }))]);
  const placements = [{ value: 'header', label: 'Header' }, { value: 'query', label: 'Query' }];
  function setAuth(type: string) {
    if (!types.includes(type)) return;
    const next = type === 'inherit' ? 'inherit' : type === 'basic' ? { type, username: '', password: '{{password}}' } : type === 'bearer' ? { type, token: '{{token}}' } : { type, key: 'X-API-Key', value: '{{apiKey}}', placement: 'header' };
    onedit(path, next);
  }
</script>

<div class="editor-fields"><div class="editor-select"><label for={`${idPrefix}-type`}>Authentication</label><Select.Root type="single" value={authType} items={options} allowDeselect={false} {disabled} onValueChange={setAuth}><Select.Trigger id={`${idPrefix}-type`} aria-label="Authentication type"><Select.Value /></Select.Trigger><Select.Content portalProps={{disabled:inDialog}}>{#each options as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>
  {#each (authType === 'basic' ? ['username', 'password'] : authType === 'bearer' ? ['token'] : authType === 'apikey' ? ['key', 'value'] : []) as field}<label>{field}<VariableInput {variables} {onvariableedit} aria-label={`Auth ${field}`} type={['password', 'token', 'value'].includes(field) ? 'password' : 'text'} value={typeof fields?.[field] === 'string' ? fields[field] : ''} {disabled} oninput={(e) => onedit([...path, field], e.currentTarget.value)} placeholder={'Value or {{secret}}'} autocomplete="off" /></label>{/each}
  {#if authType === 'apikey'}<div class="editor-select"><label for={`${idPrefix}-placement`}>Placement</label><Select.Root type="single" value={fields?.placement} items={placements} allowDeselect={false} {disabled} onValueChange={(value) => onedit([...path, 'placement'], value)}><Select.Trigger id={`${idPrefix}-placement`} aria-label="API key placement"><Select.Value /></Select.Trigger><Select.Content portalProps={{disabled:inDialog}}>{#each placements as option}<Select.Item value={option.value} label={option.label}>{option.label}</Select.Item>{/each}</Select.Content></Select.Root></div>{/if}
  <p>Save writes configuration values to YAML. Use placeholders for credentials and supply secrets in memory when sending.</p>
</div>
