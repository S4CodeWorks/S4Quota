export type ProviderId = "mock" | "codex";
export type QuotaWindowKind = "rolling" | "weekly" | "other";
export type PresentationMode = "main" | "compact";

// Provider-specific wire fields are concrete in the Rust adapter. The shared
// domain remains intentionally provider-agnostic at this boundary.
export interface QuotaWindow { id: string; kind: QuotaWindowKind; label: string | null; durationMinutes: unknown | null; usedPercent: unknown | null; remainingPercent: unknown | null; resetsAt: unknown | null; limitStatus: unknown; }
export interface QuotaSnapshot { providerId: ProviderId; fetchedAt: number; windows: QuotaWindow[]; }
export type ProviderState =
  | { status: "stopped" | "discovering" | "needsAuthentication" }
  | { status: "starting"; executable: string }
  | { status: "handshaking"; processId: number | null }
  | { status: "refreshing"; previousSnapshot: QuotaSnapshot | null }
  | { status: "ready"; snapshot: QuotaSnapshot }
  | { status: "degraded"; snapshot: QuotaSnapshot | null; error: string; retryAt: number | null }
  | { status: "unsupported"; missingCapabilities: string[]; version: string | null }
  | { status: "unavailable"; reason: string }
  | { status: "stopping" };
export interface AppSettings { presentationMode: PresentationMode; alwaysOnTopCompact: boolean; autostart: boolean; }
export interface AppSnapshot { provider: ProviderState; quota: QuotaSnapshot | null; settings: AppSettings; }
