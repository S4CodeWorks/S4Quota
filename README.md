<table align="center" border="0">
  <tr><td align="center" bgcolor="#F4F4F0"><img src="assets/branding/s4quota-mark-primary.png" width="112" alt="S4Quota mark" /></td></tr>
</table>

<h1 align="center">S4Quota</h1>

<p align="center"><strong>Personal AI capacity, at a glance.</strong></p>

<p align="center">A calm Windows desktop utility for seeing what AI capacity remains and when it resets.<br />
By S4CodeWorks · Development preview</p>

<p align="center">
  <a href="#download-for-windows"><img alt="Download for Windows — coming soon" src="https://img.shields.io/badge/Download%20for%20Windows-coming%20soon-454640?style=flat-square&logo=windows&logoColor=white" /></a>
  <a href="https://github.com/S4CodeWorks/S4Quota/releases"><img alt="Releases" src="https://img.shields.io/badge/Releases-view-77796F?style=flat-square" /></a>
  <a href="docs/README.md"><img alt="Docs" src="https://img.shields.io/badge/Docs-read-77796F?style=flat-square" /></a>
  <a href="docs/architecture.md"><img alt="Architecture" src="https://img.shields.io/badge/Architecture-view-77796F?style=flat-square" /></a>
</p>

<p align="center">
  <img alt="Windows" src="https://img.shields.io/badge/platform-Windows-454640?style=flat-square&logo=windows&logoColor=white" />
  <img alt="Tauri 2" src="https://img.shields.io/badge/Tauri-2-454640?style=flat-square" />
  <img alt="Rust" src="https://img.shields.io/badge/backend-Rust-454640?style=flat-square&logo=rust&logoColor=white" />
  <img alt="React and TypeScript" src="https://img.shields.io/badge/frontend-React%20%2B%20TypeScript-454640?style=flat-square" />
</p>

## Download for Windows

<table>
  <tr>
    <td>
      <strong>Windows desktop · x64 · MSI / NSIS</strong><br />
      The first public installer has not been published yet. When available, it will be attached to a GitHub Release.
    </td>
    <td align="right">
      <a href="https://github.com/S4CodeWorks/S4Quota/releases"><strong>Check releases</strong></a>
    </td>
  </tr>
</table>

You can build the current preview locally with the steps in
[Development](#development).

## Screenshots

Real application screenshots are not versioned yet. The capture slots are
prepared for Main Dark, Main Light, Compact Dark, and Compact Light in
[`docs/screenshots/`](docs/screenshots/README.md). The approved Impeccable comps
are design references, not screenshots of the running application.

## Features

- Live Codex five-hour and weekly quota windows.
- Remaining capacity, reset countdowns, and provider freshness/state.
- Main and Compact desktop surfaces with Light and Dark themes.
- Manual refresh and 60-second backend reconciliation.
- Windows process supervision and cleanup for the Codex App Server.

## What is S4Quota?

S4Quota helps people who use AI agents and tools intensively understand their
personal working capacity throughout the day and week: how much remains, when
it returns, which limit is closer, and whether the displayed data is current.

Codex is the first provider. The quota domain is provider-agnostic so other
tools can be added later without coupling the product language or UI to Codex.

## Architecture

```text
Codex
  → Codex App Server (stdio / JSON-RPC)
  → CodexProvider (Rust)
  → ProviderManager (async task)
  → canonical ProviderState
  → Tauri IPC commands and state events
  → React / TypeScript Main and Compact surfaces
```

React and TypeScript render the published state and request narrow actions.
Rust owns the domain, provider lifecycle, quota normalization, and process
supervision. Tauri connects them and manages native windows. More detail is in
[docs/architecture.md](docs/architecture.md).

## Security

S4Quota uses the existing Codex sign-in through the local App Server. It does
not read `auth.json` or authentication caches, store credentials, or call
private ChatGPT HTTP endpoints. Diagnostics are bounded and sanitized, and the
frontend cannot launch arbitrary processes.

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

The live Codex provider probe is opt-in; see [docs/provider.md](docs/provider.md).

Windows MSI and NSIS bundles are generated under
`src-tauri/target/release/bundle/msi/` and
`src-tauri/target/release/bundle/nsis/` by:

```powershell
npm run tauri -- build
```

## Project Status

**Implemented:** Codex provider, real five-hour and weekly quota data, Main and
Compact surfaces, Light and Dark themes, Windows MSI/NSIS bundle targets.

**Planned:** tray integration, autostart, final geometry persistence, release
hardening, and additional providers.

## Documentation

- [Product definition](PRODUCT.md)
- [Design System](DESIGN.md)
- [Architecture](docs/architecture.md)
- [Codex provider and security](docs/provider.md)
- [Development and Windows builds](docs/development.md)
- [Approved Main and Compact composition](.impeccable/surfaces/main-compact.md)
- [Original provider spike and sanitized report](spike/codex-app-server/README.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and architectural
boundaries.

## License

No license has been selected by S4CodeWorks yet; the repository currently has
no `LICENSE` file.
