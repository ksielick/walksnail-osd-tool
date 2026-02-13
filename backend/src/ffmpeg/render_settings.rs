use serde::{Deserialize, Serialize};

use crate::ffmpeg::{Codec, Encoder};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum UpscaleTarget {
    #[serde(rename = "none")]
    None,
    #[serde(rename = "1440p")]
    P1440,
    #[serde(rename = "2160p")]
    P2160,
}

impl std::fmt::Display for UpscaleTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                UpscaleTarget::None => "None",
                UpscaleTarget::P1440 => "1440p",
                UpscaleTarget::P2160 => "2160p",
            }
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RenderSettings {
    pub encoder: Encoder,
    pub selected_encoder_idx: usize,
    pub show_undetected_encoders: bool,
    pub bitrate_mbps: u32,
    pub upscale: UpscaleTarget,
    pub use_chroma_key: bool,
    pub chroma_key: [f32; 3],
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            encoder: Encoder {
                name: "libx264".to_string(),
                codec: Codec::H264,
                hardware: false,
                detected: false,
                extra_args: Vec::new(),
            },
            selected_encoder_idx: 0,
            show_undetected_encoders: false,
            bitrate_mbps: 40,
            upscale: UpscaleTarget::None,
            use_chroma_key: false,
            chroma_key: [1.0 / 255.0, 177.0 / 255.0, 64.0 / 255.0],
        }
    }
}
