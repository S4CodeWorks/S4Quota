import { StringDecoder } from "node:string_decoder";

export class JsonlDecoder {
  #decoder = new StringDecoder("utf8");
  #buffer = "";
  constructor({ maxFrameBytes = 1024 * 1024 } = {}) { this.maxFrameBytes = maxFrameBytes; }
  push(chunk) {
    this.#buffer += this.#decoder.write(chunk);
    if (Buffer.byteLength(this.#buffer) > this.maxFrameBytes && !this.#buffer.includes("\n")) {
      this.#buffer = "";
      return [{ ok: false, error: "frame_too_large" }];
    }
    const events = [];
    let newline;
    while ((newline = this.#buffer.indexOf("\n")) !== -1) {
      const line = this.#buffer.slice(0, newline).trimEnd();
      this.#buffer = this.#buffer.slice(newline + 1);
      if (!line) continue;
      if (Buffer.byteLength(line) > this.maxFrameBytes) { events.push({ ok: false, error: "frame_too_large" }); continue; }
      try { events.push({ ok: true, value: JSON.parse(line) }); }
      catch { events.push({ ok: false, error: "invalid_json" }); }
    }
    return events;
  }
  end() {
    const tail = this.#buffer + this.#decoder.end();
    this.#buffer = "";
    if (!tail.trim()) return [];
    try { return [{ ok: true, value: JSON.parse(tail) }]; }
    catch { return [{ ok: false, error: "truncated_or_invalid_json" }]; }
  }
}
