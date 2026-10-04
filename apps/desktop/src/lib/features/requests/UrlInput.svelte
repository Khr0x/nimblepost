<script lang="ts">
  import { tick } from 'svelte';
  import VariableInput from '$lib/components/VariableInput.svelte';
  import type { VariablePreview } from '$lib/desktop/api';
  import { completeUrlVariable, urlVariableMatch } from './url-completion';

  let { value, variables, disabled = false, onvariableedit, onchange, onblur }: {
    value: string; variables: VariablePreview[]; onvariableedit: (name: string, value: string) => void; disabled?: boolean; onchange: (value: string) => void;
    onblur?: () => void;
  } = $props();
  const id = $props.id();
  let input = $state<HTMLInputElement | null>(null);
  let cursor = $state(0), focused = $state(false), dismissed = $state(false), selected = $state(0);
  const match = $derived(urlVariableMatch(value, cursor));
  const options = $derived(match ? variables.filter(row => row.name.toLowerCase().startsWith(match.prefix.toLowerCase())) : []);
  const active = $derived(Math.min(selected, Math.max(0, options.length - 1)));
  const open = $derived(focused && !disabled && !dismissed && options.length > 0);

  function updateSelection() {
    cursor = input?.selectionStart ?? 0;
  }
  function update() {
    updateSelection();
    selected = 0; dismissed = false;
  }
  async function accept(name: string) {
    if (disabled) return;
    const result = completeUrlVariable(value, cursor, name);
    if (!result) return;
    dismissed = true;
    onchange(result.value);
    await tick();
    input?.focus();
    input?.setSelectionRange(result.cursor, result.cursor);
    updateSelection();
  }
  function keydown(event: KeyboardEvent) {
    if (!open || event.isComposing) return;
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      selected = (active + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length;
      void tick().then(() => document.getElementById(`${id}-${selected}`)?.scrollIntoView({ block: 'nearest' }));
    } else if (event.key === 'Enter' || event.key === 'Tab') {
      event.preventDefault();
      void accept(options[active].name);
    } else if (event.key === 'Escape') {
      event.preventDefault(); event.stopPropagation(); dismissed = true;
    }
  }
</script>

<div class="url-input">
  <VariableInput bind:ref={input} class="h-full w-full rounded-none" aria-label="Request URL" {value} {variables} {onvariableedit} {disabled} required
    placeholder="https://api.example.com" autocomplete="off" spellcheck="false" role="combobox"
    aria-autocomplete="list" aria-expanded={open} aria-controls={open ? id : undefined}
    aria-activedescendant={open ? `${id}-${active}` : undefined}
    oninput={(event) => { onchange(event.currentTarget.value); update(); }}
    onfocus={() => { focused = true; update(); }} onblur={() => { focused = false; onblur?.(); }}
    onclick={update} onselect={updateSelection} onkeydown={keydown}
    onkeyup={(event) => { if (['ArrowLeft', 'ArrowRight', 'Home', 'End'].includes(event.key)) update(); }} />
  {#if open}
    <div id={id} role="listbox" aria-label="URL variables" class="url-options">
      {#each options as variable, index (variable.name)}
        <div id={`${id}-${index}`} role="option" aria-selected={index === active}>
          <button type="button" tabindex="-1" class:active={index === active}
            onmousedown={(event) => event.preventDefault()} onclick={() => accept(variable.name)}>
            <span>{variable.name}</span><small>{variable.scope}{variable.secret ? ' · secret' : ''}</small>
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .url-input { --variable-input-border:0px; position:relative; flex:1; min-width:0; height:100%; }
  .url-options { position:absolute; top:calc(100% + 4px); left:0; right:0; z-index:50; max-height:240px; overflow:auto; padding:4px; background:var(--surface); border:1px solid var(--border); border-radius:5px; box-shadow:0 4px 12px #0002; }
  button { display:flex; align-items:center; justify-content:space-between; gap:12px; width:100%; padding:6px 8px; border:0; border-radius:3px; background:transparent; color:var(--text); text-align:left; font-family:ui-monospace,SFMono-Regular,monospace; font-size:12px; cursor:pointer; }
  button.active, button:hover { background:var(--subtle); }
  small { color:var(--text-muted); font-size:10px; }
</style>
