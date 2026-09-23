import type { AppSnapshot, ProviderState, QuotaSnapshot, QuotaWindow, QuotaWindowKind } from "./contracts.ts";

export type Freshness = "fresh" | "refreshing" | "stale" | "degraded" | "unavailable" | "unsupported" | "auth" | "starting";

export interface DisplayWindow extends QuotaWindow {
  remainingPercent: number | null;
  resetsAtMs: number | null;
  durationMinutes: number | null;
  limitStatusText: string | null;
}

export interface QuotaViewModel {
  snapshot: QuotaSnapshot | null;
  primary: DisplayWindow | null;
  weekly: DisplayWindow | null;
  limiting: DisplayWindow | null;
  freshness: Freshness;
  ageSeconds: number | null;
  providerMessage: string | null;
  providerAction: "retry" | "authenticate" | "details" | null;
}

const FRESHNESS_MAX_AGE_SECONDS = 180;

function numberValue(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim() !== "") {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

export function percentValue(value: unknown): number | null {
  const parsed = numberValue(value);
  if (parsed === null) return null;
  return Math.min(100, Math.max(0, parsed));
}

export function epochMilliseconds(value: unknown): number | null {
  const numeric = numberValue(value);
  if (numeric !== null) {
    if (numeric <= 0) return null;
    return numeric < 10_000_000_000 ? numeric * 1000 : numeric;
  }
  if (typeof value === "string") {
    const parsed = Date.parse(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function statusText(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const trimmed = value.trim();
  return trimmed === "" ? null : trimmed;
}

export function toDisplayWindow(window: QuotaWindow): DisplayWindow {
  return {
    ...window,
    remainingPercent: percentValue(window.remainingPercent),
    resetsAtMs: epochMilliseconds(window.resetsAt),
    durationMinutes: numberValue(window.durationMinutes),
    limitStatusText: statusText(window.limitStatus),
  };
}

function durationOf(window: DisplayWindow): number {
  return window.durationMinutes ?? Number.POSITIVE_INFINITY;
}

function kindRank(kind: QuotaWindowKind): number {
  if (kind === "rolling") return 0;
  if (kind === "weekly") return 1;
  return 2;
}

/**
 * Deterministic limiting-window rule:
 * lowest known remaining percentage wins; ties prefer the shorter duration,
 * then rolling over weekly/other, then a stable id. Unknown percentages never
 * become zero and therefore never become limiting by accident.
 */
export function findLimitingWindow(windows: DisplayWindow[]): DisplayWindow | null {
  return windows
    .filter((window) => window.remainingPercent !== null)
    .sort((left, right) => {
      const remaining = left.remainingPercent! - right.remainingPercent!;
      if (remaining !== 0) return remaining;
      const duration = durationOf(left) - durationOf(right);
      if (duration !== 0) return duration;
      const kind = kindRank(left.kind) - kindRank(right.kind);
      if (kind !== 0) return kind;
      return left.id.localeCompare(right.id);
    })[0] ?? null;
}

function findKind(windows: DisplayWindow[], kind: QuotaWindowKind, duration: number): DisplayWindow | null {
  return windows.find((window) => window.kind === kind && window.durationMinutes === duration)
    ?? windows.find((window) => window.kind === kind)
    ?? null;
}

export function snapshotFromProvider(provider: ProviderState, fallback: QuotaSnapshot | null = null): QuotaSnapshot | null {
  if (provider.status === "ready") return provider.snapshot;
  if (provider.status === "refreshing") return provider.previousSnapshot;
  if (provider.status === "degraded") return provider.snapshot;
  return fallback;
}

function ageSeconds(snapshot: QuotaSnapshot | null, nowMs: number): number | null {
  if (!snapshot || !Number.isFinite(snapshot.fetchedAt)) return null;
  return Math.max(0, nowMs / 1000 - snapshot.fetchedAt);
}

function freshnessFor(provider: ProviderState, snapshot: QuotaSnapshot | null, nowMs: number): Freshness {
  switch (provider.status) {
    case "needsAuthentication": return "auth";
    case "unsupported": return "unsupported";
    case "unavailable": return "unavailable";
    case "starting":
    case "discovering":
    case "handshaking":
    case "stopping":
    case "stopped": return snapshot ? "stale" : "starting";
    case "refreshing": return snapshot ? "refreshing" : "starting";
    case "degraded": return snapshot ? "degraded" : "unavailable";
    case "ready": {
      const age = ageSeconds(snapshot, nowMs);
      return age !== null && age <= FRESHNESS_MAX_AGE_SECONDS ? "fresh" : "stale";
    }
  }
}

function providerMessage(provider: ProviderState, freshness: Freshness, age: number | null): string | null {
  switch (provider.status) {
    case "needsAuthentication": return "Sign in to Codex to read quota.";
    case "unsupported": return "This Codex installation does not expose the required quota capabilities.";
    case "unavailable": return provider.reason;
    case "degraded": return age === null ? provider.error : `Stale · ${provider.error}`;
    case "refreshing": return "Refreshing · last value retained";
    case "ready": return freshness === "stale" ? `Stale · updated ${formatAge(age)}` : null;
    default: return null;
  }
}

function providerAction(provider: ProviderState): QuotaViewModel["providerAction"] {
  if (provider.status === "needsAuthentication") return "authenticate";
  if (provider.status === "unsupported") return "details";
  if (provider.status === "unavailable" || provider.status === "degraded") return "retry";
  return null;
}

function formatAge(age: number | null): string {
  if (age === null) return "an unknown time";
  if (age < 60) return `${Math.max(1, Math.floor(age))}s ago`;
  return `${Math.floor(age / 60)}m ago`;
}

export function buildQuotaViewModel(app: AppSnapshot, nowMs = Date.now()): QuotaViewModel {
  const snapshot = snapshotFromProvider(app.provider, app.quota);
  const windows = snapshot?.windows.map(toDisplayWindow) ?? [];
  const age = ageSeconds(snapshot, nowMs);
  const freshness = freshnessFor(app.provider, snapshot, nowMs);
  return {
    snapshot,
    primary: findKind(windows, "rolling", 300),
    weekly: findKind(windows, "weekly", 10_080),
    limiting: findLimitingWindow(windows),
    freshness,
    ageSeconds: age,
    providerMessage: providerMessage(app.provider, freshness, age),
    providerAction: providerAction(app.provider),
  };
}

export function formatCountdown(resetsAtMs: number | null, nowMs = Date.now()): string | null {
  if (resetsAtMs === null) return null;
  const remainingMs = Math.max(0, resetsAtMs - nowMs);
  const totalMinutes = Math.ceil(remainingMs / 60_000);
  const days = Math.floor(totalMinutes / (24 * 60));
  if (days > 0) {
    const hours = Math.floor((totalMinutes % (24 * 60)) / 60);
    return `${days}d ${String(hours).padStart(2, "0")}h`;
  }
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;
  return `${String(hours).padStart(2, "0")}:${String(minutes).padStart(2, "0")}`;
}

export function formatResetDate(resetsAtMs: number | null, locale = navigator.language): string | null {
  if (resetsAtMs === null) return null;
  return new Intl.DateTimeFormat(locale, { weekday: "short", hour: "2-digit", minute: "2-digit" }).format(resetsAtMs);
}

export function isAtLimit(window: DisplayWindow | null): boolean {
  return window?.remainingPercent === 0;
}
