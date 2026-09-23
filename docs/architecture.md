# Architecture

S4Quota has a provider-neutral quota domain and a Codex-specific Rust adapter.
The current implementation keeps the provider process and protocol behind the
Tauri boundary.

```text
Codex App Server
    │ stdio JSON-RPC
    ▼
CodexProvider ── normalizes provider responses
    ▼
ProviderManager ── owns start, refresh, retry, poll, and shutdown
    ▼
canonical ProviderState ── watch publication
    ▼
Tauri IPC commands / provider-state event
    ▼
React + TypeScript ── Main and Compact presentation
```

## Responsibilities

- **Domain (`src-tauri/src/domain.rs`)** defines quota windows and the provider
  lifecycle state machine without Codex protocol types.
- **Provider Manager (`provider_manager.rs`)** runs as an async task. It accepts
  `Start`, `Refresh`, and `Shutdown` through `mpsc` and publishes the canonical
  state through `watch`.
- **Codex adapter (`codex_provider.rs`, `json_rpc.rs`)** discovers Codex,
  probes required capabilities, performs account and rate-limit reads, and
  normalizes the results. Windows process containment is in `windows_job.rs`.
- **Tauri (`src-tauri/src/lib.rs`)** exposes typed snapshot/settings/window
  operations and provider events. Capabilities stay narrow; there is no
  frontend shell or arbitrary process command.
- **Frontend (`src/lib/`, `src/components/`)** converts published DTOs into
  display-ready values, computes countdowns locally, and renders the two
  surfaces. It does not own process lifecycle or provider protocol.

`AppSnapshot.quota` is derived from its canonical provider state when sent to
the frontend. The Rust state machine remains the authority for lifecycle and
snapshot retention.

## Code map

| Path | Purpose |
| --- | --- |
| `src-tauri/src/domain.rs` | Provider-neutral quota and state machine |
| `src-tauri/src/provider_manager.rs` | Async lifecycle, polling, and retry |
| `src-tauri/src/codex_provider.rs` | Codex discovery, protocol adapter, normalization |
| `src-tauri/src/json_rpc.rs` | Stdio framing, request correlation, process supervision |
| `src-tauri/src/windows_job.rs` | Windows Job Object process-tree cleanup |
| `src/lib/contracts.ts` | Frontend DTO contract |
| `src/lib/quota.ts` | View model and local countdown calculation |
| `src/components/QuotaSurfaces.tsx` | Main and Compact composition |
| `src/design-system/` | Shared visual tokens and primitives |
| `spike/codex-app-server/` | Isolated integration spike and sanitized report |

The current product requirements and visual rules remain in
[PRODUCT.md](../PRODUCT.md) and [DESIGN.md](../DESIGN.md); the approved
surface composition is recorded in
[`.impeccable/surfaces/main-compact.md`](../.impeccable/surfaces/main-compact.md).
