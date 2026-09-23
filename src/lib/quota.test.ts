import { buildQuotaViewModel, formatCountdown, findLimitingWindow, toDisplayWindow } from "./quota.ts";
import type { AppSnapshot, QuotaWindow } from "./contracts.ts";

function assert(condition: boolean, message: string): asserts condition {
  if (!condition) throw new Error(message);
}

const rolling: QuotaWindow = {
  id: "rolling",
  kind: "rolling",
  label: null,
  durationMinutes: 300,
  usedPercent: 28,
  remainingPercent: 72,
  resetsAt: 1_700_000_000,
  limitStatus: "ok",
};

const weekly: QuotaWindow = {
  id: "weekly",
  kind: "weekly",
  label: null,
  durationMinutes: 10_080,
  usedPercent: 16,
  remainingPercent: 84,
  resetsAt: 1_700_400_000,
  limitStatus: "ok",
};

const unknownPercent: QuotaWindow = {
  ...rolling,
  id: "unknown",
  kind: "other",
  remainingPercent: null,
};

const snapshot: AppSnapshot = {
  provider: { status: "ready", snapshot: { providerId: "codex", fetchedAt: 1_700_000_000, windows: [rolling, weekly, unknownPercent] } },
  quota: null,
  settings: { presentationMode: "main", alwaysOnTopCompact: true, autostart: false },
};

const view = buildQuotaViewModel(snapshot, 1_700_000_010_000);
assert(view.primary?.id === "rolling", "rolling window should be the primary five-hour window");
assert(view.weekly?.id === "weekly", "weekly window should be selected by kind and duration");
assert(view.limiting?.id === "rolling", "lowest known remaining percentage should limit");
assert(toDisplayWindow(unknownPercent).remainingPercent === null, "unknown percentage must not become zero");

const tie = findLimitingWindow([
  toDisplayWindow({ ...rolling, id: "longer", durationMinutes: 600, remainingPercent: 20 }),
  toDisplayWindow({ ...rolling, id: "shorter", durationMinutes: 300, remainingPercent: 20 }),
]);
assert(tie?.id === "shorter", "equal percentages should prefer the shorter duration");
assert(formatCountdown(1_700_007_800_000, 1_700_000_000_000) === "02:10", "countdown should format local hours and minutes");

console.log("frontend quota contract tests passed");
