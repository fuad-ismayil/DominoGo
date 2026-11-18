# DominoGo

Multiplayer domino game rebuilt in Rust: an authoritative Axum server, SQLite persistence, and a Dioxus WebAssembly browser interface. The restored full-screen emerald table includes mouse/touch dragging, fitted snake geometry, dimensional ivory pieces and recorded tabletop foley. Menus use clear glass controls and responsive phone navigation.

## Run Locally

Prerequisites: stable Rust, the `wasm32-unknown-unknown` target, and Dioxus CLI 0.7.10. Windows GNU Rust requires MinGW tools on PATH. No Node/npm toolchain is required.

1. Build with `./build.ps1` (add `-Release` for an optimized build).
2. Start with `./run.ps1` (or `./run.ps1 -Release` after a release build).
3. Open `http://localhost:3000`

## Architecture

- `rust/crates/domino-core`: rules, legal moves, scoring, shuffling and bot strategy.
- `rust/crates/domino-server`: authenticated HTTP/realtime play, persistence and turn scheduling.
- `rust/crates/domino-client`: browser screens and standalone responsive CSS.

`DOMINO_PORT` changes the port (default 3000). `DOMINO_DB_PATH` selects the SQLite file. Use a separate database for verification. Public hosting requires an HTTPS reverse proxy with WebSocket support and regular database backups.

The application source is Rust and CSS. Browser output includes WebAssembly and generated JavaScript bindings required by Dioxus/wasm-bindgen to access browser APIs. These are build artifacts, not a separate JavaScript application.

## Verification

Run `cargo test --manifest-path rust/Cargo.toml --workspace` and `cargo check --manifest-path rust/Cargo.toml -p domino-client --target wasm32-unknown-unknown`.

With a test server running, `./verify.ps1` also checks sessions, lobby creation, starting a bot match, history and leaderboard over HTTP. Browser interaction and visual checks are separate from compilation and automated rules tests.

Run the full acceptance harness against an isolated server/database with `DOMINO_E2E_URL=http://127.0.0.1:3010 cargo run --manifest-path rust/Cargo.toml -p domino-e2e`. On PowerShell set `$env:DOMINO_E2E_URL` before running Cargo. The harness creates test accounts and plays real matches.

## Portable build

From `rust/crates/domino-client`, run `dx build --web --release`. From `rust`, run `cargo build -p domino-server --release`. Serve the generated web/public directory using `DOMINO_STATIC_DIR`.

## Android hosting

The `android-host` companion runs the same Rust server directly on an ARM64 Android phone, with Start/Stop, public-link sharing and a hosting notification. Build/install with `./android-host/build.ps1 -Install`. The paired phone's dedicated permanent address is `https://domino.driveatbaku.games`.

The PC database is copied privately to the phone, then synced over authorized USB or wireless ADB when both servers are stopped. Enable the PC helper with `./android-host/start-sync.ps1 -InstallStartup`; `-Stop` disables it. Newer match saves win, and per-account history hides sync without erasing other players' history. See [Android setup and recovery](android-host/README.md).
