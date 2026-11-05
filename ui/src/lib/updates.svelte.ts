// New versions of the app: the release list on GitHub is checked (at launch and on demand) and
// the changes are shown. Nothing is downloaded or installed automatically.

import { openUrl } from '@tauri-apps/plugin-opener';
import { api } from './api';
import { t } from './i18n.svelte';
import { app, errorText, saveSettings, toast } from './store.svelte';
import type { AppUpdate } from './types';

export const updates = $state({
  checking: false,
  available: null as AppUpdate | null,
  /** The "new version" window is open. */
  dialog: false,
  error: '',
});

let closed: (() => void) | null = null;

export async function checkForUpdates(silent = false): Promise<AppUpdate | null> {
  updates.checking = true;
  updates.error = '';
  try {
    updates.available = await api.checkAppUpdate();
    if (!updates.available && !silent) toast(t('about.upToDate'), 'success');
  } catch (e) {
    updates.error = errorText(e);
    if (!silent) toast(t('about.updateError', { e: updates.error }), 'error', 7000);
  } finally {
    updates.checking = false;
  }
  return updates.available;
}

/** Launch check: shows the window for a new, not skipped version; resolves when it is closed. */
export async function announceUpdate(): Promise<void> {
  const update = await checkForUpdates(true);
  if (!update || update.version === app.settings?.general.skippedVersion) return;
  await showUpdate();
}

export function showUpdate(): Promise<void> {
  updates.dialog = true;
  return new Promise((resolve) => (closed = resolve));
}

export function closeUpdate() {
  updates.dialog = false;
  closed?.();
  closed = null;
}

export async function downloadUpdate() {
  const update = updates.available;
  if (update) await openUrl(update.installerUrl ?? update.pageUrl).catch((e) => toast(errorText(e), 'error'));
  closeUpdate();
}

export async function skipUpdate() {
  if (updates.available && app.settings) {
    app.settings.general.skippedVersion = updates.available.version;
    await saveSettings(true);
  }
  closeUpdate();
}

/** Links inside rendered release notes open in the browser, not in the app window. */
export function openNoteLink(event: MouseEvent) {
  const link = (event.target as HTMLElement).closest('a');
  if (!link) return;
  event.preventDefault();
  const href = link.getAttribute('href') ?? '';
  if (/^https?:\/\//.test(href)) openUrl(href).catch(() => {});
}
