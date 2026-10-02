<script lang="ts">
  import * as Collapsible from '$lib/components/ui/collapsible';
  import type { JSONLine, JSONToken } from './response-body';
  import { ChevronRight } from '@lucide/svelte';
  let { lines, wrap = true }: { lines: JSONLine[]; wrap?: boolean } = $props();
</script>

{#snippet tokens(values: JSONToken[])}
  {#each values as token}{#if token.kind === 'plain'}{token.text}{:else}<span class={`json-token json-${token.kind}`}>{token.text}</span>{/if}{/each}
{/snippet}

{#snippet rows(values: JSONLine[])}
  {#each values as line (line.id)}
    {#if line.children && line.closing}
      <Collapsible.Root class="json-fold" open={true}>
        <Collapsible.Trigger class="json-line" aria-label={`Toggle ${line.label}`}><ChevronRight class="json-fold-chevron" size={12} aria-hidden="true" />{@render tokens(line.tokens)}<span class="json-fold-preview" aria-hidden="true"> … {@render tokens(line.closing.filter(token => token.kind !== 'plain'))}</span></Collapsible.Trigger>
        <Collapsible.Content forceMount>
        {@render rows(line.children)}
        <div class="json-line">{@render tokens(line.closing)}</div>
      </Collapsible.Content>
      </Collapsible.Root>
    {:else}
      <div class="json-line">{@render tokens(line.tokens)}</div>
    {/if}
  {/each}
{/snippet}

<div class="response-json" class:nowrap={!wrap} role="region" aria-label="Response body">{@render rows(lines)}</div>
