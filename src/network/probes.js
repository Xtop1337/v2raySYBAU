import net from 'node:net';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';

const execute = promisify(execFile);

export async function tcpProbe({ host, port, timeoutMs = 5_000 }) {
  const started = performance.now();
  return new Promise((resolve) => {
    const socket = net.connect({ host, port });
    let settled = false;
    const finish = (ok, error) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      resolve({ type: 'tcp', ok, durationMs: Math.round(performance.now() - started), bytes: 0, error });
    };
    socket.setTimeout(timeoutMs, () => finish(false, 'timeout'));
    socket.once('connect', () => finish(true));
    socket.once('error', (error) => finish(false, error.code ?? error.message));
  });
}

export async function icmpProbe({ host, platform = process.platform, executeCommand = execute }) {
  if (platform === 'win32') {
    return { type: 'icmp', ok: false, skipped: true, durationMs: null, bytes: 0, error: 'ICMP is disabled on Windows; use TCP or enable a privileged helper.' };
  }
  const started = performance.now();
  try {
    await executeCommand('ping', ['-c', '1', '-W', '2', host], { timeout: 3_000 });
    return { type: 'icmp', ok: true, durationMs: Math.round(performance.now() - started), bytes: 0 };
  } catch (error) {
    return { type: 'icmp', ok: false, durationMs: Math.round(performance.now() - started), bytes: 0, error: error.code === 'ENOENT' ? 'ping command unavailable' : 'unreachable' };
  }
}

export async function httpProxyProbe({ url, proxyUrl, timeoutMs = 8_000, fetchImpl = fetch }) {
  const started = performance.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    // Node's built-in fetch has no proxy option. The core must expose a proxy-aware fetch adapter.
    const response = await fetchImpl(url, { method: 'GET', signal: controller.signal, proxyUrl });
    const body = await response.arrayBuffer();
    return { type: 'http', ok: response.ok, status: response.status, durationMs: Math.round(performance.now() - started), bytes: body.byteLength, error: response.ok ? undefined : `HTTP ${response.status}` };
  } catch (error) {
    return { type: 'http', ok: false, durationMs: Math.round(performance.now() - started), bytes: 0, error: error.name === 'AbortError' ? 'timeout' : error.message };
  } finally {
    clearTimeout(timeout);
  }
}
