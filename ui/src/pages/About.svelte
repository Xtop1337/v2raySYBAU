<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '../components/Icon.svelte';
  import Logo from '../components/Logo.svelte';
  import { api } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { hide } from '../lib/privacy';
  import { app, errorText, toast } from '../lib/store.svelte';
  import type { CoreVersions } from '../lib/types';
  import ReleaseNotesList from '../components/ReleaseNotesList.svelte';
  import { checkForUpdates, updates } from '../lib/updates.svelte';

  const REPO = 'https://github.com/Xtop1337/v2raySYBAU';
  // Shipped unmodified next to the app; full texts in the installed licenses folder.
  const components = [
    { name: 'Xray-core', license: 'MPL-2.0', url: 'https://github.com/XTLS/Xray-core' },
    { name: 'sing-box', license: 'GPL-3.0', url: 'https://github.com/SagerNet/sing-box' },
    { name: 'Wintun', license: 'Wintun Prebuilt Binaries License', url: 'https://www.wintun.net' },
    { name: 'geoip / geosite', license: 'GPL-3.0 · CC BY-SA 4.0 (GeoLite2, MaxMind)', url: 'https://github.com/Loyalsoldier/v2ray-rules-dat' },
    { name: 'Twemoji Country Flags', license: 'MIT · CC BY 4.0 (Twemoji)', url: 'https://github.com/talkjs/country-flag-emoji-polyfill' },
  ];
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
    <Logo size={72} />
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

  <div class="card block" data-testid="about-updates">
    <h2>{t('about.updates')}</h2>
    <p class="muted note">{t('about.updatesNote')}</p>
    <div class="row">
      <button class="btn" disabled={updates.checking} onclick={() => checkForUpdates(false)}>
        <Icon name="refresh" size={15} />{updates.checking ? t('about.checking') : t('about.checkApp')}
      </button>
      {#if updates.error}<span class="error">{updates.error}</span>{/if}
    </div>
    {#if updates.available}
      <div class="update">
        <b>{t('about.available', { v: updates.available.version })}</b>
        <ReleaseNotesList releases={updates.available.releases} />
        <div class="row">
          <button class="btn primary" onclick={() => updates.available && openUrl(updates.available.installerUrl ?? updates.available.pageUrl)}>
            <Icon name="download" size={15} />{t('update.download')}
          </button>
          <button class="btn" onclick={() => updates.available && openUrl(updates.available.pageUrl)}><Icon name="globe" size={15} />{t('about.openRelease')}</button>
        </div>
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
    <div class="faint mono dir">{hide(cores?.dir ?? '')}</div>
  </div>

  <div class="card block" data-testid="licenses">
    <h2>{t('about.licenses')}</h2>
    <p class="muted note">{t('about.licensesNote')}</p>
    <div class="cores">
      <div class="core">
        <span class="core-name">V2Ray Sybau</span>
        <span>MIT</span>
        <button class="btn small" onclick={() => openUrl(`${REPO}/blob/main/LICENSE`)}><Icon name="link" size={14} />{t('about.license')}</button>
      </div>
      {#each components as c}
        <div class="core">
          <span class="core-name">{c.name}</span>
          <span>{c.license}</span>
          <button class="btn small" onclick={() => openUrl(c.url)}><Icon name="globe" size={14} />{t('about.source')}</button>
        </div>
      {/each}
    </div>
    <div class="row">
      <button class="btn small" onclick={() => api.openLicensesDir().catch((e) => toast(errorText(e), 'error'))}>
        <Icon name="folder" size={14} />{t('about.openLicenses')}
      </button>
      <button class="btn small" onclick={() => openUrl(`${REPO}/blob/main/THIRD-PARTY-NOTICES.md`)}><Icon name="file" size={14} />THIRD-PARTY-NOTICES</button>
    </div>
  </div>

  <div class="card block">
    <h2>{t('about.dataDir')}</h2>
    <div class="row">
      <span class="mono faint">{hide(app.dataDir)}</span>
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

  .note {
    margin: 0;
    font-size: 12.5px;
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
