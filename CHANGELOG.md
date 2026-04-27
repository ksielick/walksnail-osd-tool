# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.3] - 2026-04-27

### Added
- **Improved Firmware Detection**: The tool can now automatically identify the flight controller (Betaflight, INAV, ArduPilot) by scanning the OSD content if the file header is missing.
- **Corrupted File Alerts**: Added a red warning for empty or corrupted OSD files, specifically addressing issues seen in some VTX firmware versions (e.g., Avatar 39.44.15).
- **Unknown Firmware Indicator**: Added a red warning for "Unknown" firmware to highlight when manual font selection might be required.

### Fixed
- **Enhanced App Stability**: Fixed several issues that caused the application to crash when loading malformed, empty, or missing files.
- **Better Avatar Compatibility**: Improved support for loading and processing native Avatar OSD files.
- **Encoder Safety**: Fixed a crash that occurred if no video encoders were found on the computer.

## [0.5.2] - 2026-04-25

### Added
- **Artlynk Firmware 2.0.5 Support**: Implemented a robust, search-based OSD parser that handles the new variable-length command format in 2.0.5+ DVR files.
- **OSD Caching and Persistence**: Extracted OSD data is now automatically saved to a `.osd` file in the same directory as the video. These files are reloaded instantly on subsequent imports, eliminating the need for expensive re-scans.
- **Expanded Telemetry**: Added support for Ascent debug telemetry fields in SRT data (SSNR, GSNR, Stemp, Gtemp, Gerr, Serr).
- **Linux AppImage Support**: Added official support for Linux via AppImage builds, including integrated FFmpeg binaries (contributed by @Mischala).
- **AV1 Encoder Support**: Added support for AV1 hardware encoders for improved video quality and efficiency (contributed by @Mischala).

### Fixed
- **Batch Processing Reliability**: Resolved a logic issue where batch processing would spam log messages and fail to reset its state correctly after the last file was processed.
- **Improved SRT Parsing**: Enhanced regex-based parsing for debug SRT files to handle variable whitespace patterns more reliably.

## [0.5.1] - 2026-04-14

### Added
- **Artlynk Rendering Optimization**: Drastically improved CPU performance when blending on 4K files by introducing a flat byte-slice `fast_overlay` math module.

### Fixed
- **Artlynk Checksum Bottleneck**: Disabled `ffmpeg` checksum generation during Artlynk file data extraction, vastly speeding up OSD data parsing times.
- **Batch Processing Synchronization**: Addressed a severe bug where batch processing would not wait for background OSD data extraction to finish before loading the next file in the folder, queuing them properly now.

## [0.5.0] - 2026-04-01

### Fixed
- **Artlynk (BetaFPV P1) DVR rendering improvements**: Artlynk reduces the framerate in low-power mode to 15fps, so I adjusted the rendering to match the original 60 fps.

## [0.4.9] - 2026-03-23

### Added
- **Sequenced SRT Matching**: Automatically match and load sequential SRT files even if their filenames differ from the corresponding MP4 files (e.g., `AvatarS0044.mp4` with `AvatarG0039.srt`).
- **Duration Verification**: Visual warning (red text with `!`) in the UI when the loaded SRT file duration significantly differs from the video duration.
- **Improved Batch Processing**: Enhanced logic for finding the next SRT file in sequence during batch rendering.


## [0.4.8] - 2026-02-16

### Fixed
- **macOS UI Hang**: Moved dependency checks to a background thread to prevent the application from hanging on startup when binaries are blocked by Gatekeeper.
- **macOS Permissions**: Switched to native `zip` in CI to preserve executable bits in the app bundle.


## [0.4.7] - 2026-02-15

### Added
- **Batch Processing**: Automatically load and render the next MP4 in a folder after the current one finishes.
- Progress visualization for batch processing (current file / total files).
- Automatic top-left positioning of the application window on startup for better multi-monitor support.


## [0.4.6] - 2026-02-15

### Added
- Added "Select font folder" button and support for loading fonts from a user-specified directory.
- Automatic detection and selection of "Ascent" specific fonts for both Betaflight and INAV.
- Side panel auto-resize to accommodate long font filenames.

### Changed
- Parallel OSD rendering using all CPU cores (was single-threaded). Significantly improves rendering speed (FPS).

### Removed

- Removed embedded fonts as per SNEAKY_FPV request. You can download fonts from https://sites.google.com/view/sneaky-fpv/

## [0.4.5] - 2026-02-14

### Added

- Support for direct OSD data extraction from Artlynk MP4 recordings (SEI User Data messages).
- Background OSD extraction with visual "Scanning for OSD data..." indicator to prevent UI freezing.
- Optimized Artlynk OSD detection (fast 2s check) to avoid long scans on non-Artlynk videos.
- Support for Artlynk SRT telemetry fields: `AirTemp`, `GndTemp`, and `STYMode`.
- Context-aware default SRT settings for Avatar, Ascent, and Artlynk files.

### Changed

- Rearranged SRT telemetry checkboxes into a compact, wrapped horizontal layout.
- Preview window now automatically refreshes (font selection and centering) after background OSD extraction finishes.

## [0.4.4] - 2026-02-14

### Added

- Bundled OSD fonts for Betaflight, INAV, and ArduPilot (720p and 1080p). The correct font is automatically selected based on the video resolution and OSD firmware type.
- Auto-select first available H.264 hardware encoder on startup.
- Hardware-accelerated video decoding (`-hwaccel auto`) when a hardware encoder is selected.
- Auto-resize application window after loading files.

### Changed

- OSD rendering performance improved ~2x via glyph caching (each glyph is resized only once instead of every frame).
- `FontFile::get_character` now returns a reference instead of cloning the image.

## [0.4.3] - 2026-02-13

### Added

- OSD size slider (50-200%) to scale OSD characters for better visibility, especially on 4:3 video.
- Automatic OSD horizontal centering when loading files or toggling "Pad 4:3 to 16:9".
- Version number display in the top panel (reads from Cargo.toml automatically).

### Fixed

- OSD "Center" button now correctly accounts for 4:3 to 16:9 padding.
- Extended horizontal position slider range (-500 to 700) to support large centering offsets.

## [0.4.2] - 2026-02-13

### Changed

- Default encoding bitrate now matches the source video's bitrate upon import.

## [0.4.1] - 2026-02-13

### Added

- Ability to render video without loading an SRT file.

## [0.4.0] - 2026-02-13

### Added

- Support for Ascent V16.5.7 SRT format from Caddx Walksnail systems.
- Parsing of additional SRT fields: Hz, Sp, and Gp.
- Improved SRT text rendering with multi-line support and text shadows.
- Option for 4K upscaling

### Changed

- Updated `SrtFrameData` to support string-based channel names (e.g., "AUTO").
- Refined SRT overlay rendering logic for better visibility.

[0.3.0] - 2024-03-23

### Added

- Load last used OSD font file on startup (@dz0ny).
- Option to render video with a chroma key background instead of the input video so the OSD can be overlayed in a video editor.
- Support for Betaflight 4.5 four color fonts.
- Support for INAV two color fonts ([#43](https://github.com/avsaase/walksnail-osd-tool/pull/43), @mmosca).
- Support for 4K and 2.7K DVR ([#43](https://github.com/avsaase/walksnail-osd-tool/pull/43), @mmosca).

### Fixed

- Bug that caused font files with unexpected number of characters to not open.

## [0.2.0] - 2023-04-23

### Added

- Save OSD and SRT options between program runs.
- Custom position and text size of SRT data.
- Option to adjust OSD playback speed to correct for OSD lag with <=32.37.10 firmware.
- Check for app updates during startup.
- Hide/mask OSD elements from the rendered video ([demo](https://i.imgur.com/u8xi2tX.mp4)).
- Tooltips explaining options and settings.

### Changed

- When loading a SRT file with distance data the distance checkbox doesn't get automatically checked.
- Options sections can be collapsed to save screen space.

## [0.1.0] - 2023-03-31

### Fixed

- Parsing of firmware version 32.37.10 SRT data.

## [0.1.0-beta4] - 2023-03-28

### Added

- Render data from the SRT file on the video. Select which values are rendered.
- Automatically load the matching OSD and SRT files when importing a video (they must be in the same folder and have the same file name).
- Upscale output video to 1440p to get better compression on YouTube.

### Changed

- New UI layout with better support for different screen sizes.
- Many small UI tweaks.

### Fixed

- Show correct number of characters in font file.

## [0.1.0-beta3] - 2023-03-21

### Added

- Open files by dropping them on the window.
- Improve render speed.
- Logging of ffmpeg errors and warnings.
- Option to select undetected encoders (use at your own risk).
- Dark theme (default light, toggle by clicking the sun/moon icon in the top right).

### Changed

- Improved handling of ffmpeg events.

### Fixed

- Issue with non-critical ffmpeg errors stopping the render process.
- Output videos not playable in some video players.

## [0.1.0-beta2] - 2023-03-15

### Added

- Make main window resizable in vertical direction to accomodate retina displays and screens with lower resolutions.
- Display errors from ffmpeg.
- Display tooltip when hovering over start render button when it is disabled.

### Changed

- Improved formatting of "About" window.
- Improved display of render status when rendering is finished or cancelled.

### Fixed

- Check for `hevc_videotoolbox` encoder on MacOS.
- Stop ffmpeg decoder when encoder returns error.
- Fixed version info display.
- Properly disable buttons that cannot be used.

## [0.1.0-beta1] - 2023-03-11

### Added

First beta release with limited features.
