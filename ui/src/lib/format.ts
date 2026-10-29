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
// Country names (English / Russian stems) for servers named "Germany 1", "Нидерланды" and so on.
const COUNTRIES: Record<string, string[]> = {
  AE: ['emirates', 'uae', 'dubai', 'эмират', 'оаэ', 'дубай'],
  AM: ['armenia', 'армени'],
  AR: ['argentina', 'аргентин'],
  AT: ['austria', 'vienna', 'австри', 'вена'],
  AU: ['australia', 'австрали'],
  AZ: ['azerbaijan', 'азербайджан'],
  BE: ['belgium', 'бельги'],
  BG: ['bulgaria', 'болгари'],
  BR: ['brazil', 'бразили'],
  BY: ['belarus', 'беларус', 'белорус'],
  CA: ['canada', 'канад'],
  CH: ['switzerland', 'zurich', 'швейцари', 'цюрих'],
  CN: ['china', 'китай'],
  CY: ['cyprus', 'кипр'],
  CZ: ['czech', 'prague', 'чехи', 'прага'],
  DE: ['germany', 'frankfurt', 'berlin', 'герман', 'франкфурт', 'берлин'],
  DK: ['denmark', 'дани'],
  EE: ['estonia', 'tallinn', 'эстони', 'таллин'],
  ES: ['spain', 'madrid', 'испани', 'мадрид'],
  FI: ['finland', 'helsinki', 'финлянд', 'хельсинки'],
  FR: ['france', 'paris', 'франци', 'париж'],
  GB: ['united kingdom', 'britain', 'england', 'london', 'великобритан', 'англи', 'лондон'],
  GE: ['georgia', 'tbilisi', 'грузи', 'тбилиси'],
  GR: ['greece', 'греци'],
  HK: ['hong kong', 'hongkong', 'гонконг'],
  HU: ['hungary', 'венгри'],
  IE: ['ireland', 'ирланди'],
  IL: ['israel', 'израил'],
  IN: ['india', 'инди'],
  IS: ['iceland', 'исланди'],
  IT: ['italy', 'milan', 'итали', 'милан'],
  JP: ['japan', 'tokyo', 'япони', 'токио'],
  KG: ['kyrgyzstan', 'киргиз', 'кыргыз'],
  KR: ['korea', 'seoul', 'коре', 'сеул'],
  KZ: ['kazakhstan', 'казахстан'],
  LT: ['lithuania', 'литв'],
  LV: ['latvia', 'riga', 'латви', 'рига'],
  MD: ['moldova', 'молдав', 'молдов'],
  MX: ['mexico', 'мексик'],
  NL: ['netherlands', 'holland', 'amsterdam', 'нидерланд', 'голланд', 'амстердам'],
  NO: ['norway', 'норвеги'],
  PL: ['poland', 'warsaw', 'польш', 'варшав'],
  PT: ['portugal', 'португали'],
  RO: ['romania', 'румыни'],
  RS: ['serbia', 'серби'],
  RU: ['russia', 'moscow', 'росси', 'москв', 'рф'],
  SE: ['sweden', 'stockholm', 'швеци', 'стокгольм'],
  SG: ['singapore', 'сингапур'],
  TR: ['turkey', 'turkiye', 'istanbul', 'турци', 'стамбул'],
  TW: ['taiwan', 'тайван'],
  UA: ['ukraine', 'kyiv', 'украин', 'киев'],
  US: ['united states', 'usa', 'america', 'new york', 'los angeles', 'сша', 'америк', 'нью-йорк'],
  UZ: ['uzbekistan', 'узбекистан'],
  VN: ['vietnam', 'вьетнам'],
  ZA: ['south africa', 'юар'],
};

const flagOf = (code: string) => String.fromCodePoint(...[...code].map((c) => 0x1f1e6 + c.charCodeAt(0) - 65));

/** ISO 3166 code of the country a server name refers to: its flag emoji, a code like
 *  "NL-01" / "[DE]" / "US |", or a country or city name. */
export function countryCode(name: string): string | null {
  const emoji = name.match(/([\u{1F1E6}-\u{1F1FF}])([\u{1F1E6}-\u{1F1FF}])/u);
  if (emoji) return [emoji[1], emoji[2]].map((c) => String.fromCharCode(c.codePointAt(0)! - 0x1f1e6 + 65)).join('');
  for (const token of name.match(/(?<![A-Za-z])[A-Z]{2}(?![A-Za-z])/g) ?? []) {
    const code = token === 'UK' ? 'GB' : token;
    if (code in COUNTRIES) return code;
  }
  const lower = name.toLowerCase();
  for (const [code, names] of Object.entries(COUNTRIES)) {
    if (names.some((n) => (n.length <= 3 ? new RegExp(`(^|[^\\p{L}])${n}([^\\p{L}]|$)`, 'u').test(lower) : lower.includes(n)))) return code;
  }
  return null;
}

/** Flag emoji for a server name (see `countryCode`), rendered by the bundled flag font. */
export function flag(name: string): string | null {
  const code = countryCode(name);
  return code ? flagOf(code) : null;
}

/** Flag emoji of an ISO 3166 code. */
export function flagOfCode(code: string): string {
  return /^[A-Z]{2}$/.test(code) ? flagOf(code) : '';
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
