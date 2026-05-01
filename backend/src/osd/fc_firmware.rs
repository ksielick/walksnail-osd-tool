use std::fmt::Display;

use super::error::OsdFileError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum FcFirmware {
    Betaflight,
    Inav,
    ArduPilot,
    Kiss,
    KissUltra,
    Unknown,
}

impl TryFrom<&str> for FcFirmware {
    type Error = OsdFileError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "BTFL" => Ok(Self::Betaflight),
            "INAV" => Ok(Self::Inav),
            "ARDU" => Ok(Self::ArduPilot),
            "KISS" => Ok(Self::Kiss),
            "ULTR" => Ok(Self::KissUltra),
            _ => Ok(Self::Unknown),
        }
    }
}

impl TryFrom<&[u8]> for FcFirmware {
    type Error = OsdFileError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        if value.len() < 4 {
            return Ok(Self::Unknown);
        }
        match &value[0..4] {
            b"BTFL" => Ok(Self::Betaflight),
            b"INAV" => Ok(Self::Inav),
            b"ARDU" => Ok(Self::ArduPilot),
            b"KISS" => Ok(Self::Kiss),
            b"ULTR" => Ok(Self::KissUltra),
            _ => Ok(Self::Unknown),
        }
    }
}

impl Display for FcFirmware {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Betaflight => "BetaFlight",
                Self::Inav => "INAV",
                Self::ArduPilot => "ArduPilot",
                Self::Kiss => "KISS",
                Self::KissUltra => "KISS ULTRA",
                Self::Unknown => "Unknown",
            }
        )
    }
}

impl FcFirmware {
    pub fn as_bytes(&self) -> &'static [u8] {
        match self {
            Self::Betaflight => b"BTFL",
            Self::Inav => b"INAV",
            Self::ArduPilot => b"ARDU",
            Self::Kiss => b"KISS",
            Self::KissUltra => b"ULTR",
            Self::Unknown => b"UNKN",
        }
    }
}
