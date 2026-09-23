# Contributing

S4Quota is an early Windows desktop utility. Start with
[the development guide](docs/development.md), then read
[PRODUCT.md](PRODUCT.md) and [DESIGN.md](DESIGN.md) before changing product
behavior or surfaces.

## Local checks

Before sending a change, run the checks relevant to it:

```powershell
npm run test:frontend
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

For Tauri or Windows packaging changes, also run `npm run tauri -- dev` or
`npm run tauri -- build` as appropriate and describe the Windows validation.

## Project boundaries

- Keep provider protocol and process management in Rust; frontend commands
  should remain narrow and must not execute arbitrary processes.
- Keep the shared quota domain provider-agnostic. Add provider-specific wire
  types inside that provider’s adapter.
- Do not read Codex credential files, tokens, or authentication caches, and do
  not call private ChatGPT endpoints. Use the current local App Server
  integration exercised by the provider tests.
- Do not persist raw account/quota payloads or unsanitized diagnostics.
- Update documentation when observable behavior, setup, or architecture changes.
