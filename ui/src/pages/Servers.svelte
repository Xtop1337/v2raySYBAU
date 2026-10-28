<script lang="ts">
  import Icon from '../components/Icon.svelte';
  import ProfileEditor from '../components/ProfileEditor.svelte';
  import {
    connectTo,
    copyLinks,
    deleteProfiles,
    exportCoreConfig,
    exportProfilesJson,
    importClipboard,
    importFiles,
    importText,
    runTest,
    toggleConnection,
  } from '../lib/actions';
  import { api } from '../lib/api';
  import { delayClass, emptyProfile, flag, protocolLabel, speed, transportLabel } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { app, errorText, toast } from '../lib/store.svelte';
  import type { Profile, TestMethod } from '../lib/types';

  let keyInput = $state('');
  let search = $state('');
  let editing = $state<{ profile: Profile; isNew: boolean } | null>(null);
  let menu = $state<{ x: number; y: number; profile: Profile } | null>(null);
  let moreOpen = $state(false);
  let lastClicked = $state<string | null>(null);

  const groups = $derived([...new Set(app.profiles.map((p) => p.group))]);
  const visible = $derived(
    app.profiles.filter((p) => {
      if (app.selectedGroup !== 'all' && p.group !== app.selectedGroup) return false;
      if (!search.trim()) return true;
      const q = search.toLowerCase();
      return p.name.toLowerCase().includes(q) || p.address.toLowerCase().includes(q) || p.protocol.includes(q);
    }),
  );
  const selected = $derived(app.selection.filter((id) => visible.some((p) => p.id === id)));
  const focused = $derived(selected.length ? selected[selected.length - 1] : null);
  const willDisconnect = $derived(app.connection.connected && (!focused || focused === app.connection.profileId));

  $effect(() => {
    if (app.selectedGroup !== 'all' && !groups.includes(app.selectedGroup)) app.selectedGroup = 'all';
  });

  function targets(): string[] | null {
    if (selected.length) return selected;
    return app.selectedGroup === 'all' && !search ? null : visible.map((p) => p.id);
  }

  function test(method: TestMethod) {
    runTest(targets(), method);
  }

  function onRowClick(event: MouseEvent, profile: Profile) {
    if (event.shiftKey && lastClicked) {
      const a = visible.findIndex((p) => p.id === lastClicked);
      const b = visible.findIndex((p) => p.id === profile.id);
      const [from, to] = a < b ? [a, b] : [b, a];
      app.selection = visible.slice(from, to + 1).map((p) => p.id);
    } else if (event.ctrlKey || event.metaKey) {
      app.selection = app.selection.includes(profile.id) ? app.selection.filter((id) => id !== profile.id) : [...app.selection, profile.id];
      lastClicked = profile.id;
    } else {
      app.selection = [profile.id];
      lastClicked = profile.id;
    }
  }

  function onContextMenu(event: MouseEvent, profile: Profile) {
    event.preventDefault();
    if (!app.selection.includes(profile.id)) app.selection = [profile.id];
    menu = { x: Math.min(event.clientX, window.innerWidth - 240), y: Math.min(event.clientY, window.innerHeight - 300), profile };
  }

  async function addKey() {
    const text = keyInput;
    keyInput = '';
    await importText(text);
  }

  function onKeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    const typing = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT';
    if (editing || typing) return;
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'v') {
      event.preventDefault();
      importClipboard();
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
      event.preventDefault();
      app.selection = visible.map((p) => p.id);
    } else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'r') {
      event.preventDefault();
      test(app.settings?.test.method ?? 'real');
    } else if (event.key === 'Delete' && selected.length) {
      deleteProfiles(selected);
    } else if (event.key === 'Enter' && focused) {
      connectTo(focused);
    }
  }

  async function removeUnreachable() {
    moreOpen = false;
    const n = await api.removeUnreachable(app.selectedGroup === 'all' ? null : app.selectedGroup).catch((e) => (toast(errorText(e), 'error'), 0));
    toast(t('servers.removed', { n }));
  }

  async function removeDuplicates() {
    moreOpen = false;
    toast(t('servers.removed', { n: await api.removeDuplicates() }));
  }

  function delayText(p: Profile) {
    const ms = p.delay.ms;
    if (ms == null) return t('servers.untested');
    if (ms < 0) return t('servers.timeout');
    return `${ms} ${t('common.ms')}`;
  }
</script>

<svelte:window onkeydown={onKeydown} onclick={() => ((menu = null), (moreOpen = false))} />

<div class="page servers">
  <div class="hero card">
    <div class="key-row">
      <Icon name="link" />
      <input
        class="key-input"
        placeholder={t('servers.keyPlaceholder')}
        bind:value={keyInput}
        onkeydown={(e) => e.key === 'Enter' && addKey()}
        spellcheck="false"
      />
      <button class="btn primary" disabled={!keyInput.trim()} onclick={addKey}><Icon name="plus" size={16} />{t('servers.add')}</button>
      <button class="btn" onclick={importClipboard} title="Ctrl+V"><Icon name="clipboard" size={16} />{t('servers.fromClipboard')}</button>
      <button class="btn" onclick={() => importFiles()}><Icon name="file" size={16} />{t('servers.fromFile')}</button>
      <button class="btn icon" title={t('servers.newProfile')} onclick={() => (editing = { profile: emptyProfile(), isNew: true })}>
        <Icon name="edit" size={16} />
      </button>
    </div>
    <div class="connect-card">
      <div class="conn-state">
        <span class="dot" class:on={app.connection.connected} class:busy={app.connecting}></span>
        <div>
          <div class="conn-title">
            {app.connecting ? t('status.connecting') : app.connection.connected ? t('status.connected') : t('status.disconnected')}
          </div>
          <div class="faint conn-sub">
            {#if app.connection.connected}
              {app.connection.profileName} · {app.connection.mode === 'tun' ? t('status.mode.tun') : t('status.mode.proxy')}
              {#if app.settings?.general.showSpeed}· ↑ {speed(app.traffic.up)} ↓ {speed(app.traffic.down)}{/if}
            {:else}
              {app.profiles.find((p) => p.id === (focused ?? app.settings?.lastProfileId))?.name ?? ''}
            {/if}
          </div>
        </div>
      </div>
      <button
        class="btn connect-btn"
        class:primary={!willDisconnect}
        class:stop={willDisconnect}
        disabled={app.connecting || !app.profiles.length}
        onclick={() => toggleConnection(focused)}
      >
        <Icon name="power" size={17} />
        {willDisconnect ? t('servers.disconnect') : t('servers.connect')}
      </button>
    </div>
  </div>

  <div class="toolbar">
    <div class="chips">
      <button class="chip" class:active={app.selectedGroup === 'all'} onclick={() => (app.selectedGroup = 'all')}>
        {t('common.all')} <span class="count">{app.profiles.length}</span>
      </button>
      {#each groups as group}
        <button class="chip" class:active={app.selectedGroup === group} onclick={() => (app.selectedGroup = group)}>
          {group} <span class="count">{app.profiles.filter((p) => p.group === group).length}</span>
        </button>
      {/each}
    </div>
    <div class="spacer"></div>
    <div class="search">
      <Icon name="search" size={15} />
      <input placeholder={t('common.search')} bind:value={search} spellcheck="false" />
    </div>
  </div>

  <div class="toolbar">
    {#if app.test.running}
      <div class="progress">
        <div class="bar" style:width="{app.test.total ? (app.test.done / app.test.total) * 100 : 0}%"></div>
        <span>{t('servers.testing', { done: app.test.done, total: app.test.total })}</span>
      </div>
      <button class="btn small" onclick={() => api.cancelTest()}><Icon name="stop" size={14} />{t('servers.stopTest')}</button>
    {:else}
      <button class="btn small primary" onclick={() => test('real')} title="Ctrl+R"><Icon name="zap" size={14} />{t('servers.testReal')}</button>
      <button class="btn small" onclick={() => test('tcp')}>{t('servers.testTcp')}</button>
      <button class="btn small" onclick={() => test('icmp')}>{t('servers.testIcmp')}</button>
      <span class="faint hint">{selected.length ? t('servers.selected', { n: selected.length }) : ''}</span>
    {/if}
    <div class="spacer"></div>
    <button class="btn small" onclick={() => api.sortByDelay()}><Icon name="sort" size={14} />{t('servers.sortByDelay')}</button>
    <div class="more">
      <button class="btn small icon" onclick={(e) => (e.stopPropagation(), (moreOpen = !moreOpen))} aria-label="more"><Icon name="more" size={16} /></button>
      {#if moreOpen}
        <div class="menu card" role="menu">
          <button onclick={removeUnreachable}><Icon name="trash" size={15} />{t('servers.removeUnreachable')}</button>
          <button onclick={removeDuplicates}><Icon name="copy" size={15} />{t('servers.removeDuplicates')}</button>
          <button onclick={() => copyLinks(selected.length ? selected : visible.map((p) => p.id))}><Icon name="link" size={15} />{t('servers.menu.copyLink')}</button>
          <button onclick={() => exportProfilesJson(selected.length ? selected : visible.map((p) => p.id))}><Icon name="download" size={15} />{t('servers.menu.exportJson')}</button>
        </div>
      {/if}
    </div>
  </div>

  {#if app.profiles.length === 0}
    <div class="empty card">
      <Icon name="servers" size={34} stroke={1.4} />
      <h2>{t('servers.empty')}</h2>
      <p class="muted">{t('servers.emptyHint')}</p>
      <button class="btn primary" onclick={importClipboard}><Icon name="clipboard" size={16} />{t('servers.fromClipboard')}</button>
    </div>
  {:else}
    <div class="table card">
      <div class="thead">
        <span class="c-name">{t('servers.col.name')}</span>
        <span class="c-proto">{t('servers.col.protocol')}</span>
        <span class="c-addr">{t('servers.col.address')}</span>
        <span class="c-transport">{t('servers.col.transport')}</span>
        <span class="c-delay">{t('servers.col.delay')}</span>
        <span class="c-loss">{t('servers.col.loss')}</span>
      </div>
      <div class="tbody">
        {#each visible as p (p.id)}
          {@const active = app.connection.connected && app.connection.profileId === p.id}
          {@const stats = app.nodeStats[p.id]}
          <div
            class="tr"
            class:selected={app.selection.includes(p.id)}
            class:active
            role="row"
            tabindex="-1"
            onclick={(e) => onRowClick(e, p)}
            ondblclick={() => connectTo(p.id)}
            onkeydown={(e) => e.key === ' ' && onRowClick(e as unknown as MouseEvent, p)}
            oncontextmenu={(e) => onContextMenu(e, p)}
          >
            <span class="c-name">
              <span class="avatar">{flag(p.name) ?? protocolLabel[p.protocol].slice(0, 1)}</span>
              <span class="name-text">
                <span class="name">{p.name.replace(/^\s*(?:[\u{1F1E6}-\u{1F1FF}]){2}\s*/u, '') || p.address}</span>
                {#if app.selectedGroup === 'all' && groups.length > 1}<span class="faint group">{p.group}</span>{/if}
              </span>
              {#if active}<span class="badge accent">●</span>{/if}
            </span>
            <span class="c-proto"><span class="badge">{protocolLabel[p.protocol]}</span></span>
            <span class="c-addr faint mono">{p.address}:{p.port}</span>
            <span class="c-transport faint">{transportLabel(p)}</span>
            <span class="c-delay {delayClass(p.delay.ms)}">{delayText(p)}</span>
            <span class="c-loss faint">{stats ? `${stats.loss.toFixed(0)}%` : ''}</span>
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>

{#if menu}
  {@const p = menu.profile}
  <div class="menu card context" style:left="{menu.x}px" style:top="{menu.y}px" role="menu">
    <button onclick={() => connectTo(p.id)}><Icon name="power" size={15} />{t('servers.menu.connect')}</button>
    <button onclick={() => runTest(selected.length ? selected : [p.id], app.settings?.test.method ?? 'real')}><Icon name="zap" size={15} />{t('servers.menu.test')}</button>
    <button onclick={() => (editing = { profile: p, isNew: false })}><Icon name="edit" size={15} />{t('servers.menu.edit')}</button>
    <button onclick={() => copyLinks(selected.length ? selected : [p.id])}><Icon name="link" size={15} />{t('servers.menu.copyLink')}</button>
    <button onclick={() => exportProfilesJson(selected.length ? selected : [p.id])}><Icon name="download" size={15} />{t('servers.menu.exportJson')}</button>
    <button onclick={() => exportCoreConfig(p)}><Icon name="file" size={15} />{t('servers.menu.exportConfig')}</button>
    <hr />
    <button class="danger" onclick={() => deleteProfiles(selected.length ? selected : [p.id])}><Icon name="trash" size={15} />{t('servers.menu.delete')}</button>
  </div>
{/if}

{#if editing}
  <ProfileEditor profile={editing.profile} isNew={editing.isNew} onclose={() => (editing = null)} />
{/if}

<style>
  .servers {
    gap: 12px;
  }

  .hero {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px 16px;
    box-shadow: var(--shadow);
  }

  .key-row {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--text-3);
  }

  .key-input {
    flex: 1;
    min-width: 120px;
    height: 38px;
    border: 1px solid var(--border);
    border-radius: 9px;
    padding: 0 12px;
    background: var(--surface-2);
    color: var(--text);
    outline: none;
  }

  .key-input:focus {
    border-color: var(--accent);
    background: var(--surface);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .connect-card {
    display: flex;
    align-items: center;
    gap: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
  }

  .conn-state {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .conn-title {
    font-weight: 600;
  }

  .conn-sub {
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dot {
    width: 11px;
    height: 11px;
    border-radius: 50%;
    background: var(--text-3);
    flex: none;
  }

  .dot.on {
    background: var(--success);
    box-shadow: 0 0 0 4px var(--success-soft);
  }

  .dot.busy {
    background: var(--warning);
    animation: pulse 1s infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.4;
    }
  }

  .connect-btn {
    height: 40px;
    padding: 0 22px;
    font-weight: 600;
    border-radius: 10px;
  }

  .connect-btn.stop {
    background: var(--danger-soft);
    border-color: transparent;
    color: var(--danger);
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
  }

  .chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
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

  .chip.active {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }

  .count {
    color: var(--text-3);
    margin-left: 3px;
  }

  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--border);
    border-radius: 8px;
    color: var(--text-3);
    background: var(--surface);
  }

  .search input {
    border: none;
    outline: none;
    background: transparent;
    width: 170px;
    color: var(--text);
  }

  .hint {
    font-size: 12.5px;
  }

  .progress {
    position: relative;
    width: 260px;
    height: 28px;
    border-radius: 8px;
    background: var(--surface-3);
    overflow: hidden;
    display: flex;
    align-items: center;
    padding: 0 10px;
    font-size: 12.5px;
  }

  .progress .bar {
    position: absolute;
    inset: 0 auto 0 0;
    background: var(--accent-soft);
    border-right: 2px solid var(--accent);
    transition: width 0.2s;
  }

  .progress span {
    position: relative;
  }

  .more {
    position: relative;
  }

  .menu {
    position: absolute;
    right: 0;
    top: 34px;
    z-index: 20;
    min-width: 230px;
    padding: 5px;
    box-shadow: var(--shadow-lg);
    display: flex;
    flex-direction: column;
  }

  .menu.context {
    position: fixed;
    right: auto;
  }

  .menu button {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 10px;
    border: none;
    background: none;
    border-radius: 6px;
    text-align: left;
    cursor: pointer;
  }

  .menu button:hover {
    background: var(--surface-2);
  }

  .menu .danger {
    color: var(--danger);
  }

  .menu hr {
    border: none;
    border-top: 1px solid var(--border);
    margin: 4px 0;
  }

  .empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px;
    text-align: center;
    color: var(--text-3);
  }

  .empty h2 {
    color: var(--text);
  }

  .empty p {
    max-width: 420px;
    margin: 0 0 8px;
  }

  .table {
    flex: 1;
    min-height: 180px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .thead,
  .tr {
    display: grid;
    grid-template-columns: minmax(200px, 2.4fr) 92px minmax(140px, 1.6fr) minmax(110px, 1.2fr) 92px 60px;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
  }

  .thead {
    height: 36px;
    font-size: 12px;
    color: var(--text-3);
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
  }

  .tbody {
    overflow: auto;
    flex: 1;
  }

  .tr {
    height: 44px;
    border-bottom: 1px solid var(--border);
    cursor: default;
    outline: none;
  }

  .tr:hover {
    background: var(--surface-2);
  }

  .tr.selected {
    background: var(--accent-soft);
  }

  .tr.active {
    box-shadow: inset 3px 0 0 var(--success);
  }

  .c-name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .avatar {
    flex: none;
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    background: var(--surface-3);
    font-size: 15px;
    font-weight: 600;
    color: var(--text-2);
  }

  .name-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .name,
  .group,
  .c-addr,
  .c-transport {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .group {
    font-size: 11.5px;
  }

  .c-delay {
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .c-transport,
  .c-loss {
    font-size: 12.5px;
  }
</style>
