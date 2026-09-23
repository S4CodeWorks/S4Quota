<div align="center">
  <p><img src="docs/assets/s4quota-readme-mark-tile.svg" width="112" alt="S4Quota mark" /></p>

  <h1>S4Quota</h1>
  <p><strong>Personal AI capacity, at a glance.</strong></p>
  <p>A calm Windows desktop utility for tracking Codex capacity, reset times, and quota state without leaving your workflow.</p>
  <p><sub>By S4CodeWorks · Windows 10/11 · x64 · Development preview</sub></p>

  <p><a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-setup-x64.exe"><kbd>Download for Windows</kbd></a></p>
  <p><sub><a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-portable-x64.exe">Portable .exe</a> · <a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-x64.msi">MSI</a> · <a href="https://github.com/S4CodeWorks/S4Quota/releases">All releases</a></sub></p>
</div>

## Download for Windows

<table>
  <tr><th colspan="2" align="left">Windows 10/11 · x64</th></tr>
  <tr>
    <td>
      <strong>Installer</strong><br />
      Recommended · easiest setup<br /><br />
      <a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-setup-x64.exe"><strong>Download .exe</strong></a>
    </td>
    <td>
      <strong>Portable</strong><br />
      No installation required<br /><br />
      <a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-portable-x64.exe"><strong>Download portable .exe</strong></a>
    </td>
  </tr>
  <tr><td colspan="2">Prefer MSI? <a href="https://github.com/S4CodeWorks/S4Quota/releases/latest/download/S4Quota-x64.msi">Download the x64 MSI</a>.</td></tr>
</table>

## Screenshots

Real application screenshots coming shortly.

<!-- Screenshot slots: docs/screenshots/main-dark.png, docs/screenshots/main-light.png, docs/screenshots/compact-dark.png, docs/screenshots/compact-light.png -->

## Why S4Quota

S4Quota helps people who rely on AI agents understand their personal working capacity: what remains, when it resets, which window is more limiting, and whether the information is current.

- Live Codex five-hour and weekly quota windows.
- Remaining capacity, reset countdowns, and provider freshness/state.
- Main and Compact desktop surfaces with Light and Dark themes.
- Manual refresh and 60-second backend reconciliation.
- Windows process supervision for the Codex App Server.

## How it works

S4Quota connects to the locally installed Codex App Server and uses its existing sign-in. It reconciles quota data periodically and on request, then presents remaining capacity and reset times in two desktop surfaces. Codex is the first provider; the product domain is designed to accommodate other tools later.

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

React and TypeScript present published state and request narrow actions. Rust owns the domain, provider lifecycle, quota normalization, and process supervision. Tauri bridges the layers and manages native windows. See [Architecture](docs/architecture.md).

## Security

S4Quota reuses the existing Codex sign-in through the local App Server. It does not read `auth.json` or authentication caches, persist credentials, or call private ChatGPT HTTP endpoints. Diagnostics are bounded and sanitized; the frontend cannot launch arbitrary processes.

## Project status

**Implemented:** Codex provider and real five-hour/weekly quota data; Main and Compact surfaces; Light and Dark themes; manual refresh and polling; Windows MSI and NSIS installers plus a portable executable.

**Planned:** tray integration, autostart, final geometry persistence, release hardening, and additional providers.

## Development

Requirements and setup details are in [Development](docs/development.md).

```powershell
npm ci
npm run dev
npm run test:frontend
npm run build
npm run tauri -- dev
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

The live Codex provider probe is opt-in; see [Codex provider](docs/provider.md). Run `npm run tauri -- build` to generate the Windows bundles under `src-tauri/target/release/bundle/` (MSI and NSIS). Generated installers are not committed.

## Documentation

- [Product definition](PRODUCT.md)
- [Design System](DESIGN.md)
- [Architecture](docs/architecture.md)
- [Codex provider and security](docs/provider.md)
- [Development and Windows builds](docs/development.md)
- [Approved Main and Compact composition](.impeccable/surfaces/main-compact.md)
- [Provider spike and sanitized report](spike/codex-app-server/README.md)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for setup, checks, and architectural boundaries.

## License

No license has been selected by S4CodeWorks; the repository currently has no `LICENSE` file.
