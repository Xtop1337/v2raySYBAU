import { currentLanguage } from './i18n.svelte';
import type { Profile } from './types';

export function bytes(value: number | null | undefined): string {
  if (value == null) return '—';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let v = value;
  let unit = 0;
  while (v >= 1024 && unit < units.length - 1) {
    v /= 1024;
    unit++;
  }
  return unit === 0 ? `${v} B` : `${v.toFixed(v >= 100 ? 0 : 1)} ${units[unit]}`;
}

export function speed(value: number): string {
  return `${bytes(value)}/s`;
}

/** Delay colour classes like v2rayNG: green < 300 ms, yellow < 800 ms, red above, grey timeout. */
export function delayClass(ms: number | null | undefined): string {
  if (ms == null) return 'delay-none';
  if (ms < 0) return 'delay-timeout';
  if (ms < 300) return 'delay-good';
  if (ms < 800) return 'delay-mid';
  return 'delay-bad';
}

export function dateTime(value: string | number | null | undefined): string {
  if (value == null) return '—';
  const date = typeof value === 'number' ? new Date(value * 1000) : new Date(value);
  return date.toLocaleString(currentLanguage() === 'ru' ? 'ru-RU' : 'en-US', { dateStyle: 'short', timeStyle: 'short' });
}

export function time(value: string): string {
  return new Date(value).toLocaleTimeString(currentLanguage() === 'ru' ? 'ru-RU' : 'en-US', { hour12: false });
}

export function duration(since: string | null, now: number): string {
  if (!since) return '';
  const seconds = Math.max(0, Math.floor((now - new Date(since).getTime()) / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = seconds % 60;
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`;
}

export const protocolLabel: Record<Profile['protocol'], string> = {
  vless: 'VLESS',
  vmess: 'VMess',
  trojan: 'Trojan',
  shadowsocks: 'SS',
  hysteria2: 'Hysteria2',
  tuic: 'TUIC',
};

export function transportLabel(p: Profile): string {
  if (p.protocol === 'hysteria2' || p.protocol === 'tuic') return 'QUIC';
  if (p.protocol === 'shadowsocks') return p.method;
  const security = p.tls.security === 'none' ? '' : ` · ${p.tls.security === 'reality' ? 'Reality' : 'TLS'}`;
  const vision = p.flow ? ' · Vision' : '';
  return `${p.transport.network.toUpperCase()}${security}${vision}`;
}

/** Country flag emoji at the start of a server name, used as the row avatar. */
export function flag(name: string): string | null {
  const match = name.match(/^\s*((?:[\u{1F1E6}-\u{1F1FF}]){2})/u);
  return match ? match[1] : null;
}

export function newId(): string {
  return crypto.randomUUID();
}

export function emptyProfile(): Profile {
  return {
    id: newId(),
    name: '',
    group: 'Default',
    subscriptionId: null,
    protocol: 'vless',
    address: '',
    port: 443,
    uuid: '',
    password: '',
    alterId: 0,
    method: 'none',
    flow: '',
    plugin: '',
    transport: { network: 'tcp', headerType: '', host: '', path: '', serviceName: '', mode: '', extra: '', seed: '' },
    tls: { security: 'tls', sni: '', alpn: [], fingerprint: 'chrome', allowInsecure: false, publicKey: '', shortId: '', spiderX: '', mldsa65Verify: '', pinnedSha256: '', verifyNames: '', echConfig: '' },
    quic: { obfs: '', obfsPassword: '', upMbps: 0, downMbps: 0, hopPorts: '', congestionControl: '', udpRelayMode: '' },
    dns: '',
    delay: { ms: null, method: null, testedAt: null },
    createdAt: new Date().toISOString(),
  };
}
