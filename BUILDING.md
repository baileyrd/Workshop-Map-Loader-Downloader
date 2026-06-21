# Building & Running (Rust app)

The standalone Rust rewrite lives in a Cargo workspace:

- `crates/wml-core` — game-independent logic (library, catalog, downloads,
  install, config, paths, BakkesMod RCON bridge)
- `crates/wml-app` — the egui/eframe desktop application (binary
  `workshop-map-loader`)

See `RUST_CONVERSION.md` for the design and rationale.

## Prerequisites

- Rust (stable) — install via <https://rustup.rs>.
- Linux only: the GUI runtime needs X11/Wayland libraries (already present on a
  normal desktop). Building does not require GTK — the file dialog uses the XDG
  desktop portal backend.

## Build & run

```sh
# Build everything
cargo build --workspace

# Run the app
cargo run -p wml-app

# Run the core tests
cargo test -p wml-core

# Optimized build (the binary is at target/release/workshop-map-loader)
cargo build -p wml-app --release
```

## Optional features

### Gamepad support (`controller`)

D-pad navigation of the library and launching with the South (A) button.

```sh
cargo run -p wml-app --features controller
```

This pulls [`gilrs`]. On **Linux** it links `libudev` at build time, so install
the dev package first (e.g. `sudo apt install libudev-dev`). On **Windows** it
uses XInput/RawInput with no extra dependency. The feature is **off by default**
so the standard build stays portable and CI-friendly.

[`gilrs`]: https://crates.io/crates/gilrs

## Configuration

Config is stored as TOML in the OS config directory (resolved via the
`directories` crate), e.g. `~/.config/WorkshopMapLoader/config.toml` on Linux or
`%APPDATA%\WorkshopMapLoader\config.toml` on Windows.

On first run, if no config exists, the app attempts to migrate the plugin's
legacy `workshopmaploader.cfg` (Windows, from BakkesMod's data folder).

## Launching maps (BakkesMod bridge)

In-app "Play" sends `load_workshop` to BakkesMod over its RCON WebSocket
(default `127.0.0.1:9876`). Enable RCON in BakkesMod and set the matching
password in the app config. When BakkesMod isn't reachable, the app runs in
manager-only mode (download/organize maps; use "Open Folder" and load in-game).

## CI

`.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`,
`cargo test`, and a release build of the app on Linux and Windows. A separate
`controller` job builds and clippy-checks the gamepad feature on Linux (after
installing `libudev-dev`) and Windows.
