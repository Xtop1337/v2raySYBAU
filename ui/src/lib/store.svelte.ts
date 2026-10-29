// Global reactive state (Svelte 5 runes) fed by the backend snapshot and events.

import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { api } from './api';
import { flagOfCode } from './format';
import { resolveLanguage, setLanguage, t } from './i18n.svelte';
import type {
  ConnectionCheck,
  ConnectionInfo,
  DelayResult,
  LogEntry,
  NodeStats,
  Profile,
  Settings,
  Subscription,
  TestProgress,
  TrafficSample,
} from './types';

export type Page = 'servers' | 'analytics' | 'journal' | 'routing' | 'subscriptions' | 'settings' | 'about';

const MAX_LOGS = 5000;
const SPEED_POINTS = 90;

export const app = $state({
  ready: false,
  page: 'servers' as Page,
  profiles: [] as Profile[],
  subscriptions: [] as Subscription[],
  settings: null as Settings | null,
  connection: { connected: false, profileId: null, profileName: null, mode: null, since: null, systemProxy: false, core: null } as ConnectionInfo,
  appVersion: '',
  elevated: false,
  dataDir: '',
  platform: '',
  connecting: false,
  test: { done: 0, total: 0, running: false } as TestProgress,
  traffic: { up: 0, down: 0, directUp: 0, directDown: 0, totalUp: 0, totalDown: 0 } as TrafficSample,
  speedHistory: [] as { up: number; down: number }[],
  nodeStats: {} as Record<string, NodeStats>,
  logs: [] as LogEntry[],
  logsPaused: false,
  selectedGroup: 'all',
  selection: [] as string[],
  /** The server the connect button acts on (v2rayNG "selected server"). */
  selectedId: null as string | null,
  /** Traffic check of the current connection (exit address), null until it finishes. */
  check: null as ConnectionCheck | null,
});

export interface Toast {
  id: number;
  kind: 'info' | 'success' | 'error';
  text: string;
}

export const toasts = $state({ list: [] as Toast[] });
let toastId = 0;

export function toast(text: string, kind: Toast['kind'] = 'info', timeout = 4000): number {
  const id = ++toastId;
  toasts.list.push({ id, kind, text });
  setTimeout(() => dismissToast(id), timeout);
  return id;
}

export function dismissToast(id: number) {
  toasts.list = toasts.list.filter((t) => t.id !== id);
}

// "Creating the tunnel…" stays on screen until the traffic check of that connection reports.
let tunnelToast: number | null = null;

export function showConnecting() {
  if (tunnelToast !== null) dismissToast(tunnelToast);
  tunnelToast = toast(t('connect.creating'), 'info', 30000);
}

export function hideConnecting() {
  if (tunnelToast !== null) dismissToast(tunnelToast);
  tunnelToast = null;
}

function reportCheck(check: ConnectionCheck) {
  hideConnecting();
  const exit = check.exit ? `${flagOfCode(check.exit.country)} ${check.exit.ip}`.trim() : '';
  switch (check.problem) {
    case null: {
      // 2ip.ru and other Russian sites show the user's own IP with this preset, on purpose.
      const ru = app.settings?.routing.preset === 'bypassLanRu' ? ` ${t('connect.ruDirect')}` : '';
      toast(t('connect.created', { ip: exit }) + ru, 'success', ru ? 9000 : 6000);
      break;
    }
    case 'no-traffic':
      toast(t('connect.noTraffic', { name: check.profileName }), 'error', 10000);
      break;
    case 'direct':
      toast(t('connect.direct', { ip: exit }), 'error', 12000);
      break;
    case 'system-proxy':
      toast(t('connect.systemProxy', { detail: check.detail ?? '' }), 'error', 12000);
      break;
  }
}

export function errorText(e: unknown): string {
  return typeof e === 'string' ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

// ---- theme ---------------------------------------------------------------------------------

const darkQuery = window.matchMedia('(prefers-color-scheme: dark)');

export function applyTheme() {
  const theme = app.settings?.general.theme ?? 'system';
  const dark = theme === 'dark' || (theme === 'system' && darkQuery.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
  getCurrentWindow()
    .setTheme(theme === 'system' ? null : theme)
    .catch(() => {});
}

darkQuery.addEventListener('change', applyTheme);

export function applyLanguage() {
  const lang = resolveLanguage(app.settings?.general.language ?? 'auto');
  setLanguage(lang);
  api.setUiLanguage(lang).catch(() => {});
}

// ---- settings --------------------------------------------------------------------------------

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Saves settings (debounced so typing in inputs does not reconnect on every key). */
export function saveSettings(immediate = false): Promise<void> {
  if (!app.settings) return Promise.resolve();
  clearTimeout(saveTimer);
  const run = async () => {
    try {
      const saved = await api.saveSettings($state.snapshot(app.settings) as Settings);
      app.settings = saved;
      applyTheme();
      applyLanguage();
    } catch (e) {
      toast(errorText(e), 'error');
    }
  };
  if (immediate) return run();
  saveTimer = setTimeout(run, 600);
  return Promise.resolve();
}

// ---- bootstrap -------------------------------------------------------------------------------

export async function init() {
  const snapshot = await api.state();
  app.profiles = snapshot.profiles;
  app.subscriptions = snapshot.subscriptions;
  app.settings = snapshot.settings;
  app.connection = snapshot.connection;
  app.appVersion = snapshot.appVersion;
  app.elevated = snapshot.elevated;
  app.dataDir = snapshot.dataDir;
  app.platform = snapshot.platform;
  const known = (id: string | null | undefined) => (id && snapshot.profiles.some((p) => p.id === id) ? id : null);
  app.selectedId = known(snapshot.connection.profileId) ?? known(snapshot.settings.lastProfileId) ?? snapshot.profiles[0]?.id ?? null;
  if (app.selectedId) app.selection = [app.selectedId];
  app.logs = await api.logs(2000);
  for (const stats of await api.nodeStats()) app.nodeStats[stats.profileId] = stats;
  applyTheme();
  applyLanguage();

  await listen<Profile[]>('profiles', (e) => {
    app.profiles = e.payload;
    if (!app.profiles.some((p) => p.id === app.selectedId)) app.selectedId = app.connection.profileId ?? app.profiles[0]?.id ?? null;
  });
  await listen<Subscription[]>('subscriptions', (e) => (app.subscriptions = e.payload));
  await listen<Settings>('settings', (e) => {
    app.settings = e.payload;
    applyTheme();
  });
  await listen<ConnectionInfo>('connection', (e) => {
    if (e.payload.since !== app.connection.since || !e.payload.connected) app.check = null;
    app.connection = e.payload;
    if (e.payload.profileId && !app.selectedId) app.selectedId = e.payload.profileId;
    if (!e.payload.connected) {
      app.traffic = { up: 0, down: 0, directUp: 0, directDown: 0, totalUp: 0, totalDown: 0 };
    }
  });
  await listen<DelayResult>('delay-result', (e) => {
    const profile = app.profiles.find((p) => p.id === e.payload.profileId);
    if (profile) profile.delay = { ms: e.payload.ms, method: e.payload.method, testedAt: new Date().toISOString() };
  });
  await listen<TestProgress>('test-progress', (e) => (app.test = e.payload));
  await listen<TrafficSample>('traffic', (e) => {
    app.traffic = e.payload;
    app.speedHistory.push({ up: e.payload.up, down: e.payload.down });
    if (app.speedHistory.length > SPEED_POINTS) app.speedHistory.splice(0, app.speedHistory.length - SPEED_POINTS);
  });
  await listen<NodeStats[]>('node-stats', (e) => {
    for (const stats of e.payload) app.nodeStats[stats.profileId] = stats;
  });
  await listen<LogEntry[]>('logs', (e) => {
    if (app.logsPaused) return;
    app.logs.push(...e.payload);
    if (app.logs.length > MAX_LOGS) app.logs.splice(0, app.logs.length - MAX_LOGS);
  });
  await listen<string>('core-error', (e) => toast(e.payload, 'error', 7000));
  await listen<ConnectionCheck>('connection-check', (e) => {
    app.check = e.payload;
    reportCheck(e.payload);
  });
  app.ready = true;
}
