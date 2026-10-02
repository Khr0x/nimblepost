<script lang="ts">
  import { tick } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { ChevronLeft, ChevronRight, Plus, X } from '@lucide/svelte';
  import type { Draft } from './draft';
  import type { RequestTab } from './request-tabs';

  let { tabs, activeId, draft, disabled = false, visible = true, onactivate, onclose, onnew }: {
    tabs: RequestTab[]; activeId: number; draft: Draft | null; disabled?: boolean; visible?: boolean;
    onactivate: (id: number) => Promise<void>; onclose: (id: number) => Promise<void>; onnew: () => Promise<void>;
  } = $props();
  const isEdited = $derived(!!draft?.edits.length);
  let requestTabsList = $state<HTMLDivElement | null>(null);
  let tabsOverflow = $state(false);
  let tabsCanScrollLeft = $state(false);
  let tabsCanScrollRight = $state(false);
  function updateTabNavigation() {
    if (!requestTabsList) return;
    const { scrollLeft, scrollWidth, clientWidth } = requestTabsList;
    tabsOverflow = scrollWidth > clientWidth + 1;
    tabsCanScrollLeft = scrollLeft > 1;
    tabsCanScrollRight = scrollLeft + clientWidth < scrollWidth - 1;
  }
  function observeRequestTabs(node: HTMLDivElement) {
    const observer = new ResizeObserver(updateTabNavigation);
    observer.observe(node);
    observer.observe(node.firstElementChild!);
    return { destroy: () => observer.disconnect() };
  }
  function scrollRequestTabs(direction: number) {
    requestTabsList?.scrollBy({ left: direction * requestTabsList.clientWidth * 0.8, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth' });
  }
  $effect(() => {
    const list = requestTabsList;
    const id = activeId;
    if (!list || !visible) return;
    void (async () => {
      await tick();
      updateTabNavigation();
      await tick(); // Account for the width occupied by newly visible chevrons.
      const tab = list.querySelector<HTMLElement>(`[data-tab-id="${id}"]`);
      if (!tab) return;
      const viewport = list.getBoundingClientRect();
      const rect = tab.getBoundingClientRect();
      if (rect.left < viewport.left) list.scrollLeft += rect.left - viewport.left;
      else if (rect.right > viewport.right) list.scrollLeft += rect.right - viewport.right;
      updateTabNavigation();
    })();
  });
</script>

<nav class="request-tabs" aria-label="Request tabs">
  <div class="request-tabs-list" bind:this={requestTabsList} use:observeRequestTabs onscroll={updateTabNavigation}>
    <div class="request-tabs-track">
      {#each tabs as item (item.id)}
        {@const current = item.id === activeId}
        {@const value = current ? draft?.value : item.draft.value}
        {@const name = value?.info?.name ?? 'Untitled'}
        <div class="request-document" class:active={current} data-tab-id={item.id}>
          <Button variant="ghost" class="request-tab-trigger" title={name} aria-label={`Open request tab ${item.id}: ${name}`} aria-pressed={current} {disabled} onclick={() => onactivate(item.id)}><span class="http-method" data-method={value?.http?.method ?? 'GET'}>{value?.http?.method ?? 'GET'}</span><span class="request-tab-name">{name}</span>{#if current ? isEdited : item.draft.edits.length}<span class="request-dirty" aria-label="Unsaved changes"></span>{/if}</Button>
          <Button variant="ghost" class="request-tab-close" aria-label={`Close request tab ${item.id}: ${name}`} title="Close tab" {disabled} onclick={() => onclose(item.id)}><X size={12} aria-hidden="true" /></Button>
        </div>
      {/each}
    </div>
  </div>
  {#if tabsOverflow}
    <div class="request-tabs-navigation">
      <Button variant="ghost" size="icon-sm" class="size-[28px] text-muted-foreground" aria-label="Scroll request tabs left" title="Previous tabs" disabled={!tabsCanScrollLeft} onclick={() => scrollRequestTabs(-1)}><ChevronLeft aria-hidden="true" /></Button>
      <Button variant="ghost" size="icon-sm" class="size-[28px] text-muted-foreground" aria-label="Scroll request tabs right" title="Next tabs" disabled={!tabsCanScrollRight} onclick={() => scrollRequestTabs(1)}><ChevronRight aria-hidden="true" /></Button>
    </div>
  {/if}
  <Button variant="ghost" size="icon-sm" class="size-[28px] shrink-0 text-muted-foreground" aria-label="New request" title="New untitled request" {disabled} onclick={onnew}><Plus aria-hidden="true" /></Button>
</nav>
