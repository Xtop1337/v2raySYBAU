import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeSettings } from '../src/config.js';
import { PacketCapture } from '../src/network/capture.js';
import { TelemetryStore } from '../src/network/telemetry.js';
import { ProfileTester } from '../src/network/tester.js';
import { icmpProbe } from '../src/network/probes.js';
import { resolveRoute } from '../src/routing.js';
import { compareVersions, GitHubUpdater } from '../src/updates/github.js';

test('settings validates DNS and clamps capture size', () => {
  const settings = normalizeSettings({ dns: { fromKey: true, local: true, fakeDns: true, localPort: 5354, customServer: 'https://dns.example/dns-query' }, packetCapture: { enabled: true, maxEntries: 99_999, maxBytes: 1 } });
  assert.equal(settings.dns.localPort, 5354); assert.equal(settings.packetCapture.maxEntries, 5_000); assert.equal(settings.packetCapture.maxBytes, 1_024);
  assert.throws(() => normalizeSettings({ dns: { localPort: 0 } }));
});

test('capture is opt-in and bounded', () => {
  const capture = new PacketCapture({ enabled: true, maxEntries: 2, maxBytes: 500 });
  capture.record({ id: 1 }); capture.record({ id: 2 }); capture.record({ id: 3 });
  assert.deepEqual(capture.snapshot().map((item) => item.id), [2, 3]);
  assert.equal(new PacketCapture().snapshot().length, 0);
});

test('batch tests publish live latency, loss and speed', async () => {
  const telemetry = new TelemetryStore();
  const tester = new ProfileTester({ telemetry, probes: {
    tcpProbe: async () => ({ type: 'tcp', ok: true, durationMs: 20, bytes: 100 }),
    icmpProbe: async () => ({ type: 'icmp', ok: false, durationMs: 10, bytes: 0, error: 'unreachable' }),
    httpProxyProbe: async () => ({ type: 'http', ok: true, durationMs: 30, bytes: 300 })
  } });
  await tester.testAll([{ id: 'a', host: 'a.test', port: 443, proxyUrl: 'socks5://a' }], { icmp: true, httpUrl: 'https://test/' });
  assert.deepEqual(telemetry.get('a').latencyMs, 25); assert.equal(telemetry.get('a').lossPercent, 33.33); assert.equal(telemetry.get('a').speedBps, 6667);
});

test('Windows ICMP is safely skipped', async () => {
  const result = await icmpProbe({ host: 'example.test', platform: 'win32' });
  assert.equal(result.skipped, true); assert.match(result.error, /Windows/);
});

test('domain and selected-application routing is deterministic', () => {
  const settings = { proxySelectedApplicationsOnly: true, applications: ['browser.exe'], rules: [{ domain: '*.internal.test', action: 'direct' }] };
  assert.equal(resolveRoute('api.internal.test', 'browser.exe', settings), 'direct');
  assert.equal(resolveRoute('public.test', 'other.exe', settings), 'direct');
  assert.equal(resolveRoute('public.test', 'browser.exe', settings), 'proxy');
});

test('update checks and semver ordering', async () => {
  assert.equal(compareVersions('1.10.0', '1.9.0'), 1);
  const updater = new GitHubUpdater({ owner: 'o', repo: 'r', currentVersion: '1.0.0', fetchImpl: async () => new Response(JSON.stringify({ tag_name: 'v1.1.0', assets: [] })), notify: () => {} });
  assert.equal((await updater.check()).available, true);
});
