/** Validated settings passed to the network core; never store secrets in logs. */
export function normalizeSettings(input = {}) {
  const dns = input.dns ?? {};
  const localDnsPort = Number(dns.localPort ?? 5353);
  if (!Number.isInteger(localDnsPort) || localDnsPort < 1 || localDnsPort > 65535) {
    throw new Error('DNS local port must be between 1 and 65535');
  }
  const customServer = dns.customServer?.trim();
  if (customServer && !/^https?:\/\/[^\s]+$/.test(customServer)) {
    throw new Error('Custom DNS server must be an HTTP(S) URL');
  }
  return {
    dns: {
      fromKey: Boolean(dns.fromKey),
      local: Boolean(dns.local),
      fakeDns: Boolean(dns.fakeDns),
      localPort: localDnsPort,
      customServer: customServer || null
    },
    transport: {
      multiplex: Boolean(input.transport?.multiplex),
      fragment: Boolean(input.transport?.fragment)
    },
    packetCapture: {
      enabled: Boolean(input.packetCapture?.enabled),
      maxEntries: clamp(Number(input.packetCapture?.maxEntries ?? 500), 1, 5_000),
      maxBytes: clamp(Number(input.packetCapture?.maxBytes ?? 1_000_000), 1_024, 10_000_000)
    }
  };
}

export function clamp(value, minimum, maximum) {
  return Math.min(maximum, Math.max(minimum, value));
}
