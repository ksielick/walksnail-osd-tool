#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // Hide console on Windows in release builds
#![allow(clippy::too_many_arguments)]
#![allow(clippy::collapsible_else_if)]

use app::WalksnailOsdTool;
use backend::{
    config::AppConfig,
    ffmpeg::{ffmpeg_available, ffprobe_available, Encoder},
};
use egui::{IconData, ViewportBuilder};
use poll_promise::Promise;

use crate::util::check_updates;

mod app;
mod audio_preview;
mod bottom_panel;
mod central_panel;
mod osd_preview;
mod render_status;
mod side_panel;
mod top_panel;
mod util;

use util::build_info;

fn main() -> Result<(), eframe::Error> {
    let _guard = util::init_tracing();

    tracing::info!(
        "{}",
        format!(
            "App started (version: {}, target: {}, compiled with: rustc {})",
            build_info::get_version(),
            build_info::get_target(),
            build_info::get_compiler()
        )
    );

    // On startup check if ffmpeg and ffprobe are available on the user's system
    // Then check which encoders are available
    let ffmpeg_path = util::get_dependency_path("ffmpeg");
    let ffprobe_path = util::get_dependency_path("ffprobe");

    let dep_ffmpeg_path = ffmpeg_path.clone();
    let dep_ffprobe_path = ffprobe_path.clone();

    let dependency_check_promise = Promise::spawn_thread("dependency_check", move || {
        let satisfied = ffmpeg_available(&dep_ffmpeg_path) && ffprobe_available(&dep_ffprobe_path);
        let encoders = if satisfied {
            Encoder::get_available_encoders(&dep_ffmpeg_path)
        } else {
            vec![]
        };
        (satisfied, encoders)
    });

    let config = AppConfig::load_or_create();
    let update_promise = if config.app_update.check_on_startup {
        Promise::spawn_thread("check_updates", check_updates).into()
    } else {
        None
    };

    let icon_data = IconData {
        rgba: include_bytes!(concat!(env!("OUT_DIR"), "/icon_bytes")).to_vec(),
        width: 256,
        height: 256,
    };

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_drag_and_drop(true)
            .with_icon(icon_data)
            .with_min_inner_size([600.0, 300.0])
            .with_inner_size([1000.0, 700.0])
            .with_position([0.0, 0.0]),
        ..Default::default()
    };
    tracing::info!("Starting GUI");
    eframe::run_native(
        "Walksnail OSD Tool",
        options,
        Box::new(move |cc| {
            Ok(Box::new(WalksnailOsdTool::new(
                &cc.egui_ctx,
                ffmpeg_path,
                ffprobe_path,
                config,
                match build_info::get_version() {
                    build_info::Build::Release { version, .. } => format!("v{version}"),
                    build_info::Build::Dev { commit } => format!("v{} dev ({commit})", env!("CARGO_PKG_VERSION")),
                    build_info::Build::Unknown => format!("v{}", env!("CARGO_PKG_VERSION")),
                },
                build_info::get_target().to_string(),
                dependency_check_promise,
                update_promise,
            )))
        }),
    )
}
