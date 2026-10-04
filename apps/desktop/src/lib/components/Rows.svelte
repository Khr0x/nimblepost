<script lang="ts">
  import { tick } from 'svelte';
  import type { VariablePreview } from '$lib/desktop/api';
  type Row = Record<string, any>;
  import * as Table from '$lib/components/ui/table';
  import { Button } from '$lib/components/ui/button';
  import { Checkbox } from '$lib/components/ui/checkbox';
  import { Input } from '$lib/components/ui/input';
  import VariableInput from './VariableInput.svelte';
  import { GripVertical, Trash2 } from '@lucide/svelte';
  import { reorderRows } from './reorder-rows';
  let { rows = [], path, mode, disabled = false, environment = false, variables = [], onvariableedit, onedit }: {
    rows?: Row[]; path: string[]; mode: 'headers' | 'params' | 'variables'; disabled?: boolean;
    environment?: boolean; variables?: VariablePreview[]; onvariableedit?: (name: string, value: string) => void; onedit: (path: string[], value: unknown) => void;
  } = $props();
  const emptyRow = $derived<Row>(mode === 'params' ? { name: '', value: '', type: 'query' } : { name: '', value: '' });
  const visibleRows = $derived([...rows, emptyRow]);
  let host: HTMLDivElement;
  let dragging = $state<number | null>(null), dropTarget = $state<number | null>(null), reorderNotice = $state('');
  let dragPointer: number | null = null;
  function finishDrag() { dragPointer = null; dragging = null; dropTarget = null; }
  function startDrag(event: PointerEvent, index: number) {
    if (event.button !== 0 || disabled || rows.length < 2 || dragPointer !== null) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    dragPointer = event.pointerId; dragging = index; dropTarget = index;
  }
  function targetRow(event: PointerEvent) {
    const elements = Array.from(host.querySelectorAll<HTMLTableRowElement>('tbody > tr'));
    const index = elements.findIndex(row => {
      const rect = row.getBoundingClientRect();
      return event.clientX >= rect.left && event.clientX <= rect.right && event.clientY >= rect.top && event.clientY <= rect.bottom;
    });
    return index < 0 ? null : Math.min(index, rows.length - 1);
  }
  function dragMove(event: PointerEvent) {
    if (dragPointer !== event.pointerId) return;
    dropTarget = disabled ? null : targetRow(event);
  }
  function endDrag(event: PointerEvent) {
    if (dragPointer !== event.pointerId || dragging === null) return;
    const from = dragging, to = targetRow(event);
    finishDrag();
    const handle = event.currentTarget as HTMLElement;
    if (handle.hasPointerCapture(event.pointerId)) handle.releasePointerCapture(event.pointerId);
    if (to !== null) void move(from, to);
  }
  async function move(from: number, to: number) {
    if (disabled) return;
    const next = reorderRows(rows, from, to);
    if (next === rows) return;
    const name = rows[from].name || 'Row';
    onedit(path, next);
    reorderNotice = `${name} moved to position ${to + 1}`;
    await tick();
    host.querySelector<HTMLButtonElement>(`[data-row-handle="${to}"]`)?.focus();
  }
  function reorderKey(event: KeyboardEvent, index: number) {
    const positions: Record<string, number> = { ArrowUp: index - 1, ArrowDown: index + 1, Home: 0, End: rows.length - 1 };
    if (!(event.key in positions)) return;
    event.preventDefault(); event.stopPropagation();
    void move(index, positions[event.key]);
  }
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

<div class="rows-editor" bind:this={host}>
  <span class="sr-only" role="status">{reorderNotice}</span>
  <div class="rows-table">
  <Table.Root><Table.Header><Table.Row><Table.Head><span class="sr-only">Enabled</span></Table.Head><Table.Head>Name</Table.Head><Table.Head>Value</Table.Head>{#if environment}<Table.Head>Description</Table.Head><Table.Head>Secret</Table.Head>{/if}<Table.Head><span class="sr-only">Remove</span></Table.Head></Table.Row></Table.Header>
    <Table.Body>{#each visibleRows as row, index}<Table.Row
      class={dragging === index ? 'row-dragging' : dropTarget === index && dragging !== null ? dragging > index ? 'row-drop-before' : dragging < index ? 'row-drop-after' : '' : ''}>
      <Table.Cell>{#if index < rows.length}<div class="row-controls"><Button type="button" variant="ghost" size="icon-xs" class="row-drag-handle" data-row-handle={index}
        aria-label={`Reorder ${mode} row ${index + 1}`} title="Drag to reorder · ↑/↓ to move" disabled={disabled || rows.length < 2}
        onkeydown={(event) => reorderKey(event, index)}
        onpointerdown={(event) => startDrag(event, index)} onpointermove={dragMove} onpointerup={endDrag}
        onpointercancel={finishDrag} onlostpointercapture={finishDrag}><GripVertical size={14} aria-hidden="true" /></Button><Checkbox aria-label={`Enable ${mode} row ${index + 1}`} checked={!row.disabled} {disabled} onCheckedChange={(checked) => set(index, 'disabled', !checked)} /></div>{/if}</Table.Cell>
      <Table.Cell><VariableInput {variables} {onvariableedit} class="h-[28px]" aria-label={`${mode} name ${index + 1}`} value={row.name ?? ''} {disabled} oninput={(e) => set(index, 'name', e.currentTarget.value)} placeholder={mode === 'headers' ? 'Header name' : 'Name'} /></Table.Cell>
      <Table.Cell><VariableInput {variables} {onvariableedit} class="h-[28px]" aria-label={`${mode} value ${index + 1}`} value={row.secret ? '' : text(row)} disabled={disabled || !!row.secret || (typeof row.value !== 'string' && row.value !== undefined && row.value?.type !== 'string')} placeholder={row.secret ? 'Supplied in memory at Send' : 'Value or {{variable}}'} oninput={(e) => row.value?.type === 'string' ? onedit([...path, String(index), 'value', 'data'], e.currentTarget.value) : set(index, 'value', e.currentTarget.value)} /></Table.Cell>
      {#if environment}<Table.Cell><Input class="h-[28px]" aria-label={`variables description ${index + 1}`} value={typeof row.description === 'string' ? row.description : row.description?.content ?? ''} {disabled} placeholder="Description" oninput={(e) => row.description && typeof row.description === 'object' ? onedit([...path, String(index), 'description', 'content'], e.currentTarget.value) : set(index, 'description', e.currentTarget.value)} /></Table.Cell><Table.Cell>{#if index < rows.length}<Checkbox aria-label={`Secret variable ${index + 1}`} checked={!!row.secret} {disabled} onCheckedChange={(checked) => secret(index, checked)} />{/if}</Table.Cell>{/if}
      <Table.Cell>{#if index < rows.length}<Button variant="ghost" size="icon-sm" class="size-[28px]" aria-label={`Remove ${mode} row ${index + 1}`} {disabled} onclick={() => onedit([...path, String(index)], null)}><Trash2 aria-hidden="true" /></Button>{/if}</Table.Cell>
    </Table.Row>{/each}</Table.Body>
  </Table.Root>
  </div>
</div>
