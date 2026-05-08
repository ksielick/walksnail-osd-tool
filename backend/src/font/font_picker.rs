use std::path::Path;

use crate::{
    font::{dimensions::CharacterSize, FontFile},
    osd::FcFirmware,
};

/// Attempt to find a matching font file in the given folder based on FC firmware and character size.
#[must_use]
pub fn find_font_in_folder(
    folder: &Path,
    firmware: &FcFirmware,
    character_size: &CharacterSize,
    version: Option<&str>,
    osd_filename: Option<&str>,
) -> Option<FontFile> {
    if !folder.is_dir() {
        return None;
    }

    let is_ascent = osd_filename.is_some_and(|f| f.to_lowercase().contains("ascent"));

    let inav_prefix = version.map_or_else(
        || if is_ascent { "WS_INAV9_Ascent_" } else { "WS_INAV_" },
        |v| {
            if is_ascent && (v.starts_with('8') || v.chars().next().is_some_and(|c| c.is_ascii_digit() && c >= '9')) {
                "WS_INAV9_Ascent_"
            } else if v.starts_with('8') {
                "WS_INAV_8_"
            } else if v.chars().next().is_some_and(|c| c.is_ascii_digit() && c >= '9') {
                "WS_INAV9_"
            } else {
                "WS_INAV_"
            }
        },
    );

    let btfl_prefix = if is_ascent { "WS_BTFL_Ascent_" } else { "WS_BFx4_" };

    let search_patterns: Vec<String> = match (firmware, character_size) {
        // Betaflight (also used for Kiss, KissUltra, Unknown)
        (
            FcFirmware::Betaflight | FcFirmware::Kiss | FcFirmware::KissUltra | FcFirmware::Unknown,
            CharacterSize::Small | CharacterSize::Race,
        ) => vec![
            format!("{}Europa_24.png", btfl_prefix),
            format!("{}720p.png", btfl_prefix),
            format!("{}540p.png", btfl_prefix),
            "WS_BFx4_Europa_24.png".to_string(),
            "WS_BTFL_Europa_24.png".to_string(),
            "font_24.png".to_string(),
            "BF_720P.png".to_string(),
        ],
        (FcFirmware::Betaflight | FcFirmware::Kiss | FcFirmware::KissUltra | FcFirmware::Unknown, _) => {
            vec![
                format!("{}Europa_36.png", btfl_prefix),
                format!("{}1080p.png", btfl_prefix),
                "WS_BFx4_Europa_36.png".to_string(),
                "WS_BTFL_Europa_36.png".to_string(),
                "font_36.png".to_string(),
                "BF_1080P.png".to_string(),
            ]
        }

        // INAV
        (FcFirmware::Inav, CharacterSize::Small | CharacterSize::Race) => {
            vec![
                format!("{}720p.png", inav_prefix),
                format!("{}Europa_720p.png", inav_prefix),
                "WS_INAV_8_Europa_720p.png".to_string(),
                "WS_INAV_Europa_720p.png".to_string(),
                "INAV_720P.png".to_string(),
            ]
        }
        (FcFirmware::Inav, _) => {
            vec![
                format!("{}1080p.png", inav_prefix),
                format!("{}Europa_1080p.png", inav_prefix),
                "WS_INAV_8_Europa_1080p.png".to_string(),
                "WS_INAV_Europa_1080p.png".to_string(),
                "INAV_1080P.png".to_string(),
            ]
        }

        // ArduPilot
        (FcFirmware::ArduPilot, CharacterSize::Small | CharacterSize::Race) => {
            vec!["WS_ARDU_Europa_24.png".to_string(), "ARDU_720P.png".to_string()]
        }
        (FcFirmware::ArduPilot, _) => vec!["WS_ARDU_Europa_36.png".to_string(), "ARDU_1080P.png".to_string()],
    };

    for pattern in search_patterns {
        let font_path = folder.join(pattern);
        if font_path.exists() {
            if let Ok(font) = FontFile::open(font_path) {
                return Some(font);
            }
        }
    }

    if let Some(fallback_path) = find_compatible_fonts(folder, character_size, Some(firmware))
        .into_iter()
        .next()
    {
        if let Ok(font) = FontFile::open(fallback_path) {
            return Some(font);
        }
    }

    None
}

#[must_use]
pub fn find_compatible_fonts(
    folder: &Path,
    character_size: &CharacterSize,
    firmware: Option<&FcFirmware>,
) -> Vec<std::path::PathBuf> {
    let mut compatible_fonts = Vec::new();

    if let Ok(entries) = std::fs::read_dir(folder) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
                let file_name = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_uppercase())
                    .unwrap_or_default();

                let is_firmware_match = match firmware {
                    Some(FcFirmware::Betaflight | FcFirmware::Kiss | FcFirmware::KissUltra) => {
                        file_name.starts_with("WS_BTFL_")
                            || file_name.starts_with("WS_BFX4_")
                            || file_name.starts_with("BF_")
                            || file_name.starts_with("FONT_")
                    }
                    Some(FcFirmware::Inav) => {
                        file_name.starts_with("WS_INAV_")
                            || file_name.starts_with("WS_INAV9_")
                            || file_name.starts_with("INAV_")
                    }
                    Some(FcFirmware::ArduPilot) => file_name.starts_with("WS_ARDU_") || file_name.starts_with("ARDU_"),
                    _ => true,
                };

                if is_firmware_match {
                    if let Ok(reader) = image::ImageReader::open(&path) {
                        if let Ok((width, height)) = reader.into_dimensions() {
                            if let Ok((size, _, _)) = crate::font::dimensions::detect_dimensions(width, height) {
                                if size == *character_size {
                                    compatible_fonts.push(path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    compatible_fonts.sort();
    compatible_fonts
}
