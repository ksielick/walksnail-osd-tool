use serde::{Deserialize, Serialize};

use crate::util::Coordinates;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrtOptions {
    pub position: Coordinates<f32>,
    pub scale: f32,
    pub show_time: bool,
    pub show_sbat: bool,
    pub show_gbat: bool,
    pub show_signal: bool,
    pub show_latency: bool,
    pub show_bitrate: bool,
    pub show_distance: bool,
    pub show_channel: bool,
    pub show_hz: bool,
    pub show_sp: bool,
    pub show_gp: bool,
    pub show_air_temp: bool,
    pub show_gnd_temp: bool,
    pub show_sty_mode: bool,
}

impl SrtOptions {
    pub fn walksnail_optimized() -> Self {
        Self {
            position: Coordinates::new(1.5, 94.0),
            scale: 34.0,
            show_time: false,
            show_sbat: false,
            show_gbat: false,
            show_signal: true,
            show_latency: true,
            show_bitrate: true,
            show_distance: true,
            show_channel: true,
            show_hz: true,
            show_sp: true,
            show_gp: true,
            show_air_temp: false,
            show_gnd_temp: false,
            show_sty_mode: false,
        }
    }
}

impl Default for SrtOptions {
    fn default() -> Self {
        Self {
            position: Coordinates::new(1.5, 94.0),
            scale: 34.0,
            show_time: false,
            show_sbat: true,
            show_gbat: true,
            show_signal: true,
            show_latency: false,
            show_bitrate: true,
            show_distance: true,
            show_channel: true,
            show_hz: false,
            show_sp: false,
            show_gp: false,
            show_air_temp: true,
            show_gnd_temp: true,
            show_sty_mode: false,
        }
    }
}
