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

## Resident lifecycle

The native tray owns `Open S4Quota`, `Compact Mode`, `Refresh`, and
`Quit S4Quota`. Main/Compact selection is stored separately from surface
visibility, so closing either surface enters `TrayOnly` without changing the
selected mode or stopping the provider. `Open S4Quota` explicitly selects and
shows Main; `Compact Mode` explicitly selects and shows Compact. Only the tray
Quit action requests graceful ProviderManager shutdown before the app exits.
Operating-system session exit is not intercepted as a close-to-tray action;
Windows Job Object kill-on-close remains the final child-process safeguard.

## Window preferences

Native window preferences are owned and written by Rust/Tauri, independently
of React rendering. The versioned JSON file is
`app_config_dir()/window-state.json` (on Windows, under the current user's
application configuration directory). It contains only `version`,
`lastVisibleMode`, and separate Main and Compact placement records; it never
contains provider state, quota values, account identity, or credentials.

`lastVisibleMode` means the last selected visible surface (`main` or
`compact`), not the runtime visibility state. `TrayOnly` is session-only and
is not serialized. Startup restores the saved visible mode even if the prior
session ended from TrayOnly or while minimized. A single-instance activation
continues to select Main without reapplying startup geometry.

Main position offsets and client dimensions are stored in logical pixels
relative to the source monitor's work-area origin. Compact stores only its
position; its size remains the canonical 304 × 120 logical pixels. Tauri
reports native positions and sizes in physical pixels, so capture divides
offsets and dimensions by that monitor's scale factor. Restore multiplies
logical values by the selected monitor's current scale factor before applying
physical bounds. The document also records a monitor fingerprint (name when
available, display/work-area dimensions and origin, and scale factor), but no
single identifier is trusted by itself.

Restore first matches the monitor fingerprint, then chooses the available
work area with the largest intersection against the old placement, then the
primary monitor (or first valid monitor). Bounds are clamped to the selected
work area; if it is smaller than Main's minimum size, the window's top-left
titlebar origin is kept accessible. Invalid/corrupt bounds are ignored, and an
unsupported future schema is not overwritten. Main's normal bounds are saved
separately from its maximized flag; maximizing never replaces those normal
bounds. Minimized bounds are not saved.

Move/resize/scale events schedule a debounced capture and disk write, avoiding
transient bounds emitted while maximizing. The custom maximize action also
captures normal bounds before requesting native maximization. Mode changes
schedule a save, while graceful Quit takes a final capture and attempts an
immediate write. Persistence errors are diagnostic only and do not block
shutdown.

The official `tauri-plugin-window-state` was evaluated for this role. Its
global state flags and physical-pixel window records would need additional
per-window handling for fixed-size Compact and explicit scale conversion for
mixed-DPI restoration. S4Quota keeps its dedicated versioned preferences module
for those policies; it does not persist `VISIBLE`, minimized, or TrayOnly.
