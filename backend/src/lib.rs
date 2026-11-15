#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_else_if)]
#![allow(clippy::redundant_field_names)]

pub mod config;
pub mod ffmpeg;
pub mod font;
pub mod osd;
pub mod overlay;
pub mod srt;
pub mod util;

pub const NAMESPACE: &str = "walksnail-osd-tool";
