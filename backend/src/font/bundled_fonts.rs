use crate::font::dimensions::CharacterSize;
use crate::font::FontFile;
use crate::osd::FcFirmware;

/// Embedded font PNG bytes
const BF_720P: &[u8] = include_bytes!("../../../_userfont/WS_BFx4_Europa_24.png");
const BF_1080P: &[u8] = include_bytes!("../../../_userfont/WS_BFx4_Europa_36.png");
const INAV_720P: &[u8] = include_bytes!("../../../_userfont/WS_INAV_8_Europa_720p.png");
const INAV_1080P: &[u8] = include_bytes!("../../../_userfont/WS_INAV_8_Europa_1080p.png");
const ARDU_720P: &[u8] = include_bytes!("../../../_userfont/WS_ARDU_Europa_24.png");
const ARDU_1080P: &[u8] = include_bytes!("../../../_userfont/WS_ARDU_Europa_36.png");

/// Select and load the appropriate bundled font based on FC firmware and video resolution.
///
/// Returns `None` only if the embedded bytes fail to parse (should never happen with valid assets).
pub fn get_bundled_font(firmware: &FcFirmware, character_size: &CharacterSize) -> Option<FontFile> {
    let (bytes, name) = match (firmware, character_size) {
        // Betaflight (also used for Kiss, KissUltra, Unknown)
        (FcFirmware::Betaflight | FcFirmware::Kiss | FcFirmware::KissUltra | FcFirmware::Unknown, CharacterSize::Small | CharacterSize::Race) => {
            (BF_720P, "Betaflight 720p (bundled)")
        }
        (FcFirmware::Betaflight | FcFirmware::Kiss | FcFirmware::KissUltra | FcFirmware::Unknown, _) => {
            (BF_1080P, "Betaflight 1080p (bundled)")
        }

        // INAV
        (FcFirmware::Inav, CharacterSize::Small | CharacterSize::Race) => {
            (INAV_720P, "INAV 720p (bundled)")
        }
        (FcFirmware::Inav, _) => {
            (INAV_1080P, "INAV 1080p (bundled)")
        }

        // ArduPilot
        (FcFirmware::ArduPilot, CharacterSize::Small | CharacterSize::Race) => {
            (ARDU_720P, "ArduPilot 720p (bundled)")
        }
        (FcFirmware::ArduPilot, _) => {
            (ARDU_1080P, "ArduPilot 1080p (bundled)")
        }
    };

    FontFile::from_bytes(name, bytes).ok()
}
