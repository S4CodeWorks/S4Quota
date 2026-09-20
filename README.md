# S4Quota

Windows desktop foundation for S4Quota, built with Tauri 2, React,
TypeScript, and Rust.

The current application contains the provider-agnostic quota domain, an
explicit provider state machine, and a production Rust `CodexProvider` backed
by `codex app-server` over stdio JSON-RPC. A mock remains available for tests
and fixtures. The React view is only a temporary technical contract probe; the
product surfaces and Design System are not implemented yet.

The backend owns discovery, process supervision, capability probes, 60-second
reconciliation polling, retry/backoff, and shutdown. The frontend can only
read the published provider state and request a refresh; it cannot choose or
execute a process. Authentication files and private endpoints are not read.

## Development

```powershell
npm install
npm run dev
npm run tauri dev
```

Rust tests run with:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml
```

The sanitized live-provider validation is opt-in:

```powershell
cargo test --manifest-path src-tauri/Cargo.toml live_provider_probe_is_sanitized -- --ignored --nocapture
```

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
