import { spawn } from "node:child_process";
import { once } from "node:events";
import { AppServerClient } from "./client.mjs";
import { codexCommand, discoverCodex, readCodexVersion } from "./discovery.mjs";
import { summarizeAccount, summarizeRateLimits, sanitizeText } from "./sanitize.mjs";
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function connect() { const discovery = await discoverCodex(); const client = new AppServerClient(codexCommand(discovery.executable, ["app-server"])); await client.start(); return { discovery, client }; }
async function probe() {
  const { discovery, client } = await connect();
  try {
    const version = await readCodexVersion(discovery.executable); const account = await client.request("account/read", { refreshToken: false }); const limits = await client.request("account/rateLimits/read");
    let missingMethodClassified = false; let missingMethodRpcCode = null;
    try { await client.request("s4quota/capabilityProbe/missing"); } catch (error) { missingMethodRpcCode = Number.isInteger(error.rpcCode) ? error.rpcCode : null; missingMethodClassified = [-32600, -32601].includes(error.rpcCode) || /-32601|not found|unknown method/i.test(error.message); }
    return { outcome: "success", discovery: { source: discovery.source, candidateCount: discovery.candidates }, versionDiagnostic: version, account: summarizeAccount(account), rateLimits: summarizeRateLimits(limits), missingMethodClassified, missingMethodRpcCode, protocolErrorCount: client.protocolErrors.length, stderrObserved: client.stderr.length > 0, states: client.states };
  } finally { await client.shutdown(); }
}
async function observe({ seconds = 20 } = {}) { const { client } = await connect(); try { await client.request("account/rateLimits/read"); const before = client.notifications.length; await sleep(seconds * 1000); const observed = client.notifications.slice(before).map((item) => item.method); return { outcome: "success", seconds, notificationCount: observed.length, rateLimitUpdateObserved: observed.includes("account/rateLimits/updated"), notificationMethods: [...new Set(observed)].map(sanitizeText) }; } finally { await client.shutdown(); } }
async function crossProcess() {
  const { discovery, client } = await connect();
  try {
    await client.request("account/rateLimits/read"); const before = client.notifications.length;
    const workerCommand = codexCommand(discovery.executable, ["exec", "--skip-git-repo-check", "--sandbox", "read-only", "Reply with exactly: OK"]);
    const worker = spawn(workerCommand.executable, workerCommand.args, { cwd: process.cwd(), stdio: ["ignore", "ignore", "ignore"], windowsHide: true, shell: false }); const [exitCode] = await once(worker, "exit"); await sleep(5000);
    const observed = client.notifications.slice(before).map((item) => item.method); const refreshed = await client.request("account/rateLimits/read");
    return { outcome: exitCode === 0 ? "success" : "worker_failed", workerExitCode: exitCode, rateLimitUpdateObserved: observed.includes("account/rateLimits/updated"), notificationMethods: [...new Set(observed)].map(sanitizeText), refreshedSnapshot: summarizeRateLimits(refreshed) };
  } finally { await client.shutdown(); }
}
const command = process.argv[2] ?? "probe";
try { const result = command === "probe" ? await probe() : command === "observe" ? await observe({ seconds: Number(process.argv[3] ?? 20) }) : command === "cross-process" ? await crossProcess() : (() => { throw new Error(`unknown command: ${command}`); })(); console.log(JSON.stringify(result, null, 2)); }
catch (error) { console.error(JSON.stringify({ outcome: "failure", error: sanitizeText(error.message) }, null, 2)); process.exitCode = 1; }
