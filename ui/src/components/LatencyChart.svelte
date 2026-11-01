<script lang="ts">
  // Latency over the analysis window for one server: a line with a soft area under it, grid
  // lines with values, and red ticks for lost probes (-1 samples). Fills the width it is given.
  import { t } from '../lib/i18n.svelte';

  let { values, height = 190 }: { values: number[]; height?: number } = $props();

  let width = $state(600);
  const pad = { left: 44, right: 10, top: 10, bottom: 16 };
  const ok = $derived(values.filter((v) => v >= 0));
  // Four equal steps of a round size keep the grid labels readable (0, 25, 50, 75, 100).
  const STEPS = [5, 10, 20, 25, 50, 100, 200, 250, 500, 1000, 2000, 5000];
  const step = $derived.by(() => {
    const raw = Math.max(20, ...ok) * 1.1;
    return STEPS.find((s) => s * 4 >= raw) ?? Math.ceil(raw / 4000) * 1000;
  });
  const top = $derived(step * 4);
  const plotW = $derived(Math.max(10, width - pad.left - pad.right));
  const plotH = $derived(height - pad.top - pad.bottom);
  const x = (i: number) => pad.left + (values.length > 1 ? (i / (values.length - 1)) * plotW : plotW / 2);
  const y = (v: number) => pad.top + plotH - (v / top) * plotH;

  // Separate segments between lost probes.
  const segments = $derived.by(() => {
    const result: [number, number][][] = [];
    let current: [number, number][] = [];
    values.forEach((v, i) => {
      if (v < 0) {
        if (current.length) result.push(current);
        current = [];
      } else {
        current.push([x(i), y(v)]);
      }
    });
    if (current.length) result.push(current);
    return result;
  });
  const line = (points: [number, number][]) => points.map(([px, py], i) => `${i ? 'L' : 'M'}${px.toFixed(1)},${py.toFixed(1)}`).join('');
  const area = (points: [number, number][]) =>
    `${line(points)}L${points[points.length - 1][0].toFixed(1)},${pad.top + plotH}L${points[0][0].toFixed(1)},${pad.top + plotH}Z`;
  const grid = $derived([0, 1, 2, 3, 4].map((i) => i * step));
</script>

<div class="chart" bind:clientWidth={width}>
  {#if values.length === 0}
    <div class="empty faint" style:height="{height}px">{t('analytics.waiting')}</div>
  {:else}
    <svg {width} {height} viewBox="0 0 {width} {height}" role="img" aria-label={t('analytics.latency')}>
      {#each grid as g}
        <line class="grid" x1={pad.left} x2={pad.left + plotW} y1={y(g)} y2={y(g)} />
        <text class="label" x={pad.left - 6} y={y(g) + 4} text-anchor="end">{g}</text>
      {/each}
      {#each segments as points}
        {#if points.length > 1}
          <path class="area" d={area(points)} />
          <path class="line" d={line(points)} />
        {:else}
          <circle class="dot" cx={points[0][0]} cy={points[0][1]} r="2.5" />
        {/if}
      {/each}
      {#each values as v, i}
        {#if v < 0}
          <line class="lost" x1={x(i)} x2={x(i)} y1={pad.top + plotH} y2={pad.top + plotH - 12} />
        {/if}
      {/each}
      <text class="label" x={pad.left} y={height - 2}>{t('analytics.older')}</text>
      <text class="label" x={pad.left + plotW} y={height - 2} text-anchor="end">{t('analytics.now')}</text>
    </svg>
  {/if}
</div>

<style>
  .chart {
    width: 100%;
  }

  svg {
    display: block;
  }

  .grid {
    stroke: var(--border);
    stroke-width: 1;
  }

  .label {
    fill: var(--text-3);
    font-size: 10.5px;
    font-variant-numeric: tabular-nums;
  }

  .area {
    fill: var(--accent);
    opacity: 0.1;
  }

  .line {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
  }

  .dot {
    fill: var(--accent);
  }

  .lost {
    stroke: var(--danger);
    stroke-width: 2;
  }

  .empty {
    display: grid;
    place-items: center;
    font-size: 13px;
  }
</style>
