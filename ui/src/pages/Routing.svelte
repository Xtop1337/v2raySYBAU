<script lang="ts">
  import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
  import Icon from '../components/Icon.svelte';
  import Toggle from '../components/Toggle.svelte';
  import { newId } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app, errorText, saveSettings, toast } from '../lib/store.svelte';
  import type { RoutingPreset, RoutingRule, RuleKind } from '../lib/types';

  const presets: RoutingPreset[] = ['global', 'bypassLan', 'bypassLanRu', 'directByDefault'];
  const kinds: RuleKind[] = ['domain', 'ip', 'port', 'process', 'protocol', 'network'];
  const placeholders: Record<RuleKind, string> = {
    domain: 'geosite:youtube, domain:example.com',
    ip: 'geoip:ru, 10.0.0.0/8',
    port: '443, 1000-2000',
    process: 'chrome.exe, Telegram.exe',
    protocol: 'bittorrent',
    network: 'udp',
  };

  const routing = $derived(app.settings!.routing);

  function changed() {
    saveSettings();
  }

  function addRule() {
    routing.rules.push({ id: newId(), enabled: true, remark: '', kind: 'domain', values: [], outbound: 'proxy' });
    changed();
  }

  function move(index: number, delta: number) {
    const target = index + delta;
    if (target < 0 || target >= routing.rules.length) return;
    const [rule] = routing.rules.splice(index, 1);
    routing.rules.splice(target, 0, rule);
    changed();
  }

  function remove(index: number) {
    routing.rules.splice(index, 1);
    changed();
  }

  function setValues(rule: RoutingRule, text: string) {
    rule.values = text
      .split(/[\n,]/)
      .map((v) => v.trim())
      .filter(Boolean);
    changed();
  }

  async function exportRules() {
    await writeText(JSON.stringify($state.snapshot(routing.rules), null, 2));
    toast(t('common.copied'), 'success');
  }

  async function importRules() {
    try {
      const text = (await readText()) ?? '';
      const parsed = JSON.parse(text);
      const list: Partial<RoutingRule>[] = Array.isArray(parsed) ? parsed : (parsed.rules ?? parsed.routing?.rules ?? []);
      let n = 0;
      for (const r of list) {
        if (!r.kind || !Array.isArray(r.values)) continue;
        routing.rules.push({
          id: newId(),
          enabled: r.enabled ?? true,
          remark: r.remark ?? '',
          kind: r.kind,
          values: r.values,
          outbound: r.outbound ?? 'proxy',
        });
        n++;
      }
      changed();
      toast(t('routing.importedRules', { n }), 'success');
    } catch (e) {
      toast(`${t('settings.invalidJson')}: ${errorText(e)}`, 'error');
    }
  }
</script>

<div class="page">
  <div class="page-header"><h1>{t('routing.title')}</h1></div>

  <div class="card block">
    <div class="section-title">{t('routing.preset')}</div>
    <div class="presets">
      {#each presets as preset}
        <button class="preset" class:active={routing.preset === preset} onclick={() => ((routing.preset = preset), changed())}>
          <span class="radio"></span>{t(`routing.preset.${preset}`)}
        </button>
      {/each}
    </div>
    <div class="row opts">
      <label class="row"><span class="muted">{t('routing.domainStrategy')}</span>
        <select class="select" bind:value={routing.domainStrategy} onchange={changed}>
          <option value="AsIs">AsIs</option>
          <option value="IPIfNonMatch">IPIfNonMatch</option>
          <option value="IPOnDemand">IPOnDemand</option>
        </select>
      </label>
      <div class="toggle"><Toggle bind:checked={routing.blockAds} label={t('routing.blockAds')} onchange={changed} /></div>
    </div>
  </div>

  <div class="card block">
    <div class="row">
      <h2>{t('routing.rules')}</h2>
      <div class="spacer"></div>
      <button class="btn small" onclick={importRules}><Icon name="clipboard" size={14} />{t('routing.importRules')}</button>
      <button class="btn small" onclick={exportRules}><Icon name="copy" size={14} />{t('routing.exportRules')}</button>
      <button class="btn small primary" onclick={addRule}><Icon name="plus" size={14} />{t('routing.addRule')}</button>
    </div>
    <p class="faint hint">{t('routing.rulesHint')}</p>
    {#if routing.rules.length === 0}
      <div class="faint empty">{t('routing.noRules')}</div>
    {/if}
    {#each routing.rules as rule, index (rule.id)}
      <div class="rule" class:off={!rule.enabled}>
        <input type="checkbox" bind:checked={rule.enabled} onchange={changed} />
        <select class="select" bind:value={rule.kind} onchange={changed}>
          {#each kinds as kind}<option value={kind}>{t(`routing.kind.${kind}`)}</option>{/each}
        </select>
        <textarea
          class="textarea values"
          rows="1"
          placeholder={placeholders[rule.kind]}
          value={rule.values.join(', ')}
          onchange={(e) => setValues(rule, e.currentTarget.value)}
        ></textarea>
        <select class="select out out-{rule.outbound}" bind:value={rule.outbound} onchange={changed}>
          <option value="proxy">{t('routing.out.proxy')}</option>
          <option value="direct">{t('routing.out.direct')}</option>
          <option value="block">{t('routing.out.block')}</option>
        </select>
        <input class="input remark" placeholder={t('routing.remark')} bind:value={rule.remark} onchange={changed} />
        <button class="btn ghost icon small" onclick={() => move(index, -1)} aria-label="up"><Icon name="up" size={15} /></button>
        <button class="btn ghost icon small" onclick={() => move(index, 1)} aria-label="down"><Icon name="down" size={15} /></button>
        <button class="btn ghost icon small danger" onclick={() => remove(index)} aria-label="delete"><Icon name="trash" size={15} /></button>
      </div>
    {/each}
  </div>
</div>

<style>
  .block {
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .presets {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 8px;
  }

  .preset {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 11px 12px;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--surface);
    cursor: pointer;
    text-align: left;
  }

  .preset.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .radio {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid var(--border-strong);
    flex: none;
  }

  .preset.active .radio {
    border: 5px solid var(--accent);
  }

  .opts {
    gap: 24px;
    flex-wrap: wrap;
  }

  .toggle {
    min-width: 320px;
  }

  .hint {
    margin: 0;
    font-size: 12.5px;
  }

  .empty {
    padding: 14px 0;
  }

  .rule {
    display: grid;
    grid-template-columns: 20px 150px minmax(200px, 1fr) 130px 160px 28px 28px 28px;
    gap: 8px;
    align-items: center;
  }

  .rule.off {
    opacity: 0.55;
  }

  .values {
    height: 34px;
    min-height: 34px;
    resize: vertical;
    padding-top: 7px;
  }

  .out-proxy {
    color: var(--accent);
  }

  .out-direct {
    color: var(--success);
  }

  .out-block {
    color: var(--danger);
  }
</style>
