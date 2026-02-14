use std::path::Path;
use std::process::Command;
use std::time::Duration;

use regex::Regex;

use super::{
    error::OsdFileError,
    fc_firmware::FcFirmware,
    frame::Frame,
    glyph::{Glyph, GridPosition},
    osd_file::OsdFile,
};

const GRID_WIDTH: usize = 53;
const GRID_HEIGHT: usize = 20;

/// Extract SEI User Data entries from a video file using ffmpeg showinfo filter.
/// Returns a list of (pts_seconds, hex_string) tuples.
fn extract_sei_data(
    ffmpeg_path: &Path,
    video_path: &Path,
    max_duration: Option<Duration>,
) -> Vec<(f64, String)> {
    let mut command = Command::new(ffmpeg_path);

    let duration_str = max_duration.map(|t| format!("{:.3}", t.as_secs_f64()));
    if let Some(t) = &duration_str {
        command.arg("-t").arg(t);
    }

    command.args([
        "-i",
        video_path.to_str().unwrap_or(""),
        "-vf",
        "showinfo",
        "-f",
        "null",
        "-",
    ]);
    command.stdout(std::process::Stdio::null());
    command.stderr(std::process::Stdio::piped());

    #[cfg(target_os = "windows")]
    std::os::windows::process::CommandExt::creation_flags(&mut command, crate::util::CREATE_NO_WINDOW);

    let output = match command.output() {
        Ok(o) => o,
        Err(e) => {
            tracing::error!("Failed to run ffmpeg showinfo: {}", e);
            return Vec::new();
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);

    let pattern = Regex::new(r"(?s)pts_time:([\d.]+).*?User Data=([0-9a-fA-F\s]+)").expect("Invalid regex");

    let entries: Vec<_> = pattern
        .captures_iter(&stderr)
        .filter_map(|cap| {
            let pts: f64 = cap[1].parse().ok()?;
            let hex = cap[2].to_string();
            tracing::debug!(
                "Captured SEI hex (first 50 chars): {}",
                &hex.chars().take(50).collect::<String>()
            );
            Some((pts, hex))
        })
        .collect();

    tracing::info!("Found {} SEI User Data entries in stderr", entries.len());
    entries
}

/// Parse an MSP DisplayPort payload from hex string.
/// Returns a list of (row, col, glyph_index) tuples.
fn parse_msp_payload(hex_string: &str) -> Option<Vec<(u8, u8, u16)>> {
    // 1. Clean the string: remove address prefixes like "00000010:" and colons
    // showinfo often formats SEI data with address prefixes.
    let mut cleaned = String::with_capacity(hex_string.len());
    for line in hex_string.lines() {
        let line = line.trim();
        // If line starts with an address like "00000000: ", skip the address part
        if let Some(pos) = line.find(": ") {
            cleaned.push_str(&line[pos + 2..]);
        } else {
            cleaned.push_str(line);
        }
    }

    // 2. Remove all non-hex characters (except spaces which hex::decode handles poorly if not removed)
    let clean_hex: String = cleaned.chars().filter(|c| c.is_ascii_hexdigit()).collect();

    // Convert to bytes
    let raw_bytes = match hex::decode(&clean_hex) {
        Ok(b) => b,
        Err(e) => {
            tracing::debug!(
                "Hex decode failed for string: {}... error: {}",
                &clean_hex.chars().take(20).collect::<String>(),
                e
            );
            return None;
        }
    };

    // Structural removal: Remove every 3rd byte (the padding byte)
    // Artlynk SEI format often packs 2 bytes of data and 1 byte of filler (0xff)
    let mut data = Vec::with_capacity(raw_bytes.len() * 2 / 3);
    for (i, &b) in raw_bytes.iter().enumerate() {
        if (i + 1) % 3 != 0 {
            data.push(b);
        }
    }

    if data.len() < 9 {
        return None;
    }

    let num_commands = data[0] as usize;
    let mut offset = 9;
    let mut active_glyphs = Vec::new();

    for _ in 0..num_commands {
        if offset >= data.len() {
            break;
        }

        let cmd_payload_len = data[offset] as usize;

        if offset + 6 <= data.len() {
            let row = data[offset + 3];
            let col = data[offset + 4];
            let attribute = data[offset + 5];

            let num_glyphs = cmd_payload_len.saturating_sub(4);
            offset += 6;

            for i in 0..num_glyphs {
                if offset >= data.len() {
                    break;
                }
                let glyph_byte = data[offset] as u16;
                // Character index is 10-bit: 2 bits from attribute, 8 bits from glyph_byte
                let character = ((attribute as u16 & 0x03) << 8) | glyph_byte;
                active_glyphs.push((row, col.saturating_add(i as u8), character));
                offset += 1;
            }
        } else {
            break;
        }
    }

    Some(active_glyphs)
}

#[tracing::instrument(ret, err)]
pub fn extract_osd_from_video(
    ffmpeg_path: &Path,
    video_path: &Path,
) -> Result<Option<OsdFile>, OsdFileError> {
    let filename = video_path
        .file_name()
        .and_then(|f: &std::ffi::OsStr| f.to_str())
        .unwrap_or("")
        .to_lowercase();

    if filename.contains("ascent") || filename.contains("avatar") {
        tracing::info!(
            "Skipping Artlynk OSD extraction for file {:?} (name contains Ascent/Avatar)",
            video_path
        );
        return Ok(None);
    }

    tracing::info!("Attempting Artlynk OSD extraction from {:?}", video_path);

    // 1. Quick check: scan first 2 seconds to see if SEI data exists
    let quick_entries = extract_sei_data(ffmpeg_path, video_path, Some(Duration::from_secs(2)));
    if quick_entries.is_empty() {
        tracing::info!("No SEI User Data found in first 2 seconds, skipping full scan.");
        return Ok(None);
    }

    // 2. Full scan: if SEI data was found, extract everything
    let entries = extract_sei_data(ffmpeg_path, video_path, None);
    if entries.is_empty() {
        tracing::info!("No SEI User Data found in video during full scan");
        return Ok(None);
    }

    let mut frames = Vec::new();

    for (pts, hex_line) in &entries {
        let glyphs_raw = match parse_msp_payload(hex_line) {
            Some(g) => g,
            None => continue,
        };

        let ts_ms = (*pts * 1000.0) as u32;

        let glyphs: Vec<Glyph> = glyphs_raw
            .into_iter()
            .filter(|&(row, col, idx)| {
                (col as usize) < GRID_WIDTH && (row as usize) < GRID_HEIGHT && idx != 0x00 && idx != 0x20
            })
            .map(|(row, col, idx)| Glyph {
                index: idx,
                grid_position: GridPosition {
                    x: col as u32,
                    y: row as u32,
                },
            })
            .collect();

        if !glyphs.is_empty() {
            frames.push(Frame {
                time_millis: ts_ms,
                glyphs,
            });
        }
    }

    if frames.is_empty() {
        tracing::info!("SEI data found but no valid OSD frames parsed");
        return Ok(None);
    }

    let frame_count = frames.len() as u32;

    let frame_interval = if frames.len() > 1 {
        (frames.last().unwrap().time_millis - frames.first().unwrap().time_millis) as f32 / (frames.len() - 1) as f32
    } else {
        33.0 // ~30fps default
    };

    let duration = Duration::from_millis(frames.last().unwrap().time_millis.into())
        + Duration::from_secs_f32(frame_interval / 1000.0);

    tracing::info!("Extracted {} OSD frames from Artlynk SEI data", frame_count);

    Ok(Some(OsdFile {
        file_path: video_path.to_path_buf(),
        fc_firmware: FcFirmware::Betaflight,
        frame_count,
        duration,
        frames,
    }))
}
