<script lang="ts">
  import Icon from './Icon.svelte';
  import Modal from './Modal.svelte';
  import ReleaseNotesList from './ReleaseNotesList.svelte';
  import { t } from '../lib/i18n.svelte';
  import { app } from '../lib/store.svelte';
  import { closeUpdate, downloadUpdate, skipUpdate, updates } from '../lib/updates.svelte';
</script>

{#if updates.dialog && updates.available}
  <Modal title={t('update.title', { v: updates.available.version })} width={620} onclose={closeUpdate}>
    <p class="muted lead">{t('update.lead', { current: app.appVersion })}</p>
    <ReleaseNotesList releases={updates.available.releases} />
    {#snippet footer()}
      <button class="btn" onclick={skipUpdate} data-testid="update-skip">{t('update.skip')}</button>
      <button class="btn" onclick={closeUpdate} data-testid="update-later">{t('update.later')}</button>
      <button class="btn primary" onclick={downloadUpdate} data-testid="update-download"><Icon name="download" size={15} />{t('update.download')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .lead {
    margin: 0 0 12px;
    font-size: 13px;
  }
</style>
