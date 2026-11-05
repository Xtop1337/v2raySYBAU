// Streamer mode: server addresses, exit IPs, subscription links and local paths are replaced by
// a mask wherever the interface shows them.

import { app } from './store.svelte';

export const MASK = '••••••';

export function streamer(): boolean {
  return app.settings?.general.streamerMode ?? false;
}

/** `text` (an address, IP, link or path), or the mask in streamer mode. */
export function hide(text: string): string {
  return streamer() && text ? MASK : text;
}
