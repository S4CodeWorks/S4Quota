# Development

S4Quota currently targets Windows with Tauri 2, React, TypeScript, and Rust.
Install Node.js 22.6 or newer (the frontend test command uses Node’s type
stripping flag), the Rust stable MSVC toolchain, Visual Studio C++ build tools,
and the WebView2 Runtime. The [Tauri Windows prerequisites](https://v2.tauri.app/start/prerequisites/)
cover the required C++ tools and WebView2; MSI packaging may also require the
Windows VBSCRIPT optional feature. Node added the test command’s
`--experimental-strip-types` flag in 22.6.0; see the
[Node.js TypeScript documentation](https://nodejs.org/api/typescript.html).

From the repository root:

```powershell
npm ci
npm run dev
```

The Vite server supports frontend-only work. To run the desktop application
with its Rust backend:

```powershell
npm run tauri -- dev
```

## Checks

```powershell
npm run test:frontend
npm run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
```

The live Codex provider probe is described in [provider.md](provider.md). It is
opt-in because it starts the local App Server and uses the machine’s existing
Codex sign-in.

## Release bundles

```powershell
npm run tauri -- build
```

Windows MSI and NSIS output is placed in
`src-tauri/target/release/bundle/msi/` and
`src-tauri/target/release/bundle/nsis/`. These generated files are not tracked.
