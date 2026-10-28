<script lang="ts">
  let {
    checked = $bindable(false),
    label = '',
    hint = '',
    disabled = false,
    onchange,
  }: { checked?: boolean; label?: string; hint?: string; disabled?: boolean; onchange?: (value: boolean) => void } = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button type="button" class="toggle-row" class:disabled onclick={toggle} role="switch" aria-checked={checked}>
  <span class="text">
    <span>{label}</span>
    {#if hint}<span class="hint">{hint}</span>{/if}
  </span>
  <span class="switch" class:on={checked}><span class="knob"></span></span>
</button>

<style>
  .toggle-row {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 9px 0;
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
  }

  .toggle-row.disabled {
    opacity: 0.5;
    cursor: default;
  }

  .text {
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  .hint {
    font-size: 12.5px;
    color: var(--text-3);
  }

  .switch {
    position: relative;
    flex: none;
    width: 38px;
    height: 22px;
    border-radius: 11px;
    background: var(--border-strong);
    transition: background 0.15s;
  }

  .switch.on {
    background: var(--accent);
  }

  .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.25);
    transition: transform 0.15s;
  }

  .switch.on .knob {
    transform: translateX(16px);
  }
</style>
