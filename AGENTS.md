# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Cross-platform desktop GUI tool that renders the flight controller OSD and SRT telemetry from Walksnail Avatar / Ascent / BetaFPV Artlynk HD FPV systems onto goggle/VRX video recordings. Reads `.osd` files (or extracts OSD data directly from Artlynk MP4s), composites OSD glyphs and selected SRT fields onto each frame, and pipes the result through `ffmpeg` for encoding.

## Workspace layout

Cargo workspace with two members (`Cargo.toml`):

- **`backend/`** — library crate (`backend`). Pure logic: parsing, rendering, ffmpeg orchestration. No UI deps. Submodules:
  - `osd/` — `.osd` file parsing (`osd_file.rs`), Artlynk MP4 OSD extraction (`artlynk.rs`), frame/glyph types, FC firmware detection.
  - `srt/` — SRT subtitle parsing and per-field options.
  - `font/` — font file loading (`font_file.rs`) and the font picker logic.
  - `overlay/` — composites OSD frames + SRT text into image frames (`osd.rs`, `srt.rs`, `iter.rs` drives the per-video-frame iteration).
  - `ffmpeg/` — `dependencies.rs` (locate ffmpeg/ffprobe), `video_info.rs` (ffprobe), `encoders.rs` (detect available H.264/H.265 encoders incl. hardware), `render.rs` (spawn ffmpeg, pump frames), `message.rs` (`ToFfmpegMessage`/`FromFfmpegMessage` channel types), `render_settings.rs`.
  - `config.rs` — `AppConfig` persisted via `confy` (RON format).
- **`ui/`** — binary crate `walksnail-osd-tool`. egui/eframe app. `app.rs` holds `WalksnailOsdTool` central state; `*_panel.rs` files are the egui panels; `osd_preview.rs` renders the preview texture; `render_status.rs` tracks the render lifecycle. Communicates with the backend's render thread via `crossbeam-channel`.

The UI never touches ffmpeg directly — it sends `ToFfmpegMessage` and reads `FromFfmpegMessage` from a channel pair owned by `backend::ffmpeg`.

## Commands

Workspace builds (run from repo root):

```sh
cargo check --all-targets --all-features
cargo build --release                          # release binary in target/release/walksnail-osd-tool
cargo run -p walksnail-osd-tool                # run the GUI
cargo test --all-targets --all-features
cargo test -p backend <test_name>              # single test
cargo clippy --all-targets --all-features -- -D warnings
cargo +nightly fmt --check                     # CI uses nightly fmt; rustfmt.toml has unstable opts
```

`rustfmt.toml` requires nightly (`imports_granularity`, `group_imports`).

CI (`.github/workflows/ci.yaml`) runs `cargo check` on Windows + macOS + Ubuntu, and `cargo test` / `clippy` / nightly `fmt --check` on Windows. Linux check needs `libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev libgtk-3-dev`.

## Runtime ffmpeg dependency

The app shells out to `ffmpeg` and `ffprobe` at runtime — they must be on `PATH` or sit next to the executable. `backend/src/ffmpeg/dependencies.rs` resolves them. Vendored binaries live in `ext/ffmpeg/<platform>/` (used by release packaging) and `dist/ffmpeg/` (Windows installer payload). These are large and tracked in git deliberately — see release workflow before changing.

## Cargo profile note

`Cargo.toml` sets `[profile.dev] opt-level = 3` — debug builds are optimized because frame-level image processing is unusably slow at `opt-level = 0`. Don't lower this.

## Build features

`ui/Cargo.toml` exposes two feature flags used only for packaging differences:
- `macos-app-bundle` — built by `cargo bundle` for the `.app`.
- `windows-installer` — built by `build_windows_artifact.ps1` / release workflow.

Neither changes runtime behavior; don't gate logic on them.

## Versioning

`backend` and `ui` (`walksnail-osd-tool`) versions are bumped together. Release notes go in `CHANGELOG.md`. The release workflow (`.github/workflows/release.yaml`) builds per-platform artifacts and drives the GitHub release.
