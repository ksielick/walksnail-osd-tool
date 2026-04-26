<p align="center">
<img width="256" height="176" src="https://user-images.githubusercontent.com/880421/224411816-c0cf1331-c856-42e9-a3d6-1c23b7da7886.png">
</p>
<h1 align="center">Walksnail OSD Tool</h1>

[![Latest release](https://img.shields.io/github/v/release/ksielick/walksnail-osd-tool?include_prereleases&label=latest%20release)](https://github.com/ksielick/walksnail-osd-tool/releases)
[![Latest build](https://img.shields.io/github/last-commit/ksielick/walksnail-osd-tool/master?label=latest%20build)](https://nightly.link/ksielick/walksnail-osd-tool/workflows/release.yaml/master/walksnail-osd-tool-all-platforms.zip)
[![Totally awesome](https://img.shields.io/badge/totally%20awesome-true-blue)](https://github.com/ksielick/walksnail-osd-tool)

Cross-platform tool for rendering the flight controller OSD and SRT data from the Walksnail Avatar, Ascent, and BetaFPV Artlynk HD FPV systems from goggle or VRX recording. Supports both standard `.osd` files and direct extraction from MP4 recordings with embedded OSD data (Artlynk). Extracted data is cached to `.osd` files for instant reloading.

![image](https://user-images.githubusercontent.com/880421/228286034-ffd7bf0d-4bb0-4774-9ee1-dd408bd97a88.png)


## Features
- Easy to use graphical user interface.
- Native installer for Windows, App bundle for MacOS.
- Hardware-accelerated encoding powered by ffmpeg.
- Choose between H.264 and H.265 codecs (more can be added later).
- View basic information about the video, OSD, SRT and font files.
- Preview OSD frames before rendering.
- Automatically center the OSD or position it manually.
- Render selected info from the SRT file.
- Selectable output video bitrate (more encoder settings will be added later).
- Upscale output video to 1440p or 2160p for higher quality when uploading to YouTube.
- Batch processing mode to automatically render all videos in a folder.
- Mask OSD items ([demo](https://imgur.com/u8xi2tX)).

Anything else? Open a feature request [here](https://github.com/ksielick/walksnail-osd-tool/issues/new?assignees=&labels=enhancement&template=feature_request.yaml).

## Batch Processing
The tool supports an automatic batch processing mode. When enabled, the application will:
1. Load and render the currently selected video.
2. Once the render is complete, it will automatically look for the next `.mp4` file in the same folder.
3. Automatically load matching OSD/SRT files and start the next render.
   * **New in v0.5.2:** Support for Walksnail Firmware 2.0.5 OSD extraction. Automatic OSD data caching to `.osd` files. Expanded SRT debug telemetry (SSNR, GSNR, Gerr, Serr, etc.).
4. Skip files that have already been processed (ending in `_with_osd.mp4`).

This is ideal for processing entire flight sessions without manual intervention.

## Installation

### Windows
Download and run the installer from the [latest release](https://github.com/ksielick/walksnail-osd-tool/releases).

> [!NOTE]
> If Windows Defender blocks the installer, click **"More info"** and then **"Run anyway"**.

### MacOS
Download the app bundle for your processor architecture from the [latest release](https://github.com/ksielick/walksnail-osd-tool/releases) and drag it to your Applications folder.

> [!IMPORTANT]
> If the app is marked as "damaged" or from an "unverified developer", please see the [Troubleshooting Guide](TROUBLESHOOTING.md).

### Linux
The project builds on Ubuntu in CI but I don't know enough about packaging for Linux to make release binaries. For now you need to build from source.

### Building from source
1. Install the [Rust toolchain](https://www.rust-lang.org/tools/install).
2. Run `cargo install --git https://github.com/ksielick/walksnail-osd-tool.git`. The executable will be installed in `$HOME/.cargo/bin/` and added to your path.
3. Make `ffmpeg` and `ffprobe` available — see [Installing ffmpeg](#installing-ffmpeg) below.
4. Run the app with `walksnail-osd-tool`.

#### Installing ffmpeg

The app shells out to `ffmpeg` and `ffprobe` at runtime. Released `.app` bundles and the Windows installer ship both binaries. When you run from source (`cargo run` or the binary built by `cargo install`), you need to provide them yourself — either on your `PATH`, or placed next to the executable.

**macOS** — install via [Homebrew](https://brew.sh):

```sh
brew install ffmpeg
```

This puts `ffmpeg` and `ffprobe` on your `PATH`. Confirm with `which ffmpeg ffprobe`.

Alternatively, the repo ships prebuilt binaries under `ext/ffmpeg/macos-arm/` and `ext/ffmpeg/macos-intel/`. Unzip the two archives matching your CPU and either move them to a directory on your `PATH` or put them next to the `walksnail-osd-tool` executable. On first run macOS will quarantine them — clear with `xattr -d com.apple.quarantine /path/to/ffmpeg /path/to/ffprobe`.

**Linux** — install via your package manager, e.g. on Debian/Ubuntu:

```sh
sudo apt install ffmpeg
```

This installs both `ffmpeg` and `ffprobe`. Other distros: `dnf install ffmpeg` (Fedora), `pacman -S ffmpeg` (Arch).

**Windows** — install via [Chocolatey](https://chocolatey.org/):

```powershell
choco install ffmpeg
```

Or [winget](https://learn.microsoft.com/en-us/windows/package-manager/winget/):

```powershell
winget install Gyan.FFmpeg
```

Either route puts both binaries on your `PATH`. Restart your terminal afterwards so the new `PATH` takes effect.

To verify the app can find them, the startup log line `ffmpeg_available ... return: true` appears in the terminal you launched the app from. If you still see the "ffmpeg and/or ffprobe could not be found" popup, run `which ffmpeg ffprobe` (macOS/Linux) or `where ffmpeg ffprobe` (Windows) — both must resolve before launching the app.

### Similar projects
- [kirek007/ws-osd-py](https://github.com/kirek007/ws-osd-py): Python-based tool with GUI and CLI. No longer maintained in favor of this project but has a few features that this project currently lacks. Depending on your OS it can require some manual setup due to Python dependencies.
- [shellixyz/hd_fpv_video_tool](https://github.com/shellixyz/hd_fpv_video_tool): Rust-based CLI tool with support for with Walksnail and DJI. Mainly targets Linux and can be difficult to build from source on Windows and MacOS. Has some cool features like live playback of the DVR with OSD without rendering.
- https://gist.github.com/henkwiedig/f52444800a6f2323e6de7e69031772fb: Used this script to extract OSD data 

## Disclaimer
This project is not affiliated with Walksnail/Caddx.
