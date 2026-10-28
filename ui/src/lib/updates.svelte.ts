// Application self-update through the Tauri updater (GitHub Releases `latest.json`).

import { relaunch } from '@tauri-apps/plugin-process';
import { check, type Update } from '@tauri-apps/plugin-updater';
import { t } from './i18n.svelte';
import { errorText, toast } from './store.svelte';

export const updates = $state({
  checking: false,
  available: null as null | { version: string; notes: string },
  progress: null as null | number,
  error: '',
});

let pending: Update | null = null;

export async function checkForUpdates(silent = false) {
  updates.checking = true;
  updates.error = '';
  try {
    pending = await check();
    updates.available = pending ? { version: pending.version, notes: pending.body ?? '' } : null;
    if (pending && silent) toast(t('about.available', { v: pending.version }), 'info', 8000);
    if (!pending && !silent) toast(t('about.upToDate'), 'success');
  } catch (e) {
    updates.error = errorText(e);
    if (!silent) toast(t('about.updateError', { e: updates.error }), 'error', 7000);
  } finally {
    updates.checking = false;
  }
}

export async function installUpdate() {
  if (!pending) return;
  let total = 0;
  let received = 0;
  updates.progress = 0;
  try {
    await pending.downloadAndInstall((event) => {
      if (event.event === 'Started') total = event.data.contentLength ?? 0;
      if (event.event === 'Progress') {
        received += event.data.chunkLength;
        updates.progress = total ? Math.round((received / total) * 100) : null;
      }
    });
    await relaunch();
  } catch (e) {
    updates.progress = null;
    toast(errorText(e), 'error', 7000);
  }
}
