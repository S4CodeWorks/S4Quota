import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";
import { QuotaSurfaces } from "./components/QuotaSurfaces";
import type { AppSnapshot, ProviderState } from "./lib/contracts";
import { buildQuotaViewModel, snapshotFromProvider } from "./lib/quota";

type WindowAction = "minimize" | "toggleMaximize" | "close";

function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [windowLabel, setWindowLabel] = useState<"main" | "compact">("main");
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    let stopListening: UnlistenFn | undefined;

    async function connect() {
      try {
        const [label, current] = await Promise.all([
          invoke<string>("get_window_label"),
          invoke<AppSnapshot>("get_app_snapshot"),
        ]);
        if (cancelled) return;
        setWindowLabel(label === "compact" ? "compact" : "main");
        setSnapshot(current);
        stopListening = await listen<ProviderState>("provider-state", ({ payload }) => {
          setSnapshot((previous) => previous ? {
            ...previous,
            provider: payload,
            quota: snapshotFromProvider(payload, previous.quota),
          } : previous);
        });
      } catch {
        if (!cancelled) setError("S4Quota could not connect to its provider manager.");
      }
    }

    void connect();
    return () => {
      cancelled = true;
      stopListening?.();
    };
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);

  const model = useMemo(() => snapshot ? buildQuotaViewModel(snapshot, nowMs) : null, [snapshot, nowMs]);

  async function refreshProvider() {
    setError(null);
    try {
      await invoke("refresh_provider");
    } catch {
      setError("Refresh could not be requested.");
    }
  }

  async function switchSurface() {
    setError(null);
    try {
      const mode = windowLabel === "main" ? "compact" : "main";
      await invoke("set_presentation_mode", { mode });
    } catch {
      setError("The surface could not be switched.");
    }
  }

  async function windowAction(action: WindowAction) {
    setError(null);
    try {
      await invoke("window_action", { action });
    } catch {
      setError("The window action could not be completed.");
    }
  }

  if (!model) {
    return <main className={`s4-root s4-surface s4-loading-shell s4-loading-shell--${windowLabel}`}><div className="s4-loading-mark" aria-hidden="true" /><span>Connecting to Codex</span>{error && <span className="s4-inline-error" role="alert">{error}</span>}</main>;
  }

  return <>
    <QuotaSurfaces mode={windowLabel} model={model} nowMs={nowMs} onRefresh={refreshProvider} onSwitch={switchSurface} onWindowAction={windowAction} />
    {error && <div className="s4-inline-error s4-inline-error--floating" role="alert">{error}</div>}
  </>;
}

export default App;
