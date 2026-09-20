import { access } from "node:fs/promises";
import { constants } from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
const execFileAsync = promisify(execFile);
async function isExecutable(path) { try { await access(path, constants.F_OK); return true; } catch { return false; } }
export function codexCommand(executable, args) {
  if (process.platform !== "win32" || executable.toLowerCase().endsWith(".exe")) return { executable, args };
  if (executable.toLowerCase().endsWith(".cmd")) return { executable: process.env.ComSpec ?? "cmd.exe", args: ["/d", "/c", "call", executable, ...args] };
  throw new Error("PowerShell and extensionless script shims are not supported by the spike launcher");
}
export async function discoverCodex() {
  const override = process.env.CODEX_SPIKE_EXECUTABLE;
  if (override) { if (!(await isExecutable(override))) throw new Error("CODEX_SPIKE_EXECUTABLE does not exist"); return { executable: override, source: "environment_override", candidates: 1 }; }
  const locator = process.platform === "win32" ? "where.exe" : "which";
  const { stdout } = await execFileAsync(locator, ["codex"], { windowsHide: true, timeout: 5000 });
  const paths = [...new Set(stdout.split(/\r?\n/).map((line) => line.trim()).filter(Boolean))];
  if (!paths.length) throw new Error("Codex executable was not found on PATH");
  const native = process.platform === "win32" ? paths.find((path) => path.toLowerCase().endsWith(".exe")) : paths[0];
  if (!native) throw new Error("Only script shims were found; a native Codex executable is required by this spike");
  return { executable: native, source: "path_native", candidates: paths.length };
}
export async function readCodexVersion(executable) {
  const command = codexCommand(executable, ["--version"]);
  const { stdout } = await execFileAsync(command.executable, command.args, { windowsHide: true, timeout: 5000 });
  const match = stdout.trim().match(/^codex-cli\s+([0-9A-Za-z.+-]+)$/);
  return match?.[1] ?? "unrecognized";
}
