<script lang="ts">
  import { speed } from '../lib/format';

  // Live area chart of upload/download speed (one point per second).
  let { points, height = 180 }: { points: { up: number; down: number }[]; height?: number } = $props();

  let width = $state(600);
  const capacity = 90;
  const pad = { top: 10, bottom: 18, left: 4, right: 4 };
  const max = $derived(Math.max(16 * 1024, ...points.map((p) => Math.max(p.up, p.down))) * 1.2);
  const innerH = $derived(height - pad.top - pad.bottom);
  const x = (i: number) => pad.left + ((width - pad.left - pad.right) * (i + capacity - points.length)) / (capacity - 1);
  const y = (v: number) => pad.top + innerH - (v / max) * innerH;

  function area(key: 'up' | 'down') {
    if (points.length < 2) return { line: '', fill: '' };
    const line = points.map((p, i) => `${i ? 'L' : 'M'}${x(i).toFixed(1)},${y(p[key]).toFixed(1)}`).join('');
    const fill = `${line}L${x(points.length - 1).toFixed(1)},${pad.top + innerH}L${x(0).toFixed(1)},${pad.top + innerH}Z`;
    return { line, fill };
  }

  const down = $derived(area('down'));
  const up = $derived(area('up'));
</script>

<div class="chart" bind:clientWidth={width}>
  <svg {width} {height} viewBox="0 0 {width} {height}">
    {#each [0.25, 0.5, 0.75] as f}
      <line x1="0" x2={width} y1={pad.top + innerH * f} y2={pad.top + innerH * f} stroke="var(--border)" stroke-dasharray="3 4" />
    {/each}
    <path d={down.fill} fill="var(--chart-down)" opacity="0.12" />
    <path d={down.line} fill="none" stroke="var(--chart-down)" stroke-width="2" />
    <path d={up.fill} fill="var(--chart-up)" opacity="0.1" />
    <path d={up.line} fill="none" stroke="var(--chart-up)" stroke-width="2" />
    <text x={width - 6} y="14" text-anchor="end" class="axis">{speed(Math.round(max))}</text>
  </svg>
</div>

<style>
  .chart {
    width: 100%;
  }

  svg {
    display: block;
  }

  .axis {
    font-size: 11px;
    fill: var(--text-3);
  }
</style>
