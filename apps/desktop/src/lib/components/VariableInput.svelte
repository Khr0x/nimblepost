<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { Check, Copy, Database } from '@lucide/svelte';
  import type { VariablePreview } from '$lib/desktop/api';
  import type { HTMLInputAttributes } from 'svelte/elements';
  import { Input } from '$lib/components/ui/input';
  import { Button } from '$lib/components/ui/button';
  import { isVariablePlaceholder, urlHighlightParts } from '$lib/features/requests/url-completion';

  let { ref = $bindable(null), value = '', type = 'text', disabled = false, variables = [], onvariableedit, class: className = '', ...props }: Omit<HTMLInputAttributes, 'value' | 'type' | 'files'> & {
    ref?: HTMLInputElement | null; value?: string; type?: 'text' | 'password'; variables?: VariablePreview[];
    onvariableedit?: (name: string, value: string) => void;
  } = $props();
  let scrollLeft = $state(0), selectionStart = $state(0), selectionEnd = $state(0);
  let focused = $state(false);
  const parts = $derived(urlHighlightParts(value, focused ? selectionStart : 0, focused ? selectionEnd : 0));
  const masked = $derived(type === 'password' && !isVariablePlaceholder(value));
  let hovered = $state<{ name: string; left: number; top: number } | null>(null);
  const tooltipId = $props.id();
  let copied = $state(false), copyIssue = $state('');
  const variable = $derived(variables.find(row => row.name === hovered?.name));
  const scope = $derived(variable ? variable.scope.charAt(0).toUpperCase() + variable.scope.slice(1) : 'Not defined');
  let editValue = $state('');
  $effect(() => { editValue = variable?.secret ? '' : variable?.value ?? ''; });
  let closeTimer: ReturnType<typeof setTimeout>;
  const keepOpen = () => clearTimeout(closeTimer);
  function closeSoon() {
    clearTimeout(closeTimer);
    closeTimer = setTimeout(() => {
      if (!document.getElementById(tooltipId)?.contains(document.activeElement)) hovered = null;
    }, 150);
  }
  onDestroy(() => clearTimeout(closeTimer));
  function showVariable(span: HTMLElement) {
    keepOpen();
    const rect = span.getBoundingClientRect();
    const name = span.dataset.variable!;
    if (hovered?.name !== name) { copied = false; copyIssue = ''; }
    hovered = { name, left: Math.max(8, Math.min(rect.left, window.innerWidth - 428)),
      top: rect.bottom + 8 + 160 < window.innerHeight ? rect.bottom + 8 : Math.max(8, rect.top - 160) };
  }
  function hoverVariable(event: MouseEvent) {
    if (event.buttons) { hovered = null; return; }
    const span = Array.from(ref?.parentElement?.querySelectorAll<HTMLElement>('[data-variable]') ?? []).find(node => {
      const rect = node.getBoundingClientRect();
      return event.clientX >= rect.left && event.clientX <= rect.right && event.clientY >= rect.top && event.clientY <= rect.bottom;
    });
    if (span) showVariable(span); else closeSoon();
  }
  async function copyValue() {
    if (!variable || variable.secret || variable.value === null) return;
    try { await navigator.clipboard.writeText(editValue); copied = true; copyIssue = ''; }
    catch { copyIssue = 'Could not copy value'; }
  }
  function portal(node: HTMLElement) {
    node.ownerDocument.body.appendChild(node);
    return { destroy: () => node.remove() };
  }
  function updateSelection() {
    selectionStart = ref?.selectionStart ?? 0;
    selectionEnd = ref?.selectionEnd ?? 0;
  }
  $effect(() => {
    const input = ref;
    if (!input) return;
    const document = input.ownerDocument;
    const sync = () => {
      if (document.activeElement !== input) return;
      updateSelection();
      scrollLeft = input.scrollLeft;
    };
    document.addEventListener('selectionchange', sync);
    return () => document.removeEventListener('selectionchange', sync);
  });
</script>

{#if masked}
  <Input bind:ref {value} {type} {disabled} class={className} {...props} />
{:else}
  <div class="variable-input">
    <div class="variable-highlight" class:disabled aria-hidden="true"><div style:transform={`translateX(-${scrollLeft}px)`}>{#each parts as part}<span data-variable={part.name} class:variable={part.variable} class:selected-text={part.selected}>{part.text}</span>{/each}</div></div>
    <Input bind:ref {value} type="text" {disabled} class={`variable-text ${className}`} {...props}
      aria-describedby={hovered ? tooltipId : props['aria-describedby']}
      onselect={(event) => { updateSelection(); props.onselect?.(event); }}
      oninput={(event) => { updateSelection(); props.oninput?.(event); }}
      onclick={(event) => { updateSelection(); props.onclick?.(event); }}
      onkeyup={(event) => { updateSelection(); props.onkeyup?.(event); }}
      onmousemove={(event) => { hoverVariable(event); props.onmousemove?.(event); }}
      onmouseleave={(event) => { closeSoon(); props.onmouseleave?.(event); }}
      onkeydown={(event) => {
        if (event.key === 'Escape' && hovered) { hovered = null; event.preventDefault(); event.stopPropagation(); }
        else if (event.altKey && event.key === 'ArrowDown') {
          const cursor = ref?.selectionStart ?? 0;
          const match = Array.from(value.matchAll(/\{\{[ \t]*([\w.-]+)[ \t]*\}\}/g)).find(match => cursor >= match.index! && cursor <= match.index! + match[0].length);
          const span = Array.from(ref?.parentElement?.querySelectorAll<HTMLElement>('[data-variable]') ?? []).find(node => node.dataset.variable === match?.[1]);
          if (span) { event.preventDefault(); showVariable(span); void tick().then(() => document.getElementById(tooltipId)?.focus()); }
        }
        else props.onkeydown?.(event);
      }}
      onfocus={(event) => { focused = true; updateSelection(); props.onfocus?.(event); }}
      onblur={(event) => { focused = false; closeSoon(); props.onblur?.(event); }}
      onscroll={(event) => { scrollLeft = event.currentTarget.scrollLeft; hovered = null; props.onscroll?.(event); }} />
  </div>
{/if}

{#if hovered && !masked}
  <div use:portal id={tooltipId} class="variable-tooltip" role="dialog" aria-label={`Variable ${hovered.name}`} tabindex="-1"
    style:left={`${hovered.left}px`} style:top={`${hovered.top}px`}
    onmouseenter={keepOpen} onmouseleave={closeSoon}
    onfocusin={keepOpen} onfocusout={closeSoon}
    onkeydown={(event) => { if (event.key === 'Escape') { hovered = null; ref?.focus(); event.preventDefault(); event.stopPropagation(); } }}>
    <div class="variable-heading"><strong>{hovered.name}</strong><span class="variable-scope"><Database size={12} aria-hidden="true" />{scope}</span></div>
    <form onsubmit={(event) => {
      event.preventDefault();
      if (disabled || !onvariableedit || !hovered) return;
      onvariableedit(hovered.name, editValue);
      hovered = null; ref?.focus();
    }}>
      <div class="variable-value">
        <Input aria-label={`Value of ${hovered.name}`} bind:value={editValue} type={variable?.secret ? 'password' : 'text'}
          oninput={() => { copied = false; copyIssue = ''; }}
          disabled={disabled || !onvariableedit} autocomplete="off" spellcheck="false"
          placeholder={variable?.secret ? 'New secret value' : variable ? 'Value' : 'Variable not defined'} />
        {#if variable && !variable.secret && variable.value !== null}<button type="button" aria-label={`Copy value of ${hovered.name}`} onclick={copyValue}>{#if copied}<Check size={16} />{:else}<Copy size={16} />{/if}</button>{/if}
      </div>
      {#if onvariableedit}<div class="variable-actions"><small>{variable?.secret ? 'Memory only' : variable?.scope === 'runtime' ? 'Runtime override' : variable?.scope === 'request' ? 'Use Save to persist' : 'Applies to this request · Save to persist'}</small><Button type="submit" size="sm" {disabled}>{variable?.secret ? 'Set secret' : 'Apply'}</Button></div>{/if}
    </form>
    {#if copyIssue}<small role="status">{copyIssue}</small>{/if}
  </div>
{/if}

<style>
  .variable-input { position:relative; width:100%; min-width:0; height:100%; }
  .variable-highlight { position:absolute; inset:0; z-index:1; display:flex; align-items:center; padding:0 calc(var(--variable-input-padding, 10px) + var(--variable-input-border, 1px)); overflow:hidden; pointer-events:none; color:var(--text); font-family:ui-monospace,SFMono-Regular,monospace; font-size:12px; line-height:20px; }
  .variable-highlight > div { white-space:pre; }
  .variable-highlight.disabled { opacity:.5; }
  .variable { color:var(--success); }
  .selected-text { background:color-mix(in srgb, #c69c6d 38%, var(--surface)); color:var(--text); }
  .variable-input :global(input.variable-text) { position:relative; color:transparent; caret-color:var(--text); padding:4px var(--variable-input-padding, 10px); font-family:ui-monospace,SFMono-Regular,monospace; font-size:12px; line-height:20px; }
  .variable-input :global(input.variable-text::selection) { background:transparent; color:transparent; }
  .variable-tooltip { position:fixed; z-index:100; width:min(420px, calc(100vw - 16px)); padding:10px; border:1px solid var(--border); border-radius:6px; background:var(--surface); color:var(--text); box-shadow:0 6px 20px #0003; font-size:12px; }
  .variable-tooltip:focus-visible { outline:1px solid var(--brand-text); outline-offset:2px; }
  .variable-heading { display:flex; align-items:center; justify-content:space-between; gap:12px; margin-bottom:8px; }
  .variable-heading strong { color:var(--brand-text); overflow-wrap:anywhere; }
  .variable-scope { display:flex; align-items:center; gap:4px; flex-shrink:0; padding:4px 6px; border-radius:5px; background:var(--brand-subtle); color:var(--brand-text); font-size:11px; }
  .variable-value { display:flex; align-items:center; gap:8px; padding:4px 8px; border:1px solid var(--border); border-radius:5px; background:var(--subtle); }
  .variable-value :global(input) { flex:1; min-width:0; border:0; background:transparent; box-shadow:none; padding:4px 0; font-size:12px; }
  .variable-actions { display:flex; align-items:center; justify-content:space-between; gap:8px; margin-top:8px; }
  .variable-actions small { color:var(--text-muted); font-size:10px; }
  .variable-value button { display:flex; align-items:center; flex-shrink:0; border:0; background:transparent; color:var(--text-muted); cursor:pointer; }
  .variable-value button:hover { color:var(--text); }
</style>
