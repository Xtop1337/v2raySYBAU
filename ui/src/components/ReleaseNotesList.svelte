<script lang="ts">
  // Changes of every release newer than the installed one (rendered from the release notes).
  import { dateTime } from '../lib/format';
  import { renderMarkdown } from '../lib/markdown';
  import { t } from '../lib/i18n.svelte';
  import type { ReleaseNotes } from '../lib/types';
  import { openNoteLink } from '../lib/updates.svelte';

  let { releases }: { releases: ReleaseNotes[] } = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="notes" onclick={openNoteLink} data-testid="release-notes">
  {#each releases as release (release.version)}
    <section>
      <h3>
        {t('update.versionTitle', { v: release.version })}
        {#if release.published}<span class="faint date">{dateTime(release.published)}</span>{/if}
      </h3>
      {#if release.body.trim()}
        <div class="md">{@html renderMarkdown(release.body)}</div>
      {:else}
        <p class="faint">{t('update.noNotes')}</p>
      {/if}
    </section>
  {/each}
</div>

<style>
  .notes {
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow: auto;
    max-height: 46vh;
    padding-right: 4px;
  }

  h3 {
    margin: 0 0 4px;
    font-size: 14px;
    display: flex;
    gap: 8px;
    align-items: baseline;
  }

  .date {
    font-size: 12px;
    font-weight: 400;
  }

  .md {
    font-size: 13px;
    line-height: 1.5;
  }

  .md :global(h4) {
    margin: 10px 0 4px;
    font-size: 13.5px;
  }

  .md :global(p),
  .md :global(ul) {
    margin: 4px 0;
  }

  .md :global(ul) {
    padding-left: 20px;
  }

  .md :global(code) {
    font-family: 'Cascadia Code', Consolas, monospace;
    font-size: 12px;
    padding: 1px 4px;
    border-radius: 4px;
    background: var(--surface-2);
  }

  .md :global(pre) {
    padding: 8px 10px;
    border-radius: 8px;
    background: var(--surface-2);
    overflow: auto;
    font-size: 12px;
  }

  .md :global(a) {
    color: var(--accent);
  }

  .md :global(hr) {
    border: none;
    border-top: 1px solid var(--border);
    margin: 8px 0;
  }
</style>
