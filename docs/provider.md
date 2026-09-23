# Codex provider

The production provider runs the installed Codex App Server as a child process
over piped stdin/stdout JSON-RPC. Its production argument list is
`codex app-server`; stdio is the transport. The adapter performs the App Server
handshake and a positive capability probe before reading account and rate-limit
data. The local authenticated Codex installation supplies authentication.

Codex discovery checks `where.exe codex` results and the observed Codex Desktop
installation location under `%LOCALAPPDATA%`. A native executable is preferred.
A trusted npm `.cmd` shim is a controlled fallback; S4Quota does not invoke an
arbitrary shell command supplied by the frontend. Version is collected for
diagnostics, while support is decided by capability probing.

## Data freshness

The manager reconciles every 60 seconds. It also refreshes after startup,
reconnect, resume, and an explicit user request. App Server update notifications
can suggest an earlier refresh, but polling remains the consistency mechanism.
Countdowns are derived in the frontend from the snapshot reset time; they do not
trigger per-second provider requests.

## Process and diagnostics

The JSON-RPC client reserves stdout for protocol frames, incrementally parses
partial and multiple frames, limits frame and stderr sizes, correlates request
IDs, and applies timeouts. Stderr is drained continuously and sanitized before
it appears in diagnostics. Failures distinguish spawn, handshake exit, EOF,
timeout, and invalid response. On Windows a Job Object with kill-on-close
contains the child process tree; shutdown requests graceful exit before forcing
termination when needed.

S4Quota does not read Codex authentication files or caches, persist credentials,
or use private ChatGPT HTTP endpoints. The App Server is treated as a local
integration surface whose methods are verified through capabilities; its
compatibility may change between Codex releases. The isolated spike’s tests,
observations, and original go-with-caveats conclusion remain in
[`spike/codex-app-server/`](../spike/codex-app-server/README.md).

## Validation

Rust unit and process-fixture tests run with:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

The live probe is intentionally opt-in. It prints only sanitized diagnostics
and requires an installed, signed-in Codex App Server:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml live_provider_probe_is_sanitized -- --ignored --nocapture
```
