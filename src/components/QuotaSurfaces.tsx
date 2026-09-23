import type { CSSProperties, ReactNode } from "react";
import { formatCountdown, formatResetDate, isAtLimit, type DisplayWindow, type QuotaViewModel } from "../lib/quota";
import markSmall from "../../assets/branding/s4quota-mark-small.png";

type WindowAction = "minimize" | "toggleMaximize" | "close";

interface SurfaceProps {
  model: QuotaViewModel;
  nowMs: number;
  onRefresh: () => void;
  onSwitch: () => void;
  onWindowAction: (action: WindowAction) => void;
}

function MinusIcon() {
  return <svg aria-hidden="true" viewBox="0 0 16 16"><path d="M3 8h10" /></svg>;
}

function MaximizeIcon() {
  return <svg aria-hidden="true" viewBox="0 0 16 16"><rect x="3.25" y="3.25" width="9.5" height="9.5" /></svg>;
}

function CloseIcon() {
  return <svg aria-hidden="true" viewBox="0 0 16 16"><path d="m4 4 8 8m0-8-8 8" /></svg>;
}

function RefreshIcon() {
  return <svg aria-hidden="true" viewBox="0 0 16 16"><path d="M13 7a5 5 0 1 0 1 3m-1-6v3h-3" /></svg>;
}

function CompactIcon() {
  return <svg aria-hidden="true" viewBox="0 0 16 16"><rect x="2.5" y="4" width="11" height="8" rx="1" /><path d="M5 7h6M5 9h4" /></svg>;
}

function WindowChrome({ mode, onSwitch, onWindowAction }: { mode: "main" | "compact"; onSwitch: () => void; onWindowAction: (action: WindowAction) => void }) {
  return (
    <header className={`s4-window-chrome s4-window-chrome--${mode}`}>
      <div className="s4-window-brand" data-tauri-drag-region>
        <img src={markSmall} alt="" aria-hidden="true" />
        {mode === "main" && <span>S4Quota</span>}
      </div>
      <nav className="s4-window-controls" aria-label="Window controls">
        {mode === "main" && (
          <>
            <button className="s4-window-control s4-focus-target" type="button" aria-label="Minimize window" onClick={() => onWindowAction("minimize")}><MinusIcon /></button>
            <button className="s4-window-control s4-focus-target" type="button" aria-label="Maximize or restore window" onClick={() => onWindowAction("toggleMaximize")}><MaximizeIcon /></button>
          </>
        )}
        <button className="s4-window-control s4-focus-target" type="button" aria-label={mode === "main" ? "Open compact mode" : "Open main window"} onClick={onSwitch}><CompactIcon /></button>
        <button className="s4-window-control s4-focus-target" type="button" aria-label={mode === "compact" ? "Return to main window" : "Close S4Quota"} onClick={() => onWindowAction("close")}><CloseIcon /></button>
      </nav>
    </header>
  );
}

function windowLabel(window: DisplayWindow | null): string {
  if (!window) return "Quota window";
  if (window.label) return window.label;
  if (window.kind === "rolling") return "5-hour window";
  if (window.kind === "weekly") return "Weekly window";
  return "Additional window";
}

function limitingCopy(model: QuotaViewModel, primary: DisplayWindow | null): string {
  if (!model.limiting) return "Limiting window unavailable";
  if (primary && model.limiting.id === primary.id) return "Currently limiting";
  return `Currently limiting · ${windowLabel(model.limiting)}`;
}

function railStyle(window: DisplayWindow | null): CSSProperties {
  return { "--s4-cadence-ratio": window?.remainingPercent === null || window?.remainingPercent === undefined ? 0 : window.remainingPercent / 100 } as CSSProperties;
}

function CadenceRail({ window, freshness, compact = false }: { window: DisplayWindow | null; freshness: QuotaViewModel["freshness"]; compact?: boolean }) {
  if (!window || window.remainingPercent === null) return null;
  return (
    <div className={`s4-cadence${compact ? " s4-cadence--compact" : ""}`} data-freshness={freshness} style={railStyle(window)} aria-label={`${windowLabel(window)} ${window.remainingPercent}% remaining`}>
      <div className="s4-cadence__track"><span className="s4-cadence__fill" /></div>
      {!compact && <div className="s4-cadence__labels"><span>Now</span><span>Reset · {formatResetDate(window.resetsAtMs)}</span></div>}
    </div>
  );
}

function OperationalRow({ model, onRefresh }: { model: QuotaViewModel; onRefresh: () => void }) {
  const message = model.providerMessage;
  return (
    <div className={`s4-operational-row${message ? " s4-operational-row--attention" : ""}`} role={message ? "status" : undefined}>
      <div className="s4-operational-copy">
        <span className="s4-operational-mark" aria-hidden="true" />
        <span>{message ?? "Codex · Current"}</span>
        {message === null && <span className="s4-operational-muted">Updated just now</span>}
      </div>
      {model.providerAction === "retry" || model.freshness === "refreshing" ? (
        <button className="s4-action s4-action--secondary s4-action--small s4-focus-target" type="button" onClick={onRefresh}>{model.freshness === "refreshing" ? "Refreshing" : "Retry"}</button>
      ) : <span className="s4-operational-muted">{model.ageSeconds === null ? "Provider state" : `Last reconciled ${new Date(Date.now() - model.ageSeconds * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}`}</span>}
    </div>
  );
}

function EmptyState({ model, compact, onRefresh }: { model: QuotaViewModel; compact: boolean; onRefresh: () => void }) {
  const title = model.freshness === "auth" ? "Sign in to Codex" : model.freshness === "unsupported" ? "Quota is not supported" : model.freshness === "unavailable" ? "Codex is unavailable" : "Connecting to Codex";
  const description = model.freshness === "auth" ? "Quota appears after authentication." : model.freshness === "unsupported" ? "This installation does not expose rate limits." : model.freshness === "unavailable" ? "The local provider could not be reached." : "Waiting for the first valid snapshot.";
  return (
    <div className={`s4-empty-state${compact ? " s4-empty-state--compact" : ""}`} role={model.freshness === "starting" ? "status" : "alert"}>
      <span className="s4-empty-state__title">{title}</span>
      <span className="s4-empty-state__description">{description}</span>
      {(model.freshness === "auth" || model.freshness === "unavailable") && <button className="s4-action s4-action--secondary s4-action--small s4-focus-target" type="button" onClick={onRefresh}>{model.freshness === "auth" ? "Retry after sign-in" : "Retry"}</button>}
    </div>
  );
}

function PrimaryValue({ window, nowMs }: { window: DisplayWindow | null; nowMs: number }) {
  const countdown = formatCountdown(window?.resetsAtMs ?? null, nowMs);
  return (
    <div className="s4-primary-values">
      <div className="s4-primary-reading">
        <span className="s4-number s4-number--display">{window?.remainingPercent ?? "—"}{window?.remainingPercent !== null && window?.remainingPercent !== undefined && <span className="s4-number-unit">%</span>}</span>
        <span className="s4-value-caption">Remaining</span>
      </div>
      <div className="s4-reset-reading">
        <span className="s4-number s4-number--primary">{countdown ?? "—"}</span>
        <span className="s4-value-caption">{countdown ? `until reset · ${formatResetDate(window?.resetsAtMs ?? null) ?? "time unknown"}` : "Reset time unavailable"}</span>
      </div>
    </div>
  );
}

function MainSurface({ model, nowMs, onRefresh, onSwitch, onWindowAction }: SurfaceProps) {
  const primaryAtLimit = isAtLimit(model.primary);
  return (
    <main className="s4-root s4-surface s4-main-surface" data-freshness={model.freshness} data-limit={primaryAtLimit ? "true" : "false"}>
      <WindowChrome mode="main" onSwitch={onSwitch} onWindowAction={onWindowAction} />
      <div className="s4-main-content">
        <div className="s4-main-heading-row"><h1 className="s4-title">Capacity</h1><button className="s4-icon-action s4-focus-target" type="button" aria-label="Refresh quota" onClick={onRefresh}><RefreshIcon /></button></div>
        <section className={`s4-main-primary${primaryAtLimit ? " s4-main-primary--critical" : ""}`} aria-labelledby="five-hour-heading">
          <div className="s4-band-heading"><h2 id="five-hour-heading" className="s4-label">5-hour capacity</h2><span className="s4-limiting-copy"><span className="s4-limiting-dot" aria-hidden="true" />{limitingCopy(model, model.primary)}</span></div>
          {model.snapshot ? <><PrimaryValue window={model.primary} nowMs={nowMs} /><CadenceRail window={model.primary} freshness={model.freshness} /></> : <EmptyState model={model} compact={false} onRefresh={onRefresh} />}
        </section>
        <section className="s4-weekly-band" aria-labelledby="weekly-heading">
          <div><h2 id="weekly-heading" className="s4-label">Weekly capacity</h2>{model.weekly ? <div className="s4-weekly-value-row"><span className="s4-number s4-number--primary">{model.weekly.remainingPercent ?? "—"}{model.weekly.remainingPercent !== null && model.weekly.remainingPercent !== undefined && <span className="s4-number-unit">%</span>}</span><span className="s4-value-caption">remaining</span></div> : <p className="s4-muted-copy">Weekly quota unavailable</p>}</div>
          <div className="s4-weekly-reset">{model.weekly ? <><span className="s4-number s4-number--compact">{formatCountdown(model.weekly.resetsAtMs, nowMs) ?? "—"}</span><span className="s4-value-caption">resets {formatResetDate(model.weekly.resetsAtMs) ?? "at an unknown time"}</span></> : null}</div>
        </section>
        <OperationalRow model={model} onRefresh={onRefresh} />
        <div className="s4-main-footnote"><span>{model.limiting ? `${windowLabel(model.limiting)} is the closer capacity boundary.` : "The limiting window is not available from the current provider snapshot."}</span><span>{model.snapshot ? `Snapshot ${new Date(model.snapshot.fetchedAt * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}` : "No snapshot"}</span></div>
      </div>
    </main>
  );
}

function CompactSurface({ model, nowMs, onRefresh, onSwitch, onWindowAction }: SurfaceProps) {
  const primaryAtLimit = isAtLimit(model.primary);
  const hasSnapshot = model.snapshot !== null && model.primary !== null;
  return (
    <main className="s4-root s4-surface s4-compact-surface" data-freshness={model.freshness} data-limit={primaryAtLimit ? "true" : "false"}>
      <WindowChrome mode="compact" onSwitch={onSwitch} onWindowAction={onWindowAction} />
      {hasSnapshot ? <div className="s4-compact-content"><div className="s4-compact-values"><span className="s4-number s4-number--compact">{model.primary!.remainingPercent ?? "—"}{model.primary!.remainingPercent !== null && model.primary!.remainingPercent !== undefined && <span className="s4-number-unit">%</span>}</span><div className="s4-compact-reset"><span className="s4-number">{formatCountdown(model.primary!.resetsAtMs, nowMs) ?? "—"}</span><span className="s4-compact-caption">until reset</span></div></div><CadenceRail window={model.primary} freshness={model.freshness} compact />{model.providerMessage && <span className="s4-compact-state" role="status">{model.providerMessage}</span>}</div> : <EmptyState model={model} compact onRefresh={onRefresh} />}
    </main>
  );
}

export function QuotaSurfaces(props: SurfaceProps & { mode: "main" | "compact" }): ReactNode {
  return props.mode === "compact" ? <CompactSurface {...props} /> : <MainSurface {...props} />;
}
