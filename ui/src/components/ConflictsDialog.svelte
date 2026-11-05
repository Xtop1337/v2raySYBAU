<script lang="ts">
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';
  import { closeConflicts, conflicts, stopConflicts } from '../lib/conflicts.svelte';
  import { t } from '../lib/i18n.svelte';
  import type { Conflict } from '../lib/types';

  // Everything found is selected; the user can untick what should keep running.
  let chosen = $state<Record<string, boolean>>({});
  $effect(() => {
    if (conflicts.dialog) chosen = Object.fromEntries(conflicts.found.map((c) => [c.id, true]));
  });
  const selected = $derived(conflicts.found.filter((c) => chosen[c.id]).map((c) => c.id));

  // "xray ×18" rather than the same name eighteen times.
  function processList(c: Conflict): string {
    const counts = new Map<string, number>();
    for (const p of c.processes) counts.set(p.name, (counts.get(p.name) ?? 0) + 1);
    return [...counts].map(([name, n]) => (n > 1 ? `${name} ×${n}` : name)).join(', ');
  }
</script>

{#if conflicts.dialog}
  <Modal title={t('conflicts.title')} width={560} onclose={closeConflicts}>
    <p class="muted lead">{t('conflicts.lead')}</p>
    <div class="list" data-testid="conflicts">
      {#each conflicts.found as c (c.id)}
        <label class="item">
          <input type="checkbox" bind:checked={chosen[c.id]} />
          <span class="text">
            <b>{c.name}</b>
            <span class="faint mono procs">{processList(c)}</span>
            <span class="muted reason">{t(`conflicts.kind.${c.kind}`)}</span>
          </span>
        </label>
      {/each}
    </div>
    {#snippet footer()}
      <button class="btn" onclick={closeConflicts} data-testid="conflicts-keep">{t('conflicts.keep')}</button>
      <button class="btn primary" disabled={!selected.length || conflicts.stopping} onclick={() => stopConflicts(selected)} data-testid="conflicts-stop">
        <Icon name="stop" size={14} />{conflicts.stopping ? t('conflicts.stopping') : t('conflicts.stop')}
      </button>
    {/snippet}
  </Modal>
{/if}

<style>
  .lead {
    margin: 0 0 12px;
    font-size: 13px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .item {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    cursor: pointer;
  }

  .item input {
    margin-top: 3px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .procs {
    font-size: 11.5px;
  }

  .reason {
    font-size: 12.5px;
  }
</style>
