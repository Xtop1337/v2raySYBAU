<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog';
  import Icon from '../components/Icon.svelte';
  import Modal from '../components/Modal.svelte';
  import Toggle from '../components/Toggle.svelte';
  import { importFiles } from '../lib/actions';
  import { api } from '../lib/api';
  import { t } from '../lib/i18n.svelte';
  import { app, errorText, saveSettings, toast } from '../lib/store.svelte';

  const s = $derived(app.settings!);
  let newApp = $state('');
  let picker = $state<{ list: string[]; filter: string } | null>(null);
  let overrideError = $state('');

  const sections = [
    ['general', 'settings.general'],
    ['connection', 'settings.connection'],
    ['dns', 'settings.dns'],
    ['routing', 'settings.routing'],
    ['mux', 'settings.mux'],
    ['fragment', 'settings.fragment'],
    ['analysis', 'settings.analysis'],
    ['testing', 'settings.testing'],
    ['advanced', 'settings.advanced'],
  ] as const;

  const dnsPresets = [
    ['Cloudflare DoH', 'https://1.1.1.1/dns-query'],
    ['Google DoH', 'https://8.8.8.8/dns-query'],
    ['Quad9 DoH', 'https://9.9.9.9/dns-query'],
    ['AdGuard DoH', 'https://94.140.14.14/dns-query'],
    ['Cloudflare', '1.1.1.1'],
    ['Google', '8.8.8.8'],
    ['Yandex', '77.88.8.8'],
  ];

  const changed = () => saveSettings();
  const now = () => saveSettings(true);

  function addApp(name: string) {
    const value = name.trim();
    if (value && !s.tun.apps.some((a) => a.toLowerCase() === value.toLowerCase())) {
      s.tun.apps.push(value);
      now();
    }
    newApp = '';
  }

  function removeApp(name: string) {
    s.tun.apps = s.tun.apps.filter((a) => a !== name);
    now();
  }

  async function openPicker() {
    picker = { list: await api.processes(), filter: '' };
  }

  function checkOverride() {
    const text = s.advanced.configOverride.trim();
    if (!text) {
      overrideError = '';
      changed();
      return;
    }
    try {
      const parsed = JSON.parse(text);
      if (typeof parsed !== 'object' || Array.isArray(parsed)) throw new Error('object expected');
      overrideError = '';
      changed();
    } catch (e) {
      overrideError = `${t('settings.invalidJson')}: ${errorText(e)}`;
    }
  }

  async function exportBackup() {
    const path = await save({ defaultPath: 'v2raysybau-backup.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (!path) return;
    await api.exportBackup(path).then(
      () => toast(t('common.saved'), 'success'),
      (e) => toast(errorText(e), 'error'),
    );
  }

  function jump(id: string) {
    document.getElementById(`settings-${id}`)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
  }
</script>

<div class="page settings">
  <div class="page-header">
    <h1>{t('settings.title')}</h1>
  </div>
  <div class="jump">
    {#each sections as [id, key]}
      <button class="chip" onclick={() => jump(id)}>{t(key)}</button>
    {/each}
  </div>

  <section class="card" id="settings-general">
    <h2>{t('settings.general')}</h2>
    <div class="field">
      <span>{t('settings.language')}</span>
      <select class="select" bind:value={s.general.language} onchange={now}>
        <option value="auto">{t('settings.language.auto')}</option>
        <option value="ru">Русский</option>
        <option value="en">English</option>
      </select>
    </div>
    <div class="field">
      <span>{t('settings.theme')}</span>
      <div class="segmented">
        {#each ['system', 'light', 'dark'] as const as theme}
          <button class:active={s.general.theme === theme} onclick={() => ((s.general.theme = theme), now())}>{t(`settings.theme.${theme}`)}</button>
        {/each}
      </div>
    </div>
    <Toggle bind:checked={s.general.autostart} label={t('settings.autostart')} onchange={now} />
    <Toggle bind:checked={s.general.startMinimized} label={t('settings.startMinimized')} onchange={now} />
    <Toggle bind:checked={s.general.autoConnect} label={t('settings.autoConnect')} onchange={now} />
    <Toggle bind:checked={s.general.scanOnStart} label={t('settings.scanOnStart')} onchange={now} />
    <Toggle bind:checked={s.general.updateSubscriptionsOnStart} label={t('settings.updateSubsOnStart')} onchange={now} />
    <Toggle bind:checked={s.general.confirmDelete} label={t('settings.confirmDelete')} onchange={now} />
    <Toggle bind:checked={s.general.showSpeed} label={t('settings.showSpeed')} onchange={now} />
    <Toggle bind:checked={s.general.minimizeToTray} label={t('settings.minimizeToTray')} onchange={now} />
  </section>

  <section class="card" id="settings-connection">
    <h2>{t('settings.connection')}</h2>
    <div class="field">
      <span>{t('settings.mode')}</span>
      <div class="segmented">
        <button class:active={s.proxy.mode === 'proxy'} onclick={() => ((s.proxy.mode = 'proxy'), now())}>{t('settings.mode.proxy')}</button>
        <button class:active={s.proxy.mode === 'tun'} onclick={() => ((s.proxy.mode = 'tun'), now())}>{t('settings.mode.tun')}</button>
      </div>
    </div>
    {#if s.proxy.mode === 'tun' && !app.elevated}
      <div class="note warn">
        <span>{t('settings.adminNote')}</span>
        <button class="btn small" onclick={() => api.restartAsAdmin().catch((e) => toast(errorText(e), 'error'))}>{t('servers.restartAdmin')}</button>
      </div>
    {/if}
    <Toggle bind:checked={s.proxy.systemProxy} label={t('settings.systemProxy')} onchange={now} disabled={s.proxy.mode === 'tun'} />
    <div class="field"><span>{t('settings.socksPort')}</span><input class="input num" type="number" bind:value={s.proxy.socksPort} onchange={changed} /></div>
    <div class="field"><span>{t('settings.httpPort')}</span><input class="input num" type="number" bind:value={s.proxy.httpPort} onchange={changed} /></div>
    <Toggle bind:checked={s.proxy.allowLan} label={t('settings.allowLan')} onchange={now} />
    <div class="field col"><span>{t('settings.bypass')}</span><input class="input" bind:value={s.proxy.bypass} onchange={changed} /></div>

    <h3 class="sub">{t('settings.perApp')}</h3>
    <p class="faint hint">{t('settings.perAppHint')}</p>
    <div class="segmented">
      {#each ['all', 'include', 'exclude'] as const as mode}
        <button class:active={s.tun.appMode === mode} onclick={() => ((s.tun.appMode = mode), now())}>{t(`settings.appMode.${mode}`)}</button>
      {/each}
    </div>
    {#if s.tun.appMode !== 'all'}
      <div class="apps">
        {#each s.tun.apps as name}
          <span class="app-chip">{name}<button onclick={() => removeApp(name)} aria-label="remove"><Icon name="x" size={13} /></button></span>
        {/each}
      </div>
      <div class="row">
        <input class="input" placeholder="chrome.exe" bind:value={newApp} onkeydown={(e) => e.key === 'Enter' && addApp(newApp)} />
        <button class="btn" onclick={() => addApp(newApp)}><Icon name="plus" size={15} />{t('settings.addApp')}</button>
        <button class="btn" onclick={openPicker}>{t('settings.pickProcess')}</button>
      </div>
    {/if}
    <div class="field">
      <span>{t('settings.tunStack')}</span>
      <select class="select" bind:value={s.tun.stack} onchange={now}>
        <option value="mixed">mixed</option>
        <option value="system">system</option>
        <option value="gvisor">gvisor</option>
      </select>
    </div>
    <div class="field"><span>{t('settings.mtu')}</span><input class="input num" type="number" bind:value={s.tun.mtu} onchange={changed} /></div>
    <Toggle bind:checked={s.tun.strictRoute} label={t('settings.strictRoute')} onchange={now} />
  </section>

  <section class="card" id="settings-dns">
    <h2>{t('settings.dns')}</h2>
    <div class="field col">
      <span>{t('settings.remoteDns')}</span>
      <div class="row">
        <input class="input grow" bind:value={s.dns.remote} onchange={changed} />
        <select class="select" onchange={(e) => ((s.dns.remote = e.currentTarget.value), now(), (e.currentTarget.value = ''))}>
          <option value="">{t('settings.dnsPresets')}</option>
          {#each dnsPresets as [name, value]}<option {value}>{name}</option>{/each}
        </select>
      </div>
    </div>
    <div class="field col">
      <span>{t('settings.directDns')}</span>
      <div class="row">
        <input class="input grow" bind:value={s.dns.direct} onchange={changed} />
        <select class="select" onchange={(e) => ((s.dns.direct = e.currentTarget.value), now(), (e.currentTarget.value = ''))}>
          <option value="">{t('settings.dnsPresets')}</option>
          {#each dnsPresets as [name, value]}<option {value}>{name}</option>{/each}
        </select>
      </div>
    </div>
    <Toggle bind:checked={s.dns.useProfileDns} label={t('settings.useProfileDns')} onchange={now} />
    <Toggle bind:checked={s.dns.localDns} label={t('settings.localDns')} onchange={now} />
    {#if s.dns.localDns}
      <div class="field"><span>{t('settings.localDnsPort')}</span><input class="input num" type="number" bind:value={s.dns.localPort} onchange={changed} /></div>
    {/if}
    <Toggle bind:checked={s.dns.fakeDns} label={t('settings.fakeDns')} onchange={now} />
    <div class="field">
      <span>{t('settings.queryStrategy')}</span>
      <select class="select" bind:value={s.dns.queryStrategy} onchange={now}>
        <option value="UseIP">UseIP</option>
        <option value="UseIPv4">UseIPv4</option>
        <option value="UseIPv6">UseIPv6</option>
      </select>
    </div>
    <div class="field col"><span>{t('settings.hosts')}</span><textarea class="textarea" rows="3" bind:value={s.dns.hosts} onchange={changed}></textarea></div>
  </section>

  <section class="card" id="settings-routing">
    <h2>{t('settings.routing')}</h2>
    <Toggle bind:checked={s.routing.sniffing} label={t('settings.sniffing')} onchange={now} />
    <Toggle bind:checked={s.routing.routeOnly} label={t('settings.routeOnly')} onchange={now} />
    <button class="btn small link" onclick={() => (app.page = 'routing')}><Icon name="routing" size={14} />{t('nav.routing')} →</button>
  </section>

  <section class="card" id="settings-mux">
    <h2>{t('settings.mux')}</h2>
    <Toggle bind:checked={s.mux.enabled} label={t('settings.muxEnabled')} hint={t('settings.muxNote')} onchange={now} />
    {#if s.mux.enabled}
      <div class="field"><span>{t('settings.muxConcurrency')}</span><input class="input num" type="number" min="-1" max="1024" bind:value={s.mux.concurrency} onchange={changed} /></div>
      <div class="field"><span>{t('settings.xudpConcurrency')}</span><input class="input num" type="number" min="-1" max="1024" bind:value={s.mux.xudpConcurrency} onchange={changed} /></div>
      <div class="field">
        <span>{t('settings.xudpUdp443')}</span>
        <select class="select" bind:value={s.mux.xudpProxyUdp443} onchange={now}>
          <option value="reject">reject</option>
          <option value="allow">allow</option>
          <option value="skip">skip</option>
        </select>
      </div>
    {/if}
  </section>

  <section class="card" id="settings-fragment">
    <h2>{t('settings.fragment')}</h2>
    <Toggle bind:checked={s.fragment.enabled} label={t('settings.fragmentEnabled')} onchange={now} />
    {#if s.fragment.enabled}
      <div class="field"><span>{t('settings.fragmentPackets')}</span><input class="input" bind:value={s.fragment.packets} onchange={changed} /></div>
      <div class="field"><span>{t('settings.fragmentLength')}</span><input class="input" bind:value={s.fragment.length} onchange={changed} /></div>
      <div class="field"><span>{t('settings.fragmentInterval')}</span><input class="input" bind:value={s.fragment.interval} onchange={changed} /></div>
    {/if}
  </section>

  <section class="card" id="settings-analysis">
    <h2>{t('settings.analysis')}</h2>
    <Toggle bind:checked={s.analysis.enabled} label={t('settings.analysisEnabled')} onchange={now} />
    <div class="field">
      <span>{t('settings.analysisScope')}</span>
      <select class="select" bind:value={s.analysis.scope} onchange={now}>
        <option value="active">{t('settings.scope.active')}</option>
        <option value="group">{t('settings.scope.group')}</option>
        <option value="all">{t('settings.scope.all')}</option>
      </select>
    </div>
    <div class="field">
      <span>{t('settings.analysisMethod')}</span>
      <select class="select" bind:value={s.analysis.method} onchange={now}>
        <option value="tcp">{t('settings.method.tcp')}</option>
        <option value="icmp">{t('settings.method.icmp')}</option>
        <option value="real">{t('settings.method.real')}</option>
      </select>
    </div>
    <div class="field"><span>{t('settings.analysisInterval')}</span><input class="input num" type="number" min="1" bind:value={s.analysis.intervalSecs} onchange={changed} /></div>
    <div class="field"><span>{t('settings.analysisWindow')}</span><input class="input num" type="number" min="5" max="500" bind:value={s.analysis.window} onchange={changed} /></div>
  </section>

  <section class="card" id="settings-testing">
    <h2>{t('settings.testing')}</h2>
    <div class="field col"><span>{t('settings.testUrl')}</span><input class="input" bind:value={s.test.url} onchange={changed} /></div>
    <div class="field">
      <span>{t('settings.testMethod')}</span>
      <select class="select" bind:value={s.test.method} onchange={now}>
        <option value="real">{t('settings.method.real')}</option>
        <option value="tcp">{t('settings.method.tcp')}</option>
        <option value="icmp">{t('settings.method.icmp')}</option>
      </select>
    </div>
    <div class="field"><span>{t('settings.testTimeout')}</span><input class="input num" type="number" min="500" step="500" bind:value={s.test.timeoutMs} onchange={changed} /></div>
    <div class="field"><span>{t('settings.testConcurrency')}</span><input class="input num" type="number" min="1" max="128" bind:value={s.test.concurrency} onchange={changed} /></div>
  </section>

  <section class="card" id="settings-advanced">
    <h2>{t('settings.advanced')}</h2>
    <div class="field">
      <span>{t('settings.logLevel')}</span>
      <select class="select" bind:value={s.advanced.logLevel} onchange={now}>
        {#each ['debug', 'info', 'warning', 'error', 'none'] as level}<option value={level}>{level}</option>{/each}
      </select>
    </div>
    <Toggle bind:checked={s.advanced.accessLog} label={t('settings.accessLog')} onchange={now} />
    <Toggle bind:checked={s.advanced.allowInsecure} label={t('settings.allowInsecure')} onchange={now} />
    <Toggle bind:checked={s.general.checkUpdates} label={t('settings.checkUpdates')} onchange={now} />
    <Toggle bind:checked={s.advanced.updatePrerelease} label={t('settings.prerelease')} onchange={now} />
    <div class="field col"><span>{t('settings.subUserAgent')}</span><input class="input" bind:value={s.advanced.subscriptionUserAgent} onchange={changed} /></div>
    <div class="field"><span>{t('settings.apiPort')}</span><input class="input num" type="number" bind:value={s.advanced.apiPort} onchange={changed} /></div>
    <div class="field col">
      <span>{t('settings.configOverride')}</span>
      <textarea class="textarea" rows="5" bind:value={s.advanced.configOverride} onchange={checkOverride} placeholder={'{ }'}></textarea>
      {#if overrideError}<span class="error">{overrideError}</span>{:else}<span class="faint hint">{t('settings.configOverrideHint')}</span>{/if}
    </div>
    <h3 class="sub">{t('settings.backup')}</h3>
    <div class="row wrap">
      <button class="btn" onclick={exportBackup}><Icon name="download" size={15} />{t('settings.exportBackup')}</button>
      <button class="btn" onclick={() => importFiles()}><Icon name="upload" size={15} />{t('settings.importBackup')}</button>
      <button class="btn" onclick={() => api.openDataDir()}><Icon name="folder" size={15} />{t('settings.openDataDir')}</button>
    </div>
  </section>
</div>

{#if picker}
  <Modal title={t('settings.pickProcess')} width={460} onclose={() => (picker = null)}>
    <input class="input filter" placeholder={t('common.search')} bind:value={picker.filter} />
    <div class="process-list">
      {#each picker.list.filter((p) => p.toLowerCase().includes(picker!.filter.toLowerCase())) as name}
        <button class:chosen={s.tun.apps.includes(name)} onclick={() => addApp(name)}>
          {name}{#if s.tun.apps.includes(name)}<Icon name="check" size={15} />{/if}
        </button>
      {/each}
    </div>
  </Modal>
{/if}

<style>
  .settings {
    gap: 14px;
  }

  .jump {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    position: sticky;
    top: -22px;
    z-index: 5;
    padding: 6px 0;
    background: var(--bg);
  }

  .chip {
    height: 28px;
    padding: 0 11px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface);
    cursor: pointer;
    font-size: 13px;
  }

  .chip:hover {
    border-color: var(--accent);
    color: var(--accent);
  }

  section {
    padding: 16px 20px;
    display: flex;
    flex-direction: column;
    max-width: 860px;
    scroll-margin-top: 50px;
  }

  section h2 {
    margin-bottom: 6px;
  }

  h3.sub {
    font-size: 14px;
    margin: 14px 0 0;
  }

  .hint {
    font-size: 12.5px;
    margin: 2px 0 8px;
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 7px 0;
  }

  .field.col {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
  }

  .field .num {
    width: 120px;
  }

  .field > .select {
    min-width: 200px;
  }

  .grow {
    flex: 1;
  }

  .segmented {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: 9px;
    background: var(--surface-3);
    align-self: flex-start;
  }

  .segmented button {
    border: none;
    background: none;
    padding: 5px 12px;
    border-radius: 7px;
    cursor: pointer;
  }

  .segmented button.active {
    background: var(--surface);
    box-shadow: var(--shadow);
    color: var(--accent);
  }

  .note {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: 8px;
    margin: 6px 0;
  }

  .note.warn {
    background: var(--warning-soft);
    color: var(--warning);
  }

  .apps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin: 10px 0 8px;
  }

  .app-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 4px 3px 10px;
    border-radius: 7px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 13px;
  }

  .app-chip button {
    display: grid;
    place-items: center;
    border: none;
    background: none;
    color: inherit;
    cursor: pointer;
    padding: 2px;
  }

  .link {
    align-self: flex-start;
    margin-top: 4px;
  }

  .wrap {
    flex-wrap: wrap;
    margin-top: 8px;
  }

  .error {
    color: var(--danger);
    font-size: 12.5px;
  }

  .filter {
    width: 100%;
    margin-bottom: 8px;
  }

  .process-list {
    display: flex;
    flex-direction: column;
    max-height: 380px;
    overflow: auto;
  }

  .process-list button {
    display: flex;
    justify-content: space-between;
    padding: 7px 10px;
    border: none;
    background: none;
    text-align: left;
    border-radius: 6px;
    cursor: pointer;
  }

  .process-list button:hover {
    background: var(--surface-2);
  }

  .process-list button.chosen {
    color: var(--accent);
  }
</style>
