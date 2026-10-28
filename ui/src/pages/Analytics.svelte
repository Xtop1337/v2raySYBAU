<script lang="ts">
  import Sparkline from '../components/Sparkline.svelte';
  import SpeedChart from '../components/SpeedChart.svelte';
  import Toggle from '../components/Toggle.svelte';
  import { bytes, delayClass, speed } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app, saveSettings } from '../lib/store.svelte';

  const nodes = $derived(
    Object.values(app.nodeStats)
      .map((stats) => ({ stats, profile: app.profiles.find((p) => p.id === stats.profileId) }))
      .filter((n) => n.profile)
      .sort((a, b) => (a.stats.avg ?? 1e9) - (b.stats.avg ?? 1e9)),
  );

  const fmt = (v: number | null) => (v == null ? '—' : `${Math.round(v)}`);
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
    <div class="nodes">
      {#each nodes as { stats, profile } (stats.profileId)}
        <div class="card node" class:active={app.connection.profileId === stats.profileId && app.connection.connected}>
          <div class="node-head">
            <span class="node-name">{profile?.name || profile?.address}</span>
            <span class="now {delayClass(stats.last ?? -1)}">{stats.last == null ? t('servers.timeout') : `${stats.last} ${t('common.ms')}`}</span>
          </div>
          <Sparkline values={stats.history} width={300} height={46} />
          <div class="metrics">
            <div><span class="faint">{t('analytics.avg')}</span><b>{fmt(stats.avg)}</b></div>
            <div><span class="faint">{t('analytics.min')}</span><b>{fmt(stats.min)}</b></div>
            <div><span class="faint">{t('analytics.max')}</span><b>{fmt(stats.max)}</b></div>
            <div><span class="faint">{t('analytics.jitter')}</span><b>{fmt(stats.jitter)}</b></div>
            <div><span class="faint">{t('analytics.loss')}</span><b class:bad={stats.loss > 0}>{stats.loss.toFixed(0)}%</b></div>
          </div>
          <div class="faint probes">{t('analytics.probes', { sent: stats.sent, lost: stats.lost })}</div>
        </div>
      {/each}
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

  .nodes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }

  .node {
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .node.active {
    border-color: var(--success);
  }

  .node-head {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .node-name {
    flex: 1;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .now {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .metrics {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 4px;
    font-size: 12px;
  }

  .metrics div {
    display: flex;
    flex-direction: column;
  }

  .metrics b {
    font-size: 14px;
    font-variant-numeric: tabular-nums;
  }

  .metrics b.bad {
    color: var(--danger);
  }

  .probes {
    font-size: 11.5px;
  }
</style>
