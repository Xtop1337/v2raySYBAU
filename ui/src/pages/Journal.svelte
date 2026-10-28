<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog';
  import Icon from '../components/Icon.svelte';
  import { api } from '../lib/api';
  import { time } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app, errorText, toast } from '../lib/store.svelte';
  import type { LogEntry } from '../lib/types';

  let tab = $state<'requests' | 'events'>('requests');
  let route = $state('any');
  let level = $state('all');
  let search = $state('');
  const LIMIT = 1500;

  const requests = $derived(app.logs.filter((e) => e.source === 'access'));
  const counts = $derived.by(() => {
    const c = { proxy: 0, direct: 0, block: 0 };
    for (const e of requests) {
      const out = e.request?.outbound ?? '';
      if (out === 'proxy') c.proxy++;
      else if (out === 'direct') c.direct++;
      else if (out === 'block' || e.request?.accepted === false) c.block++;
    }
    return c;
  });

  function matches(e: LogEntry): boolean {
    if (tab === 'requests') {
      if (e.source !== 'access') return false;
      if (route !== 'any' && (e.request?.outbound ?? '') !== route) return false;
    } else {
      if (e.source === 'access') return false;
      if (level !== 'all' && e.level !== level) return false;
    }
    if (search.trim() && !e.message.toLowerCase().includes(search.toLowerCase())) return false;
    return true;
  }

  const rows = $derived(app.logs.filter(matches).slice(-LIMIT).reverse());

  async function exportLogs(format: 'csv' | 'json') {
    const path = await save({ defaultPath: `v2raysybau-log.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
    if (!path) return;
    await api.exportLogs(path, format).then(
      () => toast(t('common.saved'), 'success'),
      (e) => toast(errorText(e), 'error'),
    );
  }

  async function clear() {
    await api.clearLogs();
    app.logs = [];
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('journal.title')}</h1>
    <div class="tabs">
      <button class:active={tab === 'requests'} onclick={() => (tab = 'requests')}>{t('journal.requests')} <span class="faint">{requests.length}</span></button>
      <button class:active={tab === 'events'} onclick={() => (tab = 'events')}>{t('journal.events')}</button>
    </div>
    <div class="spacer"></div>
    <button class="btn small" onclick={() => (app.logsPaused = !app.logsPaused)}>
      <Icon name={app.logsPaused ? 'play' : 'pause'} size={14} />{app.logsPaused ? t('journal.resume') : t('journal.pause')}
    </button>
    <button class="btn small" onclick={clear}><Icon name="trash" size={14} />{t('journal.clear')}</button>
    <button class="btn small" onclick={() => exportLogs('csv')}><Icon name="download" size={14} />{t('journal.exportCsv')}</button>
    <button class="btn small" onclick={() => exportLogs('json')}>{t('journal.exportJson')}</button>
  </div>

  <div class="row filters">
    {#if tab === 'requests'}
      <select class="select" bind:value={route}>
        <option value="any">{t('journal.anyRoute')}</option>
        <option value="proxy">{t('routing.out.proxy')}</option>
        <option value="direct">{t('routing.out.direct')}</option>
        <option value="block">{t('routing.out.block')}</option>
      </select>
      <span class="faint stats">{t('journal.stats', { total: requests.length, ...counts })}</span>
    {:else}
      <select class="select" bind:value={level}>
        <option value="all">{t('journal.all')}</option>
        <option value="error">error</option>
        <option value="warning">warning</option>
        <option value="info">info</option>
        <option value="debug">debug</option>
      </select>
    {/if}
    <div class="spacer"></div>
    <input class="input" placeholder={t('common.search')} bind:value={search} spellcheck="false" />
  </div>

  {#if tab === 'requests' && app.settings && !app.settings.advanced.accessLog}
    <div class="card note muted">{t('journal.accessOff')}</div>
  {/if}

  <div class="card log">
    {#if rows.length === 0}
      <div class="note faint">{t('journal.empty')}</div>
    {:else if tab === 'requests'}
      <div class="thead req"><span>{t('journal.time')}</span><span>{t('journal.from')}</span><span>{t('journal.destination')}</span><span>{t('journal.route')}</span></div>
      <div class="tbody">
        {#each rows as e, i (i + e.at)}
          <div class="tr req mono">
            <span class="faint">{time(e.at)}</span>
            <span class="faint ellipsis">{e.request?.source}</span>
            <span class="ellipsis" title={e.request?.destination}>{e.request?.network ? `${e.request.network}:` : ''}{e.request?.destination}</span>
            <span><span class="route {e.request?.accepted === false ? 'block' : e.request?.outbound}">{e.request?.inbound} → {e.request?.outbound || '✕'}</span></span>
          </div>
        {/each}
      </div>
    {:else}
      <div class="thead ev"><span>{t('journal.time')}</span><span>{t('journal.level')}</span><span>{t('journal.message')}</span></div>
      <div class="tbody">
        {#each rows as e, i (i + e.at)}
          <div class="tr ev mono">
            <span class="faint">{time(e.at)}</span>
            <span class="lvl {e.level}">{e.level}</span>
            <span class="msg" title={e.message}><span class="faint">[{e.source}]</span> {e.message}</span>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .tabs {
    display: flex;
    gap: 2px;
    padding: 3px;
    background: var(--surface-3);
    border-radius: 9px;
  }

  .tabs button {
    border: none;
    background: none;
    padding: 5px 12px;
    border-radius: 7px;
    cursor: pointer;
  }

  .tabs button.active {
    background: var(--surface);
    box-shadow: var(--shadow);
  }

  .filters .input {
    width: 240px;
  }

  .stats {
    font-size: 12.5px;
  }

  .note {
    padding: 18px;
    text-align: center;
  }

  .log {
    flex: 1;
    min-height: 200px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .thead,
  .tr {
    display: grid;
    gap: 12px;
    padding: 0 14px;
    align-items: center;
  }

  .req {
    grid-template-columns: 80px minmax(120px, 1fr) minmax(200px, 3fr) 150px;
  }

  .ev {
    grid-template-columns: 80px 70px 1fr;
  }

  .thead {
    height: 34px;
    font-size: 12px;
    color: var(--text-3);
    background: var(--surface-2);
    border-bottom: 1px solid var(--border);
  }

  .tbody {
    flex: 1;
    overflow: auto;
  }

  .tr {
    min-height: 28px;
    border-bottom: 1px solid var(--border);
    user-select: text;
  }

  .ellipsis,
  .msg {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .route {
    font-size: 11.5px;
    padding: 1px 7px;
    border-radius: 5px;
    background: var(--surface-3);
  }

  .route.proxy {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .route.direct {
    background: var(--success-soft);
    color: var(--success);
  }

  .route.block {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .lvl.error {
    color: var(--danger);
  }

  .lvl.warning {
    color: var(--warning);
  }

  .lvl.debug {
    color: var(--text-3);
  }
</style>
