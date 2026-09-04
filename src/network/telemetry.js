import { EventEmitter } from 'node:events';

/** Emits a compact live sample after every probe, suitable for UI subscriptions. */
export class TelemetryStore extends EventEmitter {
  #nodes = new Map();

  record(profileId, result) {
    const previous = this.#nodes.get(profileId) ?? { samples: [] };
    const samples = [...previous.samples, result].slice(-60);
    const completed = samples.filter((sample) => sample.ok);
    const elapsed = samples.reduce((sum, sample) => sum + (sample.durationMs ?? 0), 0);
    const transferred = samples.reduce((sum, sample) => sum + (sample.bytes ?? 0), 0);
    const live = {
      profileId,
      latencyMs: completed.length ? Math.round(completed.reduce((sum, sample) => sum + sample.durationMs, 0) / completed.length) : null,
      lossPercent: Math.round(((samples.length - completed.length) / samples.length) * 10000) / 100,
      speedBps: elapsed ? Math.round((transferred * 1000) / elapsed) : 0,
      lastError: result.ok ? null : result.error,
      updatedAt: new Date().toISOString(),
      samples
    };
    this.#nodes.set(profileId, live);
    this.emit('sample', live);
    return live;
  }

  get(profileId) { return this.#nodes.get(profileId); }
  all() { return [...this.#nodes.values()]; }
}
