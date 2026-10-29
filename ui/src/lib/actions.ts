import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import { open, save } from '@tauri-apps/plugin-dialog';
import { api } from './api';
import { confirm } from './dialogs.svelte';
import { t } from './i18n.svelte';
import { app, errorText, hideConnecting, saveSettings, showConnecting, toast } from './store.svelte';
import type { ConnectionMode, ImportSummary, Profile, TestMethod, TunEngine } from './types';

// Connection requests are serialized: while one connect is running, further clicks only
// update `pendingTarget`, and the loop then connects to the most recently chosen server.
let pendingTarget: string | null = null;
let currentTarget: string | null = null;
let connectLoop = false;

function choose(id: string) {
  if (app.selectedId !== id) {
    app.selectedId = id;
    api.selectProfile(id).catch(() => {});
  }
}

/** Connects to `id` (switching from the current server if connected). */
export async function connectTo(id: string) {
  choose(id);
  pendingTarget = id;
  if (connectLoop) return;
  connectLoop = true;
  app.connecting = true;
  try {
    while (pendingTarget) {
      const target = pendingTarget;
      pendingTarget = null;
      currentTarget = target;
      showConnecting();
      try {
        await api.connect(target);
      } catch (e) {
        hideConnecting();
        const text = errorText(e);
        if (text.includes('TUN_NEEDS_ADMIN')) {
          pendingTarget = null;
          if (await confirm(t('servers.tunNeedsAdmin'), t('servers.restartAdmin'), t('common.cancel'))) {
            await api.restartAsAdmin().catch((err) => toast(errorText(err), 'error'));
          }
        } else {
          toast(text, 'error', 8000);
        }
      }
    }
  } finally {
    connectLoop = false;
    currentTarget = null;
    app.connecting = false;
  }
}

/** Picks a server in the list; while connected to another one, switches to it right away. */
export async function selectServer(id: string) {
  choose(id);
  if (connectLoop) {
    // A connect is in progress: finish on the server picked last.
    pendingTarget = id === currentTarget ? null : id;
  } else if (app.connection.connected && app.connection.profileId !== id) {
    await connectTo(id);
  }
}

/** The single connect button: disconnects when connected, otherwise connects the chosen server. */
export async function toggleConnection() {
  if (app.connection.connected) {
    pendingTarget = null;
    await api.disconnect().catch((e) => toast(errorText(e), 'error'));
    return;
  }
  const id = app.selectedId ?? app.profiles[0]?.id;
  if (id) await connectTo(id);
  else toast(t('servers.noServerSelected'));
}

/** Switches Proxy/TUN mode (and TUN engine); the backend reconnects if connected. */
export async function setConnectionMode(mode: ConnectionMode, engine?: TunEngine) {
  const settings = app.settings;
  if (!settings) return;
  const needsAdmin = mode === 'tun' && app.platform === 'windows' && !app.elevated;
  settings.proxy.mode = mode;
  if (engine) settings.tun.engine = engine;
  await saveSettings(true);
  if (needsAdmin && (await confirm(t('servers.tunNeedsAdmin'), t('servers.restartAdmin'), t('common.cancel')))) {
    await api.restartAsAdmin().catch((err) => toast(errorText(err), 'error'));
  }
}

function reportImport(summary: ImportSummary) {
  if (summary.restoredBackup) toast(t('servers.backupRestored'), 'success');
  if (summary.added === 0 && summary.subscriptions === 0 && !summary.restoredBackup) {
    toast(summary.errors[0] ?? t('servers.importNothing'), summary.errors.length ? 'error' : 'info', 6000);
    return;
  }
  let text = summary.subscriptions
    ? t('servers.importedSubs', { added: summary.added, subs: summary.subscriptions })
    : t('servers.imported', { added: summary.added });
  if (summary.duplicates) text += `, ${t('servers.importDuplicates', { n: summary.duplicates })}`;
  toast(text, 'success');
  for (const error of summary.errors.slice(0, 3)) toast(error, 'error', 6000);
}

export async function importText(text: string) {
  if (!text.trim()) {
    toast(t('servers.importNothing'));
    return;
  }
  try {
    reportImport(await api.importText(text));
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

export async function importClipboard() {
  const text = await readText().catch(() => '');
  await importText(text ?? '');
}

export async function importFiles(paths?: string[]) {
  let files = paths;
  if (!files) {
    const picked = await open({ multiple: true, filters: [{ name: 'Config', extensions: ['json', 'txt', 'conf', 'yaml'] }] });
    if (!picked) return;
    files = Array.isArray(picked) ? picked : [picked];
  }
  for (const path of files) {
    try {
      reportImport(await api.importFile(path));
    } catch (e) {
      toast(errorText(e), 'error');
    }
  }
}

export async function deleteProfiles(ids: string[]) {
  if (!ids.length) return;
  if (app.settings?.general.confirmDelete) {
    const one = ids.length === 1 ? app.profiles.find((p) => p.id === ids[0]) : undefined;
    const message = one ? t('servers.confirmDeleteOne', { name: one.name || one.address }) : t('servers.confirmDelete', { n: ids.length });
    if (!(await confirm(message, t('common.delete'), t('common.cancel'), true))) return;
  }
  const removed = await api.deleteProfiles(ids).catch((e) => {
    toast(errorText(e), 'error');
    return 0;
  });
  app.selection = app.selection.filter((id) => !ids.includes(id));
  if (removed) toast(t('servers.removed', { n: removed }));
}

export async function runTest(ids: string[] | null, method: TestMethod) {
  try {
    await api.startTest(ids, method);
  } catch (e) {
    toast(errorText(e), 'error');
  }
}

export async function copyLinks(ids: string[]) {
  const links = await api.shareLinks(ids);
  await writeText(links);
  toast(t('common.copied'), 'success');
}

export async function exportProfilesJson(ids: string[]) {
  const path = await save({ defaultPath: 'v2raysybau-servers.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
  if (!path) return;
  await api.exportProfiles(ids, path).then(
    () => toast(t('common.saved'), 'success'),
    (e) => toast(errorText(e), 'error'),
  );
}

export async function exportCoreConfig(profile: Profile) {
  const name = (profile.name || profile.address).replace(/[\\/:*?"<>|]+/g, '_');
  const path = await save({ defaultPath: `${name}.json`, filters: [{ name: 'JSON', extensions: ['json'] }] });
  if (!path) return;
  await api.exportCoreConfig(profile.id, path).then(
    () => toast(t('common.saved'), 'success'),
    (e) => toast(errorText(e), 'error'),
  );
}
