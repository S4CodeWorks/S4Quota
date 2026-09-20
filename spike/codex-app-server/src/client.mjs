import { spawn } from "node:child_process";
import { once } from "node:events";
import { JsonlDecoder } from "./jsonl.mjs";
import { sanitizeText } from "./sanitize.mjs";
export class AppServerClient {
  constructor({ executable, args = ["app-server"], requestTimeoutMs = 8000, maxStderrBytes = 64 * 1024 }) {
    this.executable = executable; this.args = args; this.requestTimeoutMs = requestTimeoutMs; this.maxStderrBytes = maxStderrBytes;
    this.pending = new Map(); this.notifications = []; this.protocolErrors = []; this.states = ["stopped"]; this.stderr = ""; this.nextId = 1;
  }
  async start() {
    if (this.child) throw new Error("client already started");
    this.states.push("starting");
    this.child = spawn(this.executable, this.args, { stdio: ["pipe", "pipe", "pipe"], windowsHide: true, shell: false });
    this.decoder = new JsonlDecoder(); this.child.stdout.on("data", (chunk) => this.#consume(chunk));
    this.child.stderr.on("data", (chunk) => { this.stderr = (this.stderr + chunk.toString("utf8")).slice(-this.maxStderrBytes); });
    this.exitPromise = once(this.child, "exit").then(([code, signal]) => { this.states.push("exited"); const error = new Error(`app-server exited (code=${code}, signal=${signal})`); for (const request of this.pending.values()) request.reject(error); this.pending.clear(); return { code, signal }; });
    await once(this.child, "spawn"); this.states.push("handshaking");
    const initialized = await this.request("initialize", { clientInfo: { name: "s4quota-provider-spike", version: "0.1.0" }, capabilities: { experimentalApi: false } });
    this.notify("initialized", {}); this.states.push("ready"); return initialized;
  }
  #consume(chunk) {
    for (const event of this.decoder.push(chunk)) {
      if (!event.ok) { this.protocolErrors.push(event.error); continue; }
      const message = event.value;
      if (Object.hasOwn(message, "id")) {
        const pending = this.pending.get(message.id); if (!pending) continue;
        clearTimeout(pending.timer); this.pending.delete(message.id);
        if (message.error) { const error = new Error(`JSON-RPC ${message.error.code}: ${sanitizeText(message.error.message)}`); error.rpcCode = message.error.code; pending.reject(error); }
        else pending.resolve(message.result);
      } else if (typeof message.method === "string") this.notifications.push({ method: message.method, params: message.params });
    }
  }
  request(method, params) {
    if (!this.child?.stdin?.writable) return Promise.reject(new Error("app-server stdin is not writable"));
    const id = this.nextId++;
    return new Promise((resolve, reject) => { const timer = setTimeout(() => { this.pending.delete(id); reject(new Error(`JSON-RPC request timed out: ${method}`)); }, this.requestTimeoutMs); timer.unref?.(); this.pending.set(id, { resolve, reject, timer }); this.child.stdin.write(`${JSON.stringify({ method, id, ...(params === undefined ? {} : { params }) })}\n`); });
  }
  notify(method, params) { if (!this.child?.stdin?.writable) throw new Error("app-server stdin is not writable"); this.child.stdin.write(`${JSON.stringify({ method, params })}\n`); }
  async shutdown({ graceMs = 1500 } = {}) {
    if (!this.child || this.child.exitCode !== null) return this.exitPromise;
    this.states.push("stopping"); this.child.stdin.end();
    const graceful = await Promise.race([this.exitPromise.then((result) => ({ graceful: true, ...result })), new Promise((resolve) => setTimeout(() => resolve(null), graceMs))]);
    if (graceful) return graceful;
    await this.#killTree(); const result = await this.exitPromise; return { graceful: false, ...result };
  }
  async #killTree() {
    if (!this.child?.pid) return;
    if (process.platform === "win32") { const killer = spawn("taskkill.exe", ["/PID", String(this.child.pid), "/T", "/F"], { stdio: "ignore", windowsHide: true, shell: false }); await once(killer, "exit"); }
    else this.child.kill("SIGKILL");
  }
}
