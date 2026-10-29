// Mirrors of the Rust models (serde camelCase).

export type Protocol = 'vless' | 'vmess' | 'trojan' | 'shadowsocks' | 'hysteria2' | 'tuic';
export type Network = 'tcp' | 'ws' | 'grpc' | 'xhttp' | 'httpupgrade' | 'h2' | 'kcp' | 'quic';
export type Security = 'none' | 'tls' | 'reality';
export type TestMethod = 'real' | 'tcp' | 'icmp';

export interface Transport {
  network: Network;
  headerType: string;
  host: string;
  path: string;
  serviceName: string;
  mode: string;
  extra: string;
  seed: string;
}

export interface TlsOptions {
  security: Security;
  sni: string;
  alpn: string[];
  fingerprint: string;
  allowInsecure: boolean;
  publicKey: string;
  shortId: string;
  spiderX: string;
  mldsa65Verify: string;
  pinnedSha256: string;
  verifyNames: string;
  echConfig: string;
}

export interface QuicOptions {
  obfs: string;
  obfsPassword: string;
  upMbps: number;
  downMbps: number;
  hopPorts: string;
  congestionControl: string;
  udpRelayMode: string;
}

export interface DelayInfo {
  ms: number | null;
  method: TestMethod | null;
  testedAt: string | null;
}

export interface Profile {
  id: string;
  name: string;
  group: string;
  subscriptionId: string | null;
  protocol: Protocol;
  address: string;
  port: number;
  uuid: string;
  password: string;
  alterId: number;
  method: string;
  flow: string;
  plugin: string;
  transport: Transport;
  tls: TlsOptions;
  quic: QuicOptions;
  dns: string;
  delay: DelayInfo;
  createdAt: string;
}

export interface Subscription {
  id: string;
  name: string;
  url: string;
  enabled: boolean;
  autoUpdateHours: number;
  userAgent: string;
  updatedAt: string | null;
  lastError: string | null;
  upload: number | null;
  download: number | null;
  total: number | null;
  expire: number | null;
}

export type RuleKind = 'domain' | 'ip' | 'port' | 'process' | 'protocol' | 'network';
export type RuleOutbound = 'proxy' | 'direct' | 'block';

export interface RoutingRule {
  id: string;
  enabled: boolean;
  remark: string;
  kind: RuleKind;
  values: string[];
  outbound: RuleOutbound;
}

export type Theme = 'system' | 'light' | 'dark';
export type ConnectionMode = 'proxy' | 'tun';
export type TunAppMode = 'all' | 'include' | 'exclude';
export type TunEngine = 'singbox' | 'xray';
export type RoutingPreset = 'global' | 'bypassLan' | 'bypassLanRu' | 'directByDefault';
export type MonitorScope = 'active' | 'group' | 'all';

export interface Settings {
  general: {
    language: string;
    theme: Theme;
    autostart: boolean;
    autoConnect: boolean;
    scanOnStart: boolean;
    confirmDelete: boolean;
    showSpeed: boolean;
    minimizeToTray: boolean;
    startMinimized: boolean;
    updateSubscriptionsOnStart: boolean;
    checkUpdates: boolean;
  };
  proxy: {
    mode: ConnectionMode;
    socksPort: number;
    httpPort: number;
    systemProxy: boolean;
    bypass: string;
    allowLan: boolean;
  };
  tun: {
    engine: TunEngine;
    appMode: TunAppMode;
    apps: string[];
    stack: string;
    mtu: number;
    strictRoute: boolean;
  };
  dns: {
    remote: string;
    direct: string;
    useProfileDns: boolean;
    localDns: boolean;
    localPort: number;
    fakeDns: boolean;
    queryStrategy: string;
    hosts: string;
  };
  routing: {
    preset: RoutingPreset;
    domainStrategy: string;
    sniffing: boolean;
    routeOnly: boolean;
    blockAds: boolean;
    rules: RoutingRule[];
  };
  mux: { enabled: boolean; concurrency: number; xudpConcurrency: number; xudpProxyUdp443: string };
  fragment: { enabled: boolean; packets: string; length: string; interval: string };
  analysis: { enabled: boolean; intervalSecs: number; window: number; method: TestMethod; scope: MonitorScope };
  test: { url: string; timeoutMs: number; concurrency: number; method: TestMethod };
  advanced: {
    logLevel: string;
    accessLog: boolean;
    allowInsecure: boolean;
    subscriptionUserAgent: string;
    configOverride: string;
    updatePrerelease: boolean;
    apiPort: number;
  };
  lastProfileId: string | null;
}

export interface ConnectionInfo {
  connected: boolean;
  profileId: string | null;
  profileName: string | null;
  mode: ConnectionMode | null;
  since: string | null;
  systemProxy: boolean;
  core: string | null;
}

export interface Snapshot {
  profiles: Profile[];
  subscriptions: Subscription[];
  settings: Settings;
  connection: ConnectionInfo;
  appVersion: string;
  elevated: boolean;
  dataDir: string;
  platform: string;
}

export interface ImportSummary {
  added: number;
  duplicates: number;
  subscriptions: number;
  restoredBackup: boolean;
  errors: string[];
}

export interface DelayResult {
  profileId: string;
  ms: number;
  method: TestMethod;
}

export interface TestProgress {
  done: number;
  total: number;
  running: boolean;
}

export interface TrafficSample {
  up: number;
  down: number;
  directUp: number;
  directDown: number;
  totalUp: number;
  totalDown: number;
}

export interface NodeStats {
  profileId: string;
  last: number | null;
  avg: number | null;
  min: number | null;
  max: number | null;
  jitter: number | null;
  loss: number;
  sent: number;
  lost: number;
  history: number[];
}

export type LogLevel = 'debug' | 'info' | 'warning' | 'error';
export type LogSource = 'app' | 'core' | 'access';

export interface RequestRecord {
  source: string;
  destination: string;
  network: string;
  inbound: string;
  outbound: string;
  accepted: boolean;
}

export interface LogEntry {
  at: string;
  level: LogLevel;
  source: LogSource;
  message: string;
  request?: RequestRecord;
}

export interface CoreVersions {
  xray: string | null;
  singbox: string | null;
  geo: boolean;
  dir: string;
}
