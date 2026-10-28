import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import { open, save } from '@tauri-apps/plugin-dialog';
import { api } from './api';
import { confirm } from './dialogs.svelte';
import { t } from './i18n.svelte';
import { app, errorText, toast } from './store.svelte';
import type { ImportSummary, Profile, TestMethod } from './types';

export async function connectTo(id: string) {
  app.connecting = true;
  try {
    await api.connect(id);
  } catch (e) {
    const text = errorText(e);
    if (text.includes('TUN_NEEDS_ADMIN')) {
      if (await confirm(t('servers.tunNeedsAdmin'), t('servers.restartAdmin'), t('common.cancel'))) {
        await api.restartAsAdmin().catch((err) => toast(errorText(err), 'error'));
      }
    } else {
      toast(text, 'error', 8000);
    }
  } finally {
    app.connecting = false;
  }
}

export async function toggleConnection(selected: string | null) {
  if (app.connection.connected && (!selected || selected === app.connection.profileId)) {
    await api.disconnect().catch((e) => toast(errorText(e), 'error'));
    return;
  }
  const id = selected ?? app.settings?.lastProfileId ?? app.profiles[0]?.id;
  if (id) await connectTo(id);
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
