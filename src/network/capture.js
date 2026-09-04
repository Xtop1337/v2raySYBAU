/** A bounded, opt-in metadata recorder. Payloads are intentionally never retained. */
export class PacketCapture {
  #entries = [];
  #bytes = 0;

  constructor({ enabled = false, maxEntries = 500, maxBytes = 1_000_000 } = {}) {
    this.enabled = enabled;
    this.maxEntries = maxEntries;
    this.maxBytes = maxBytes;
  }

  record(metadata) {
    if (!this.enabled) return;
    const entry = JSON.stringify({ at: new Date().toISOString(), ...metadata });
    const bytes = Buffer.byteLength(entry);
    if (bytes > this.maxBytes) return;
    while (this.#entries.length && (this.#entries.length >= this.maxEntries || this.#bytes + bytes > this.maxBytes)) {
      this.#bytes -= this.#entries.shift().bytes;
    }
    this.#entries.push({ entry, bytes });
    this.#bytes += bytes;
  }

  snapshot() { return this.#entries.map(({ entry }) => JSON.parse(entry)); }
  clear() { this.#entries = []; this.#bytes = 0; }
}
