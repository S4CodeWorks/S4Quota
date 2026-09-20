import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import "./App.css";
import type { AppSnapshot, ProviderState, QuotaSnapshot } from "./lib/contracts";

function quotaFromState(state: ProviderState): QuotaSnapshot | null {
  if (state.status === "ready") return state.snapshot;
  if (state.status === "refreshing") return state.previousSnapshot;
  if (state.status === "degraded") return state.snapshot;
  return null;
}

function App() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let cancelled = false;
    let stopListening: UnlistenFn | undefined;

    async function connectTechnicalProbe() {
      try {
        stopListening = await listen<ProviderState>("provider-state", ({ payload }) => {
          setSnapshot((current) => current && {
            ...current,
            provider: payload,
            quota: quotaFromState(payload),
          });
        });
        const current = await invoke<AppSnapshot>("get_app_snapshot");
        if (!cancelled) setSnapshot(current);
      } catch (reason) {
        if (!cancelled) setError(String(reason));
      }
    }

    void connectTechnicalProbe();
    return () => {
      cancelled = true;
      stopListening?.();
    };
  }, []);

  async function requestRefresh() {
    setError(null);
    try {
      await invoke("refresh_provider");
    } catch (reason) {
      setError(String(reason));
    }
  }

  return (
    <main className="container">
      <h1>S4Quota technical foundation</h1>
      <p>Temporary CodexProvider contract probe — no product surface implemented.</p>
      <button type="button" onClick={requestRefresh}>Refresh provider</button>
      {error && <p role="alert">IPC error: {error}</p>}
      {snapshot && <pre>{JSON.stringify(snapshot, null, 2)}</pre>}
    </main>
  );
}

export default App;
