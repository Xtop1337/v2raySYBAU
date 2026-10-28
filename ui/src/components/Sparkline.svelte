<script lang="ts">
  // Latency sparkline: -1 samples (lost probes) are drawn as red ticks at the bottom.
  let { values, width = 220, height = 44 }: { values: number[]; width?: number; height?: number } = $props();

  const pad = 3;
  const ok = $derived(values.filter((v) => v >= 0));
  const max = $derived(Math.max(50, ...ok) * 1.15);
  const step = $derived(values.length > 1 ? (width - pad * 2) / (values.length - 1) : 0);
  const y = (v: number) => height - pad - (v / max) * (height - pad * 2);

  const line = $derived.by(() => {
    let d = '';
    let pen = false;
    values.forEach((v, i) => {
      if (v < 0) {
        pen = false;
        return;
      }
      d += `${pen ? 'L' : 'M'}${(pad + i * step).toFixed(1)},${y(v).toFixed(1)}`;
      pen = true;
    });
    return d;
  });
</script>

<svg {width} {height} viewBox="0 0 {width} {height}" class="spark">
  <path d={line} fill="none" stroke="var(--accent)" stroke-width="1.8" stroke-linejoin="round" stroke-linecap="round" />
  {#each values as v, i}
    {#if v < 0}
      <line x1={pad + i * step} x2={pad + i * step} y1={height - pad} y2={height - pad - 8} stroke="var(--danger)" stroke-width="2" />
    {/if}
  {/each}
</svg>

<style>
  .spark {
    display: block;
    max-width: 100%;
  }
</style>
