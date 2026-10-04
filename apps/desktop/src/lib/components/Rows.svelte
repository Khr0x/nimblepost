<script lang="ts">
  type Row = Record<string, any>;
  import * as Table from '$lib/components/ui/table';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import { Trash2 } from '@lucide/svelte';
  let { rows = [], path, mode, disabled = false, environment = false, onedit }: {
    rows?: Row[]; path: string[]; mode: 'headers' | 'params' | 'variables'; disabled?: boolean;
    environment?: boolean; onedit: (path: string[], value: unknown) => void;
  } = $props();
  const emptyRow = $derived<Row>(mode === 'params' ? { name: '', value: '', type: 'query' } : { name: '', value: '' });
  const visibleRows = $derived([...rows, emptyRow]);
  function set(index: number, key: string, value: unknown) {
    if (index === rows.length) {
      if (value === '') return;
      const row = { ...emptyRow, [key]: value };
      onedit(index ? [...path, String(index)] : path, index ? row : [row]);
    } else onedit([...path, String(index), key], value);
  }
  function secret(index: number, active: boolean) {
    if (active) { set(index, 'value', null); set(index, 'secret', true); set(index, 'type', 'string'); }
    else { set(index, 'secret', null); set(index, 'type', null); set(index, 'value', ''); }
  }
  const text = (row: Row) => typeof row.value === 'string' ? row.value : row.value?.type === 'string' ? row.value.data : JSON.stringify(row.value ?? '');
</script>

<div class="rows-editor">
  <div class="rows-table">
  <Table.Root><Table.Header><Table.Row><Table.Head><span class="sr-only">Enabled</span></Table.Head><Table.Head>Name</Table.Head><Table.Head>Value</Table.Head>{#if environment}<Table.Head>Secret</Table.Head>{/if}<Table.Head><span class="sr-only">Remove</span></Table.Head></Table.Row></Table.Header>
    <Table.Body>{#each visibleRows as row, index}<Table.Row>
      <Table.Cell>{#if index < rows.length}<Checkbox aria-label={`Enable ${mode} row ${index + 1}`} checked={!row.disabled} {disabled} onCheckedChange={(checked) => set(index, 'disabled', !checked)} />{/if}</Table.Cell>
      <Table.Cell><Input class="h-[28px]" aria-label={`${mode} name ${index + 1}`} value={row.name ?? ''} {disabled} oninput={(e) => set(index, 'name', e.currentTarget.value)} placeholder={mode === 'headers' ? 'Header name' : 'Name'} /></Table.Cell>
      <Table.Cell><Input class="h-[28px]" aria-label={`${mode} value ${index + 1}`} value={row.secret ? '' : text(row)} disabled={disabled || !!row.secret || (typeof row.value !== 'string' && row.value !== undefined && row.value?.type !== 'string')} placeholder={row.secret ? 'Supplied in memory at Send' : 'Value or {{variable}}'} oninput={(e) => row.value?.type === 'string' ? onedit([...path, String(index), 'value', 'data'], e.currentTarget.value) : set(index, 'value', e.currentTarget.value)} /></Table.Cell>
      {#if environment}<Table.Cell>{#if index < rows.length}<Checkbox aria-label={`Secret variable ${index + 1}`} checked={!!row.secret} {disabled} onCheckedChange={(checked) => secret(index, checked)} />{/if}</Table.Cell>{/if}
      <Table.Cell>{#if index < rows.length}<Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label={`Remove ${mode} row ${index + 1}`} {disabled} onclick={() => onedit([...path, String(index)], null)}><Trash2 aria-hidden="true" /></Button>{/if}</Table.Cell>
    </Table.Row>{/each}</Table.Body>
  </Table.Root>
  </div>
</div>
