<script lang="ts">
  import { openUrl } from '@tauri-apps/plugin-opener';
  import Icon from '../components/Icon.svelte';
  import Modal from '../components/Modal.svelte';
  import Toggle from '../components/Toggle.svelte';
  import { api } from '../lib/api';
  import { ask } from '../lib/dialogs.svelte';
  import { bytes, dateTime, newId } from '../lib/format';
  import { t } from '../lib/i18n.svelte';
  import { hide } from '../lib/privacy';
  import { app, errorText, toast } from '../lib/store.svelte';
  import type { Subscription } from '../lib/types';

  let editing = $state<Subscription | null>(null);
  let busy = $state<Record<string, boolean>>({});
  let updatingAll = $state(false);

  const groups = $derived(
    [...new Set(app.profiles.map((p) => p.group))].map((name) => ({ name, count: app.profiles.filter((p) => p.group === name).length })),
  );

  function blank(): Subscription {
    return {
      id: newId(),
      name: '',
      url: '',
      enabled: true,
      autoUpdateHours: 12,
      userAgent: '',
      updatedAt: null,
      lastError: null,
      upload: null,
      download: null,
      total: null,
      expire: null,
      webPageUrl: null,
    };
  }

  async function update(sub: Subscription) {
    busy[sub.id] = true;
    try {
      const n = await api.updateSubscription(sub.id);
      toast(t('subs.updatedOk', { n }), 'success');
    } catch (e) {
      toast(errorText(e), 'error');
    } finally {
      busy[sub.id] = false;
    }
  }

  async function updateAll() {
    updatingAll = true;
    const [ok, total] = await api.updateAllSubscriptions();
    updatingAll = false;
    toast(t('subs.updatedAll', { ok, total }), ok === total ? 'success' : 'error');
  }

  async function save() {
    if (!editing) return;
    const sub = $state.snapshot(editing) as Subscription;
    sub.autoUpdateHours = Number(sub.autoUpdateHours) || 0;
    const isNew = !app.subscriptions.some((s) => s.id === sub.id);
    editing = null;
    try {
      await api.saveSubscription(sub);
      if (isNew && sub.url) await update(sub);
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }

  async function remove(sub: Subscription) {
    const choice = await ask(t('subs.deleteConfirm', { name: sub.name || hide(sub.url) }), [
      { id: 'cancel', label: t('common.cancel') },
      { id: 'keep', label: t('subs.deleteKeepServers') },
      { id: 'all', label: t('subs.deleteWithServers'), kind: 'danger' },
    ]);
    if (!choice || choice === 'cancel') return;
    await api.deleteSubscription(sub.id, choice === 'all').catch((e) => toast(errorText(e), 'error'));
  }

  // The provider's page: from the `profile-web-page-url` header, or the http(s) link itself
  // (panels such as Marzban and Remnawave show the subscription page at that address).
  function siteOf(sub: Subscription): string | null {
    if (sub.webPageUrl) return sub.webPageUrl;
    return /^https?:\/\//i.test(sub.url.trim()) ? sub.url.trim() : null;
  }

  function openSite(sub: Subscription) {
    const site = siteOf(sub);
    if (site) openUrl(site).catch((e) => toast(errorText(e), 'error'));
  }

  function serverCount(sub: Subscription) {
    return app.profiles.filter((p) => p.subscriptionId === sub.id).length;
  }
</script>

<div class="page">
  <div class="page-header">
    <h1>{t('subs.title')}</h1>
    <div class="spacer"></div>
    <button class="btn" disabled={updatingAll || !app.subscriptions.length} onclick={updateAll}>
      <Icon name="refresh" size={15} />{t('subs.updateAll')}
    </button>
    <button class="btn primary" onclick={() => (editing = blank())}><Icon name="plus" size={15} />{t('subs.add')}</button>
  </div>

  {#if app.subscriptions.length === 0}
    <div class="card empty muted">{t('subs.empty')}</div>
  {/if}

  <div class="list">
    {#each app.subscriptions as sub (sub.id)}
      <div class="card sub" class:off={!sub.enabled}>
        <div class="icon"><Icon name="subscriptions" size={20} /></div>
        <div class="info">
          {#if siteOf(sub)}
            <button class="title link" onclick={() => openSite(sub)} title={t('subs.openSite')} data-testid="sub-site">
              {sub.name || hide(sub.url)}<Icon name="link" size={13} />
            </button>
          {:else}
            <div class="title">{sub.name || hide(sub.url)}</div>
          {/if}
          <div class="faint mono url" data-testid="sub-url">{hide(sub.url)}</div>
          <div class="meta muted">
            <span>{t('subs.servers', { n: serverCount(sub) })}</span>
            <span>{t('subs.updated', { when: sub.updatedAt ? dateTime(sub.updatedAt) : t('common.never') })}</span>
            {#if sub.total}
              <span>{t('subs.traffic', { used: bytes((sub.upload ?? 0) + (sub.download ?? 0)), total: bytes(sub.total) })}</span>
            {/if}
            {#if sub.expire}<span>{t('subs.expires', { date: dateTime(sub.expire) })}</span>{/if}
          </div>
          {#if sub.total}
            <div class="usage"><div style:width="{Math.min(100, (((sub.upload ?? 0) + (sub.download ?? 0)) / sub.total) * 100)}%"></div></div>
          {/if}
          {#if sub.lastError}<div class="error">{sub.lastError}</div>{/if}
        </div>
        <div class="actions">
          {#if siteOf(sub)}
            <button class="btn small icon" onclick={() => openSite(sub)} aria-label={t('subs.openSite')} title={t('subs.openSite')}><Icon name="globe" size={14} /></button>
          {/if}
          <button class="btn small" disabled={busy[sub.id]} onclick={() => update(sub)}><Icon name="refresh" size={14} />{t('subs.update')}</button>
          <button class="btn small icon" onclick={() => (editing = structuredClone($state.snapshot(sub)) as Subscription)} aria-label="edit"><Icon name="edit" size={14} /></button>
          <button class="btn small icon danger" onclick={() => remove(sub)} aria-label="delete"><Icon name="trash" size={14} /></button>
        </div>
      </div>
    {/each}
  </div>

  {#if groups.length}
    <div class="section-title">{t('subs.groups')}</div>
    <div class="groups">
      {#each groups as g}
        <button class="card group" onclick={() => ((app.selectedGroup = g.name), (app.page = 'servers'))}>
          <span>{g.name}</span><span class="faint">{t('subs.groupServers', { n: g.count })}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

{#if editing}
  <Modal title={app.subscriptions.some((s) => s.id === editing?.id) ? t('common.edit') : t('subs.add')} onclose={() => (editing = null)}>
    <div class="form">
      <label><span>{t('common.name')}</span><input class="input" bind:value={editing.name} /></label>
      <label><span>{t('subs.url')}</span><input class="input mono" bind:value={editing.url} placeholder="https://" /></label>
      <label><span>{t('subs.interval')}</span><input class="input" type="number" min="0" bind:value={editing.autoUpdateHours} /></label>
      <label><span>{t('subs.userAgent')}</span><input class="input" bind:value={editing.userAgent} placeholder={app.settings?.advanced.subscriptionUserAgent} /></label>
      <Toggle bind:checked={editing.enabled} label={t('common.enabled')} />
    </div>
    {#snippet footer()}
      <button class="btn" onclick={() => (editing = null)}>{t('common.cancel')}</button>
      <button class="btn primary" disabled={!editing?.url.trim()} onclick={save}>{t('common.save')}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .empty {
    padding: 24px;
    text-align: center;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .sub {
    display: flex;
    gap: 14px;
    padding: 14px 16px;
    align-items: flex-start;
  }

  .sub.off {
    opacity: 0.6;
  }

  .icon {
    width: 38px;
    height: 38px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: var(--accent-soft);
    color: var(--accent);
    flex: none;
  }

  .info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .title {
    font-weight: 600;
  }

  .title.link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    padding: 0;
    border: none;
    background: none;
    font: inherit;
    font-weight: 600;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }

  .title.link:hover {
    color: var(--accent);
    text-decoration: underline;
  }

  .url {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 16px;
    font-size: 12.5px;
  }

  .usage {
    height: 5px;
    border-radius: 3px;
    background: var(--surface-3);
    overflow: hidden;
    margin-top: 4px;
    max-width: 360px;
  }

  .usage div {
    height: 100%;
    background: var(--accent);
  }

  .error {
    color: var(--danger);
    font-size: 12.5px;
  }

  .actions {
    display: flex;
    gap: 6px;
  }

  .groups {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .group {
    display: flex;
    gap: 10px;
    padding: 9px 14px;
    cursor: pointer;
  }

  .group:hover {
    border-color: var(--accent);
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12.5px;
    color: var(--text-2);
  }
</style>
