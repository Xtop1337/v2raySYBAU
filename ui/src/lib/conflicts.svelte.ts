// Programs that may break the connection (other VPNs, DPI bypass tools, other proxy clients):
// found at launch, the user chooses which to stop, or declines.

import { api } from './api';
import { t } from './i18n.svelte';
import { errorText, toast } from './store.svelte';
import type { Conflict } from './types';

export const conflicts = $state({
  found: [] as Conflict[],
  dialog: false,
  stopping: false,
});

let closed: (() => void) | null = null;

/** Looks for conflicting programs; when there are some, shows the window and resolves when it closes. */
export async function checkConflicts(silent = true): Promise<void> {
  try {
    conflicts.found = await api.detectConflicts();
  } catch (e) {
    if (!silent) toast(errorText(e), 'error');
    return;
  }
  if (!conflicts.found.length) {
    if (!silent) toast(t('conflicts.none'), 'success');
    return;
  }
  conflicts.dialog = true;
  await new Promise<void>((resolve) => (closed = resolve));
}

export function closeConflicts() {
  conflicts.dialog = false;
  closed?.();
  closed = null;
}

export async function stopConflicts(ids: string[]) {
  conflicts.stopping = true;
  try {
    for (const result of await api.stopConflicts(ids)) {
      if (result.ok) toast(t('conflicts.stopped', { name: result.name }), 'success');
      else if (result.error?.startsWith('NEEDS_ADMIN')) toast(t('conflicts.needsAdmin', { name: result.name }), 'error', 10000);
      else toast(t('conflicts.failed', { name: result.name, e: result.error ?? '' }), 'error', 10000);
    }
  } catch (e) {
    toast(errorText(e), 'error');
  } finally {
    conflicts.stopping = false;
    closeConflicts();
  }
}
