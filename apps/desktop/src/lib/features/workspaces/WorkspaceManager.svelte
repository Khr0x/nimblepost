<script lang="ts">
  import { Button } from '$lib/components/ui/button';
  import { ArrowLeft, Pencil, Trash2 } from '@lucide/svelte';
  import type { WorkspaceView } from '$lib/desktop/api';

  let { workspace, error = '', disabled = false, onback, onrename, ondelete }: {
    workspace: WorkspaceView | null; error?: string; disabled?: boolean;
    onback: () => Promise<void>; onrename: (id: number) => void; ondelete: (id: number) => void;
  } = $props();
</script>

<section class="workspace-manager" aria-labelledby="workspace-manager-heading">
  <Button variant="ghost" size="sm" class="workspace-manager-back" onclick={onback}><ArrowLeft aria-hidden="true" />Back to workspace</Button>
  <div class="workspace-manager-heading">
    <h1 id="workspace-manager-heading" tabindex="-1">Manage workspaces</h1>
    <p>Rename or remove your local workspaces. Collection folders and files stay on your machine.</p>
  </div>
  <ul class="workspace-manager-list">
    {#each workspace?.workspaces ?? [] as item (item.id)}
      <li class="workspace-manager-row" data-workspace-id={item.id}>
        <div class="workspace-manager-name">
          <span title={item.name}>{item.name}</span>
          {#if item.id === workspace?.activeWorkspaceId}<span class="workspace-active-label">Active</span>{/if}
        </div>
        <div class="workspace-manager-actions">
          <Button variant="ghost" size="icon-sm" aria-label={`Rename workspace ${item.name}`} title="Rename workspace" data-workspace-action="rename" {disabled} onclick={() => onrename(item.id)}><Pencil aria-hidden="true" /></Button>
          <Button variant="ghost" size="icon-sm" class="workspace-delete" aria-label={`Delete workspace ${item.name}`} title="Delete workspace" data-workspace-action="delete" {disabled} onclick={() => ondelete(item.id)}><Trash2 aria-hidden="true" /></Button>
        </div>
      </li>
    {/each}
  </ul>
  {#if error}<div class="error" role="alert">{error}</div>{/if}
</section>
