# S4Quota

**S4CodeWorks · Windows desktop utility · Development preview**

S4Quota keeps an individual’s available capacity across AI tools and agents in
view while they work. The current provider is Codex, with live five-hour and
weekly quota windows, remaining percentages, reset countdowns, and provider
freshness and status.

The product uses a provider-agnostic quota domain so additional tools can be
added later without making Codex-specific protocol details part of the UI.
Product and visual direction live in [PRODUCT.md](PRODUCT.md) and
[DESIGN.md](DESIGN.md).

## Screenshots

Real application screenshots are not versioned yet. The repository is ready
for Main Dark, Main Light, Compact Dark, and Compact Light captures; see
[`docs/screenshots/`](docs/screenshots/README.md). Impeccable comps under
[`.impeccable/`](.impeccable/surfaces/main-compact.md) are composition records,
not screenshots of the running application.

## What is implemented

- Codex App Server integration over stdio JSON-RPC, using Codex’s existing sign-in.
- Real five-hour and weekly quota data, with local reset countdowns.
- Main and Compact desktop surfaces, including OS-aware Light and Dark themes.
- Provider lifecycle and freshness states, manual refresh, and backend
  reconciliation polling.
- Windows process supervision with Job Object cleanup.
- Windows MSI and NSIS bundle targets.

Tray integration, autostart, final geometry persistence, release hardening, and
additional providers remain planned.

## Architecture

```text
Codex
  → Codex App Server (stdio / JSON-RPC)
  → CodexProvider (Rust adapter)
  → ProviderManager (async task)
  → canonical ProviderState
  → narrow Tauri commands and state events
  → React Main / Compact
```

React and TypeScript present the published domain state and request actions such
as refresh or surface switching. Rust owns the quota domain, provider lifecycle,
process discovery and supervision, and normalization. Tauri connects the two
and manages native desktop windows. See [architecture](docs/architecture.md) and
[provider integration](docs/provider.md).

## Security

S4Quota relies on the user’s existing Codex sign-in through the App Server. It
does not read `auth.json` or authentication caches, persist credentials, or
call private ChatGPT HTTP endpoints. Provider diagnostics are bounded and
sanitized. The frontend cannot launch arbitrary processes.

## Development

Requirements and setup details are in [docs/development.md](docs/development.md).

```powershell
npm ci
npm run dev
npm run test:frontend
npm run build
npm run tauri -- dev
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

To run the sanitized live Codex validation, use the opt-in probe documented in
[`docs/provider.md`](docs/provider.md). It requires Codex to be installed and
signed in on the test machine.

## Windows builds

```powershell
npm run tauri -- build
```

Tauri writes Windows bundles under
`src-tauri/target/release/bundle/`: NSIS installers go to `nsis/`, and MSI
installers go to `msi/`. Build output and installers are excluded from Git.
