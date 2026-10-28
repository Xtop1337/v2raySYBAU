<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { app, errorText, toast } from '../lib/store.svelte';
  import type { CoreVersions } from '../lib/types';
  import { checkForUpdates, installUpdate, updates } from '../lib/updates.svelte';

  const REPO = 'https://github.com/Xtop1337/v2raySYBAU';
  let cores = $state<CoreVersions | null>(null);
  let busy = $state<string | null>(null);

  async function loadCores() {
    cores = await api.coreVersions().catch(() => null);
  }

  async function updateCore(kind: 'xray' | 'singbox' | 'geo') {
    busy = kind;
    try {
      const version = await api.updateCore(kind);
      const label = kind === 'xray' ? 'Xray' : kind === 'singbox' ? 'sing-box' : 'geoip/geosite';
      toast(t('about.coreUpdated', { core: label, v: version }), 'success');
      await loadCores();
    } catch (e) {
      toast(errorText(e), 'error', 8000);
    } finally {
      busy = null;
    }
  }

  loadCores();
</script>

<div class="page">
  <div class="hero card">
    <img src="/icon.svg" alt="" width="64" height="64" />
    <div>
      <h1>V2Ray Sybau</h1>
      <div class="muted">{t('about.version', { v: app.appVersion })}</div>
      <p class="muted desc">{t('about.description')}</p>
      <div class="row">
        <button class="btn small" onclick={() => openUrl(REPO)}><Icon name="globe" size={14} />{t('about.repo')}</button>
        <button class="btn small" onclick={() => openUrl(`${REPO}/releases`)}>{t('about.releases')}</button>
      </div>
    </div>
  </div>

  <div class="card block">
    <h2>{t('about.updates')}</h2>
    {#if updates.available}
      <div class="update">
        <b>{t('about.available', { v: updates.available.version })}</b>
        {#if updates.available.notes}<pre class="notes">{updates.available.notes}</pre>{/if}
        <button class="btn primary" disabled={updates.progress !== null} onclick={installUpdate}>
          <Icon name="download" size={15} />
          {updates.progress !== null ? t('about.installing', { p: updates.progress == null ? '' : `${updates.progress}%` }) : t('about.install')}
        </button>
      </div>
    {:else}
      <div class="row">
        <button class="btn" disabled={updates.checking} onclick={() => checkForUpdates(false)}>
          <Icon name="refresh" size={15} />{updates.checking ? t('about.checking') : t('about.checkApp')}
        </button>
        {#if updates.error}<span class="error">{updates.error}</span>{/if}
      </div>
    {/if}
  </div>

  <div class="card block">
    <h2>{t('about.cores')}</h2>
    <div class="cores">
      <div class="core">
        <span class="core-name">Xray-core</span>
        <span class="mono">{cores?.xray ?? t('about.coreMissing')}</span>
        <button class="btn small" disabled={busy !== null} onclick={() => updateCore('xray')}>
          <Icon name="download" size={14} />{busy === 'xray' ? '…' : t('about.updateCore')}
        </button>
      </div>
      <div class="core">
        <span class="core-name">sing-box</span>
        <span class="mono">{cores?.singbox ?? t('about.coreMissing')}</span>
        <button class="btn small" disabled={busy !== null} onclick={() => updateCore('singbox')}>
          <Icon name="download" size={14} />{busy === 'singbox' ? '…' : t('about.updateCore')}
        </button>
      </div>
      <div class="core">
        <span class="core-name">geoip / geosite</span>
        <span class:faint={!cores?.geo}>{cores?.geo ? t('about.geoPresent') : t('about.geoMissing')}</span>
        <button class="btn small" disabled={busy !== null} onclick={() => updateCore('geo')}>
          <Icon name="download" size={14} />{busy === 'geo' ? '…' : t('about.updateGeo')}
        </button>
      </div>
    </div>
    <div class="faint mono dir">{cores?.dir}</div>
  </div>

  <div class="card block">
    <h2>{t('about.dataDir')}</h2>
    <div class="row">
      <span class="mono faint">{app.dataDir}</span>
      <button class="btn small" onclick={() => api.openDataDir()}><Icon name="folder" size={14} />{t('settings.openDataDir')}</button>
    </div>
  </div>
</div>

<style>
  .hero {
    display: flex;
    gap: 20px;
    padding: 20px;
    align-items: flex-start;
  }

  .desc {
    max-width: 560px;
  }

  .block {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .update {
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
  }

  .notes {
    max-height: 200px;
    overflow: auto;
    margin: 0;
    padding: 10px;
    border-radius: 8px;
    background: var(--surface-2);
    white-space: pre-wrap;
    font-size: 12.5px;
    width: 100%;
  }

  .cores {
    display: flex;
    flex-direction: column;
  }

  .core {
    display: grid;
    grid-template-columns: 160px 1fr auto;
    align-items: center;
    gap: 12px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border);
  }

  .core:last-child {
    border-bottom: none;
  }

  .core-name {
    font-weight: 600;
  }

  .dir {
    font-size: 12px;
  }

  .error {
    color: var(--danger);
    font-size: 12.5px;
  }
</style>
