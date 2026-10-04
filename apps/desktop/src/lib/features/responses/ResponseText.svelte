<script lang="ts">
  import { tick, untrack } from 'svelte';
  import { hasManyLines, responseLineStarts, responseLine, responseLineTops, responseLineAt, responseLineWindow, RESPONSE_LINE_HEIGHT } from './response-lines';

  let { text, wrap = true }: { text: string; wrap?: boolean } = $props();
  const virtual = $derived(hasManyLines(text));
  const starts = $derived(virtual ? responseLineStarts(text) : new Uint32Array());
  let viewport = $state<HTMLDivElement | null>(null);
  let height = $state(0), width = $state(0), scrollTop = $state(0);
  let measurements = $state(0);
  const heights = new Map<number, number>();
  const tops = $derived.by(() => { measurements; return responseLineTops(starts.length, heights); });
  const range = $derived(responseLineWindow(tops, Math.max(0, scrollTop - 16), height));
  const visible = $derived(Array.from({ length: range.end - range.start }, (_, index) => range.start + index));
  let previousText = '', previousWrap = true, previousWidth = 0, previousCount = 0;

  $effect.pre(() => {
    const source = text, wrapped = wrap, availableWidth = width;
    untrack(() => {
      const element = viewport;
      const anchor = responseLineAt(tops, Math.max(0, scrollTop - 16));
      // Appending a chunk changes the old final line; all earlier measurements remain valid.
      const appended = wrapped === previousWrap && availableWidth === previousWidth && source.startsWith(previousText);
      if (appended) heights.delete(previousCount - 1);
      else heights.clear();
      const sameText = source === previousText;
      previousText = source; previousWrap = wrapped; previousWidth = availableWidth;
      previousCount = starts.length;
      measurements++;
      if (element && !appended) {
        const target = sameText ? (tops[anchor] ?? 0) + 16 : 0;
        void tick().then(() => {
          if (element.isConnected) { element.scrollTop = target; scrollTop = element.scrollTop; }
        });
      }
    });
  });

  $effect(() => {
    if (!viewport) return;
    const element = viewport;
    const measure = () => { height = element.clientHeight; width = element.clientWidth; };
    const observer = new ResizeObserver(measure);
    observer.observe(element); measure();
    return () => observer.disconnect();
  });

  $effect(() => {
    if (!viewport) return;
    const element = viewport;
    range; text; wrap; width;
    const observer = new ResizeObserver(entries => {
      const oldTops = tops;
      const anchor = responseLineAt(oldTops, Math.max(0, element.scrollTop - 16));
      const offset = element.scrollTop - oldTops[anchor];
      const atEnd = element.scrollTop > 0 && element.scrollHeight - element.clientHeight - element.scrollTop < 2;
      let changed = false;
      for (const entry of entries) {
        const index = Number((entry.target as HTMLElement).dataset.responseLine);
        const measured = Math.max(RESPONSE_LINE_HEIGHT, entry.contentRect.height);
        if (Math.abs((heights.get(index) ?? RESPONSE_LINE_HEIGHT) - measured) > 0.1) {
          heights.set(index, measured); changed = true;
        }
      }
      if (changed) {
        measurements++;
        const target = tops[anchor] + offset;
        void tick().then(() => {
          if (element.isConnected) { element.scrollTop = atEnd ? element.scrollHeight : target; scrollTop = element.scrollTop; }
        });
      }
    });
    element.querySelectorAll('[data-response-line]').forEach(row => observer.observe(row));
    return () => observer.disconnect();
  });
</script>

{#if virtual}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (The scrollable region needs keyboard access.) -->
  <div class="response-text-viewport" class:nowrap={!wrap} bind:this={viewport} role="region" aria-label="Response body" tabindex="0" onscroll={() => scrollTop = viewport?.scrollTop ?? 0} data-response-lines={starts.length}>
    <div class="response-text-space" style:height={`${tops[starts.length]}px`}>
      <div class="response-text-window" style:top={`${tops[range.start]}px`}>
        {#each visible as index (index)}<div class="response-text-line" data-response-line={index}>{responseLine(text, starts, index) || '\n'}</div>{/each}
      </div>
    </div>
  </div>
{:else}
  <pre class="response-text-static" class:nowrap={!wrap} aria-label="Response body">{text}</pre>
{/if}
