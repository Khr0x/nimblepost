<script lang="ts">
  import type { ResponseMeta } from '../../desktop/api';
  import { formatJSON, tokenizeJSON, foldJSON } from './response-body';
  import JSONViewer from './JSONViewer.svelte';
  import ResponseText from './ResponseText.svelte';
  import { hasManyLines } from './response-lines';
  import * as Table from '$lib/components/ui/table';
  import { Button } from '$lib/components/ui/button';
  import * as Tabs from '$lib/components/ui/tabs';
  import * as Select from '$lib/components/ui/select';
  import { Check, Copy, WrapText } from '@lucide/svelte';
  let { response, body, offset, chunkLoading, onloadMore }: {
    response: ResponseMeta; body: string; offset: number; chunkLoading: boolean; onloadMore: () => Promise<void>;
  } = $props();
  let tab = $state('body');
  let mode = $state('pretty');
  let wrap = $state(true);
  let copied = $state(0);
  let copyError = $state('');
  $effect(() => {
    if (!copied) return;
    const timer = setTimeout(() => copied = 0, 2000);
    return () => clearTimeout(timer);
  });
  const complete = $derived(offset >= response.bodyBytes);
  const contentType = $derived(response.headers.find(header => header.name.toLowerCase() === 'content-type')?.value ?? '');
  const json = $derived(response.text && complete && body ? formatJSON(body) : { pretty: null, issue: null });
  const displayMode = $derived(!response.text ? 'raw' : mode === 'pretty' && json.pretty === null ? 'text' : mode);
  const displayedBody = $derived(displayMode === 'pretty' ? json.pretty ?? body : body);
  const highlightedBody = $derived(displayMode === 'pretty' && json.pretty !== null && !hasManyLines(json.pretty) ? tokenizeJSON(json.pretty) : null);
  const foldedBody = $derived(highlightedBody ? foldJSON(highlightedBody) : null);
  const formats = $derived(response.text ? [{ value: 'pretty', label: 'JSON · Pretty' }, { value: 'text', label: 'Text' }, { value: 'raw', label: 'Raw' }] : [{ value: 'raw', label: 'Hex' }]);
  const size = (bytes: number) => bytes < 1024 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
  async function copyBody() {
    const id = response.id;
    copyError = '';
    try { await navigator.clipboard.writeText(displayedBody); if (response.id === id) copied++; }
    catch { if (response.id === id) { copied = 0; copyError = 'Could not copy the response.'; } }
  }
</script>

<section class="response-panel" aria-label="Response">
  <div class="response-heading"><h2>Response</h2><div class="response-stats"><span class="status" class:bad={response.status >= 400}>{response.status}</span><span>{response.elapsedMs} ms</span><span>{size(response.bodyBytes)}</span></div></div>
  <Tabs.Root bind:value={tab} class="min-h-0 flex-1 gap-0"><Tabs.List variant="line" aria-label="Response views" class="response-tabs w-full justify-start border-b border-border"><Tabs.Trigger class="flex-none px-3" value="body">Body</Tabs.Trigger><Tabs.Trigger class="flex-none px-3" value="headers">Headers <span class="rounded bg-muted px-1 text-xs">{response.headers.length}</span></Tabs.Trigger></Tabs.List>
    <Tabs.Content value="body" class="response-content response-body-content">
      <div class="response-body-toolbar">
        <Select.Root type="single" value={displayMode} items={formats} allowDeselect={false} onValueChange={(value) => { mode = value; copied = 0; copyError = ''; }}>
          <Select.Trigger size="sm" class="response-format" aria-label="Response body format"><Select.Value /></Select.Trigger>
          <Select.Content align="start">{#each formats as format}<Select.Item value={format.value} label={format.label} disabled={format.value === 'pretty' && json.pretty === null}>{format.label}</Select.Item>{/each}</Select.Content>
        </Select.Root>
        <span class="response-content-type" title={contentType}>{contentType || (response.text ? 'Text' : 'Binary · Hex view')}</span>
        <div class="response-body-actions"><span class="sr-only" role="status">{copied ? 'Response copied.' : ''}</span>{#if copyError}<span class="response-copy-message" role="alert">{copyError}</span>{/if}<Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label="Wrap response lines" aria-pressed={wrap && displayMode !== 'raw'} title="Wrap lines" disabled={displayMode === 'raw'} onclick={() => wrap = !wrap}><WrapText aria-hidden="true" /></Button><Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label="Copy response body" title={complete ? 'Copy full body' : 'Load the complete body to copy'} disabled={!complete || !body || chunkLoading} onclick={copyBody}>{#if copied}<Check class="action-confirmation" aria-hidden="true" />{:else}<Copy aria-hidden="true" />{/if}</Button></div>
      </div>
      {#if mode === 'pretty' && json.issue === 'large'}<p class="response-body-note">This body is too large to format automatically. Text and Raw show the original content.</p>{:else if mode === 'pretty' && complete && body && json.issue === 'invalid' && contentType.toLowerCase().includes('json')}<p class="response-body-note">The body is not valid JSON. Showing the original text.</p>{/if}
      {#if foldedBody}<JSONViewer lines={foldedBody} {wrap} />{:else if highlightedBody}<pre class="response-text-static" class:nowrap={!wrap || displayMode === 'raw'} aria-label="Response body">{#each highlightedBody as token}{#if token.kind === 'plain'}{token.text}{:else}<span class={`json-token json-${token.kind}`}>{token.text}</span>{/if}{/each}</pre>{:else}<ResponseText text={displayedBody || (response.bodyBytes === 0 ? '(empty body)' : 'Loading body…')} wrap={wrap && displayMode !== 'raw'} />{/if}
      {#if !complete}<div class="body-more"><span>{size(offset)} of {size(response.bodyBytes)} loaded{mode === 'pretty' && response.text ? ' · JSON formatting requires the complete body' : ''}</span><Button variant="outline" size="sm" onclick={onloadMore} disabled={chunkLoading}>{chunkLoading ? 'Loading…' : 'Load next 64 KB'}</Button></div>{/if}
    </Tabs.Content>
    <Tabs.Content value="headers" class="response-content">{#if response.headers.length}<Table.Root><Table.Header><Table.Row><Table.Head>Name</Table.Head><Table.Head>Value</Table.Head></Table.Row></Table.Header><Table.Body>{#each response.headers as header}<Table.Row><Table.Cell>{header.name}</Table.Cell><Table.Cell>{header.value}</Table.Cell></Table.Row>{/each}</Table.Body></Table.Root>{:else}<p class="response-body-note">No response headers.</p>{/if}</Tabs.Content>
  </Tabs.Root>
</section>
