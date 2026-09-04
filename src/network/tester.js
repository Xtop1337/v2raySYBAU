import { tcpProbe, icmpProbe, httpProxyProbe } from './probes.js';

export class ProfileTester {
  constructor({ telemetry, capture, probes = { tcpProbe, icmpProbe, httpProxyProbe } }) {
    this.telemetry = telemetry;
    this.capture = capture;
    this.probes = probes;
  }

  async test(profile, options = {}) {
    const checks = [];
    if (profile.host && profile.port) checks.push(this.probes.tcpProbe({ host: profile.host, port: profile.port, ...options }));
    if (options.icmp) checks.push(this.probes.icmpProbe({ host: profile.host, ...options }));
    if (options.httpUrl && profile.proxyUrl) checks.push(this.probes.httpProxyProbe({ url: options.httpUrl, proxyUrl: profile.proxyUrl, ...options }));
    const results = await Promise.all(checks);
    for (const result of results) {
      this.capture?.record({ profileId: profile.id, type: result.type, ok: result.ok, durationMs: result.durationMs, bytes: result.bytes, error: result.error });
      this.telemetry.record(profile.id, result);
    }
    return results;
  }

  async testAll(profiles, options) {
    return Promise.all(profiles.map(async (profile) => ({ profileId: profile.id, results: await this.test(profile, options) })));
  }
}
