<script lang="ts">
  import LatencyChart from '../components/LatencyChart.svelte';
  import SpeedChart from '../components/SpeedChart.svelte';
  import Toggle from '../components/Toggle.svelte';
  import { bytes, delayClass, flag, speed } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app, saveSettings } from '../lib/store.svelte';

  const isActive = (id: string) => app.connection.connected && app.connection.profileId === id;
  // The server in use comes first, then the others from the fastest.
  const nodes = $derived(
    Object.values(app.nodeStats)
      .map((stats) => ({ stats, profile: app.profiles.find((p) => p.id === stats.profileId) }))
      .filter((n) => n.profile)
      .sort((a, b) => Number(isActive(b.stats.profileId)) - Number(isActive(a.stats.profileId)) || (a.stats.avg ?? 1e9) - (b.stats.avg ?? 1e9)),
  );

  let picked = $state<string | null>(null);
  // Follows the connected server until another one is picked; falls back when it disappears.
  const current = $derived(nodes.find((n) => n.stats.profileId === picked) ?? nodes[0] ?? null);

  const fmt = (v: number | null) => (v == null ? '—' : `${Math.round(v)}`);
  const nameOf = (p: { name: string; address: string } | undefined) => (p ? p.name.replace(/^\s*(?:[\u{1F1E6}-\u{1F1FF}]){2}\s*/u, '') || p.address : '');
</script>

<div class="page">
  <div class="page-header">
    <div>
      <h1>{t('analytics.title')}</h1>
      <div class="muted">{t('analytics.subtitle')}</div>
    </div>
    <div class="spacer"></div>
    {#if app.settings}
      <div class="controls">
        <div class="toggle-wrap">
          <Toggle bind:checked={app.settings.analysis.enabled} label={t('settings.analysisEnabled')} onchange={() => saveSettings(true)} />
        </div>
        <select class="select" bind:value={app.settings.analysis.scope} onchange={() => saveSettings(true)} title={t('analytics.scope')}>
          <option value="active">{t('settings.scope.active')}</option>
          <option value="group">{t('settings.scope.group')}</option>
          <option value="all">{t('settings.scope.all')}</option>
        </select>
        <select class="select" bind:value={app.settings.analysis.method} onchange={() => saveSettings(true)} title={t('settings.analysisMethod')}>
          <option value="tcp">{t('settings.method.tcp')}</option>
          <option value="icmp">{t('settings.method.icmp')}</option>
          <option value="real">{t('settings.method.real')}</option>
        </select>
        <select class="select" bind:value={app.settings.analysis.intervalSecs} onchange={() => saveSettings(true)} title={t('analytics.interval')}>
          {#each [1, 2, 5, 10, 30, 60] as s}<option value={s}>{s} s</option>{/each}
        </select>
      </div>
    {/if}
  </div>

  <div class="card speed">
    <div class="speed-head">
      <h2>{t('analytics.speed')}</h2>
      <div class="spacer"></div>
      <div class="legend"><span class="sw up"></span>{t('analytics.upload')} <b>{speed(app.traffic.up)}</b></div>
      <div class="legend"><span class="sw down"></span>{t('analytics.download')} <b>{speed(app.traffic.down)}</b></div>
    </div>
    <SpeedChart points={app.speedHistory} />
    <div class="totals faint">
      <span>{t('analytics.total')}: ↑ {bytes(app.traffic.totalUp)} · ↓ {bytes(app.traffic.totalDown)}</span>
      <span>{t('analytics.direct')}: ↑ {speed(app.traffic.directUp)} · ↓ {speed(app.traffic.directDown)}</span>
    </div>
  </div>

  {#if !app.settings?.analysis.enabled}
    <div class="card note muted">{t('analytics.disabled')}</div>
  {:else if nodes.length === 0}
    <div class="card note muted">{t('analytics.noData')}</div>
  {:else}
    <div class="split">
      <div class="card list" role="listbox" aria-label={t('analytics.servers')}>
        {#each nodes as { stats, profile } (stats.profileId)}
          <button
            class="item"
            class:selected={current?.stats.profileId === stats.profileId}
            role="option"
            aria-selected={current?.stats.profileId === stats.profileId}
            data-testid="node-{stats.profileId}"
            onclick={() => (picked = stats.profileId)}
          >
            <span class="avatar">{flag(profile?.name ?? '') ?? '·'}</span>
            <span class="item-text">
              <span class="item-name">{nameOf(profile)}</span>
              {#if isActive(stats.profileId)}<span class="badge-now">{t('analytics.inUse')}</span>{/if}
            </span>
            <span class="item-delay {delayClass(stats.last ?? -1)}">{stats.last == null ? '—' : stats.last}</span>
          </button>
        {/each}
        {#if nodes.length < 2 && app.settings?.analysis.scope === 'active'}
          <p class="faint hint">{t('analytics.scopeHint')}</p>
        {/if}
      </div>

      {#if current}
        {@const stats = current.stats}
        <div class="card detail" class:active={isActive(stats.profileId)} data-testid="node-detail">
          <div class="detail-head">
            <span class="avatar big">{flag(current.profile?.name ?? '') ?? '·'}</span>
            <div class="detail-title">
              <h2>{nameOf(current.profile)}</h2>
              <span class="faint">{current.profile?.address}:{current.profile?.port}{#if isActive(stats.profileId)} · {t('analytics.inUse')}{/if}</span>
            </div>
            <span class="now {delayClass(stats.last ?? -1)}">{stats.last == null ? t('servers.timeout') : `${stats.last} ${t('common.ms')}`}</span>
          </div>
          <LatencyChart values={stats.history} />
          <div class="metrics">
            <div><span class="faint">{t('analytics.avg')}</span><b>{fmt(stats.avg)}</b></div>
            <div><span class="faint">{t('analytics.min')}</span><b>{fmt(stats.min)}</b></div>
            <div><span class="faint">{t('analytics.max')}</span><b>{fmt(stats.max)}</b></div>
            <div><span class="faint">{t('analytics.jitter')}</span><b>{fmt(stats.jitter)}</b></div>
            <div><span class="faint">{t('analytics.loss')}</span><b class:bad={stats.loss > 0}>{stats.loss.toFixed(0)}%</b></div>
          </div>
          <div class="faint probes">{t('analytics.probes', { sent: stats.sent, lost: stats.lost })}</div>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .toggle-wrap {
    width: 230px;
  }

  .speed {
    padding: 14px 16px 10px;
  }

  .speed-head {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 6px;
  }

  .legend {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
  }

  .sw {
    width: 10px;
    height: 10px;
    border-radius: 3px;
  }

  .sw.up {
    background: var(--chart-up);
  }

  .sw.down {
    background: var(--chart-down);
  }

  .totals {
    display: flex;
    justify-content: space-between;
    font-size: 12.5px;
    margin-top: 4px;
  }

  .note {
    padding: 22px;
    text-align: center;
  }

  .split {
    display: grid;
    grid-template-columns: minmax(220px, 300px) 1fr;
    gap: 12px;
    align-items: start;
    min-width: 0;
  }

  .list {
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 520px;
    overflow: auto;
  }

  .item {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 8px;
    border: none;
    border-radius: 8px;
    background: none;
    text-align: left;
    cursor: pointer;
    min-width: 0;
  }

  .item:hover {
    background: var(--surface-2);
  }

  .item.selected {
    background: var(--surface-3);
  }

  .avatar {
    flex: none;
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    background: var(--surface-3);
    font-size: 17px;
  }

  .avatar.big {
    width: 38px;
    height: 38px;
    font-size: 24px;
  }

  .item-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .item-name {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .badge-now {
    font-size: 11px;
    color: var(--success);
  }

  .item-delay {
    font-variant-numeric: tabular-nums;
    font-size: 12.5px;
  }

  .hint {
    font-size: 12px;
    padding: 6px 8px 2px;
    margin: 0;
  }

  .detail {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  .detail.active {
    border-color: var(--success);
  }

  .detail-head {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .detail-title {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .detail-title h2 {
    margin: 0;
    font-size: 16px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail-title span {
    font-size: 12.5px;
  }

  .now {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .metrics {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
    font-size: 12.5px;
  }

  .metrics div {
    display: flex;
    flex-direction: column;
  }

  .metrics b {
    font-size: 16px;
    font-variant-numeric: tabular-nums;
  }

  .metrics b.bad {
    color: var(--danger);
  }

  .probes {
    font-size: 11.5px;
  }
</style>
