<script lang="ts">
  import { getCurrentWebview } from '@tauri-apps/api/webview';
  import { onMount } from 'svelte';
  import ConflictsDialog from './components/ConflictsDialog.svelte';
  import Icon from './components/Icon.svelte';
  import Logo from './components/Logo.svelte';
  import UpdateDialog from './components/UpdateDialog.svelte';
  import { importFiles } from './lib/actions';
  import { checkConflicts } from './lib/conflicts.svelte';
  import { closeDialog, confirm, dialog } from './lib/dialogs.svelte';
  import { duration, speed } from './lib/format';
  import { t } from './lib/i18n.svelte';
  import { streamer } from './lib/privacy';
  import { app, dismissAllToasts, errorText, init, saveSettings, toasts, type Page } from './lib/store.svelte';
  import { announceUpdate, updates } from './lib/updates.svelte';
  import About from './pages/About.svelte';
  import Analytics from './pages/Analytics.svelte';
  import Journal from './pages/Journal.svelte';
  import Routing from './pages/Routing.svelte';
  import Servers from './pages/Servers.svelte';
  import Settings from './pages/Settings.svelte';
  import Subscriptions from './pages/Subscriptions.svelte';

  const nav: { id: Page; icon: string; key: string }[] = [
    { id: 'servers', icon: 'servers', key: 'nav.servers' },
    { id: 'analytics', icon: 'analytics', key: 'nav.analytics' },
    { id: 'journal', icon: 'journal', key: 'nav.journal' },
    { id: 'routing', icon: 'routing', key: 'nav.routing' },
    { id: 'subscriptions', icon: 'subscriptions', key: 'nav.subscriptions' },
    { id: 'settings', icon: 'settings', key: 'nav.settings' },
    { id: 'about', icon: 'about', key: 'nav.about' },
  ];

  // Streamer mode: the journal shows server addresses, so opening it asks first (once per run).
  let journalAllowed = false;

  async function goTo(page: Page) {
    if (page === 'journal' && streamer() && !journalAllowed) {
      if (!(await confirm(t('streamer.journalWarning'), t('streamer.openJournal'), t('common.cancel')))) return;
      journalAllowed = true;
    }
    app.page = page;
  }

  function toggleStreamer() {
    if (!app.settings) return;
    app.settings.general.streamerMode = !app.settings.general.streamerMode;
    saveSettings(true);
  }

  // Notifications shown before streamer mode was turned on may carry an IP: drop them (only
  // when it turns on, not on every later settings change).
  let streamerWas = false;
  $effect(() => {
    const on = app.settings?.general.streamerMode ?? false;
    if (on && !streamerWas) dismissAllToasts();
    streamerWas = on;
  });

  let fatal = $state('');
  let dragging = $state(false);
  let now = $state(Date.now());

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 1000);
    let unlisten: (() => void) | undefined;
    init()
      .then(async () => {
        unlisten = await getCurrentWebview().onDragDropEvent((event) => {
          if (event.payload.type === 'enter' || event.payload.type === 'over') dragging = true;
          else if (event.payload.type === 'drop') {
            dragging = false;
            importFiles(event.payload.paths);
          } else dragging = false;
        });
        // At launch: first the new version (if any), then the programs that may break the connection.
        if (app.settings?.general.checkUpdates) await announceUpdate();
        if (app.settings?.general.checkConflicts) await checkConflicts();
      })
      .catch((e) => (fatal = errorText(e)));
    return () => {
      clearInterval(timer);
      unlisten?.();
    };
  });
</script>

{#if fatal}
  <div class="fatal"><h2>V2Ray Sybau</h2><p>{fatal}</p></div>
{:else if app.ready}
  <div class="shell">
    <aside class="sidebar">
      <div class="brand">
        <Logo size={32} />
        <span>V2Ray Sybau</span>
      </div>
      <nav>
        {#each nav as item}
          <button class="nav-item" class:active={app.page === item.id} onclick={() => goTo(item.id)} data-testid="nav-{item.id}">
            <Icon name={item.icon} />
            <span>{t(item.key)}</span>
            {#if item.id === 'about' && updates.available}<span class="new-dot"></span>{/if}
          </button>
        {/each}
      </nav>
    </aside>

    <main>
      {#if app.page === 'servers'}<Servers />
      {:else if app.page === 'analytics'}<Analytics />
      {:else if app.page === 'journal'}<Journal />
      {:else if app.page === 'routing'}<Routing />
      {:else if app.page === 'subscriptions'}<Subscriptions />
      {:else if app.page === 'settings'}<Settings />
      {:else}<About />{/if}
    </main>

    <footer class="statusbar">
      <span class="dot" class:on={app.connection.connected}></span>
      <span>{app.connection.connected ? t('status.connected') : t('status.disconnected')}</span>
      {#if app.connection.connected}
        <span class="sep">·</span><span class="name">{app.connection.profileName}</span>
        <span class="sep">·</span><span>{app.connection.mode === 'tun' ? t('status.mode.tun') : t('status.mode.proxy')}{app.connection.systemProxy ? ` + ${t('status.systemProxy')}` : ''}</span>
        <span class="sep">·</span><span class="mono">{duration(app.connection.since, now)}</span>
      {/if}
      <span class="spacer"></span>
      {#if app.settings?.general.showSpeed && app.connection.connected}
        <span class="speed"><Icon name="up" size={13} />{speed(app.traffic.up)}</span>
        <span class="speed"><Icon name="down" size={13} />{speed(app.traffic.down)}</span>
      {/if}
      {#if app.settings}
        <span class="faint mono">SOCKS {app.settings.proxy.socksPort} · HTTP {app.settings.proxy.httpPort}</span>
        <button
          class="streamer"
          class:on={app.settings.general.streamerMode}
          onclick={toggleStreamer}
          title={t('settings.streamerMode')}
          data-testid="streamer-toggle"
        >
          <Icon name={app.settings.general.streamerMode ? 'eye-off' : 'eye'} size={14} />
          {#if app.settings.general.streamerMode}<span>{t('streamer.on')}</span>{/if}
        </button>
      {/if}
    </footer>
  </div>

  {#if dragging}
    <div class="drop"><Icon name="upload" size={34} /><span>{t('servers.dropHint')}</span></div>
  {/if}
{/if}

<UpdateDialog />
<ConflictsDialog />

<div class="toasts">
  {#each toasts.list as toast (toast.id)}
    <div class="toast {toast.kind}">{toast.text}</div>
  {/each}
</div>

{#if dialog.current}
  <div class="backdrop" role="presentation">
    <div class="card confirm" role="alertdialog">
      <p>{dialog.current.message}</p>
      <div class="row end">
        {#each dialog.current.choices as choice}
          <button class="btn" class:primary={choice.kind === 'primary'} class:danger-fill={choice.kind === 'danger'} onclick={() => closeDialog(choice.id)}>
            {choice.label}
          </button>
        {/each}
      </div>
    </div>
  </div>
{/if}

<style>
  .streamer {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px;
    border: 1px solid transparent;
    border-radius: 999px;
    background: none;
    color: var(--text-3);
    font: inherit;
    cursor: pointer;
  }

  .streamer:hover {
    color: var(--text);
  }

  .streamer.on {
    color: var(--accent);
    border-color: var(--accent);
  }

  .shell {
    display: grid;
    grid-template-columns: 220px 1fr;
    grid-template-rows: 1fr 30px;
    height: 100vh;
  }

  .sidebar {
    grid-row: 1 / 3;
    display: flex;
    flex-direction: column;
    padding: 16px 12px;
    background: var(--sidebar);
    border-right: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 2px 8px 18px;
    font-weight: 700;
    font-size: 15px;
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .nav-item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 11px;
    height: 38px;
    padding: 0 12px;
    border: none;
    border-radius: 9px;
    background: none;
    color: var(--text-2);
    cursor: pointer;
    text-align: left;
  }

  .nav-item:hover {
    background: var(--surface-3);
    color: var(--text);
  }

  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .new-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--danger);
    margin-left: auto;
  }

  main {
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  .statusbar {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 14px;
    font-size: 12px;
    color: var(--text-2);
    border-top: 1px solid var(--border);
    background: var(--surface-2);
    white-space: nowrap;
    overflow: hidden;
  }

  .statusbar .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-3);
  }

  .statusbar .dot.on {
    background: var(--success);
  }

  .statusbar .name {
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sep {
    color: var(--text-3);
  }

  .speed {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    font-variant-numeric: tabular-nums;
    min-width: 86px;
  }

  .drop {
    position: fixed;
    inset: 12px;
    z-index: 60;
    border: 2px dashed var(--accent);
    border-radius: 16px;
    background: color-mix(in srgb, var(--accent-soft) 85%, transparent);
    color: var(--accent);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    font-size: 16px;
    font-weight: 600;
    pointer-events: none;
  }

  .toasts {
    position: fixed;
    right: 18px;
    bottom: 42px;
    z-index: 70;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 420px;
  }

  .toast {
    padding: 10px 14px;
    border-radius: 10px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 4px solid var(--accent);
    box-shadow: var(--shadow-lg);
    animation: slide 0.18s ease-out;
    user-select: text;
    word-break: break-word;
  }

  .toast.success {
    border-left-color: var(--success);
  }

  .toast.error {
    border-left-color: var(--danger);
  }

  @keyframes slide {
    from {
      transform: translateY(8px);
      opacity: 0;
    }
  }

  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: grid;
    place-items: center;
    background: rgba(15, 23, 42, 0.35);
  }

  .confirm {
    width: 460px;
    max-width: calc(100vw - 32px);
    padding: 20px;
    box-shadow: var(--shadow-lg);
  }

  .confirm p {
    margin: 0 0 18px;
    white-space: pre-wrap;
  }

  .end {
    justify-content: flex-end;
    flex-wrap: wrap;
  }

  .danger-fill {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  .fatal {
    padding: 40px;
  }
</style>
