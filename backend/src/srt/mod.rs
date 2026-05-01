mod error;
mod frame;
mod options;
mod srt_file;

pub use frame::{SrtFrame, SrtFrameData};
pub use options::SrtOptions;
pub use srt_file::SrtFile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SrtType {
    Avatar,
    Ascent,
    AscentDebug,
    Artlynk,
}

impl std::fmt::Display for SrtType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Avatar => write!(f, "Avatar"),
            Self::Ascent => write!(f, "Ascent"),
            Self::AscentDebug => write!(f, "Ascent Debug"),
            Self::Artlynk => write!(f, "Artlynk"),
        }
    }
}
