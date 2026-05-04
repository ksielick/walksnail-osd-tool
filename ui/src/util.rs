use std::{
    env::current_exe,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use backend::{
    config::AppConfig,
    ffmpeg::VideoInfo,
    font::FontFile,
    osd::OsdFile,
    srt::{SrtFile, SrtOptions},
};
use egui::{FontFamily, FontId, Margin, RichText, Separator, TextStyle, Ui};
use github_release_check::{GitHubReleaseItem, LookupError};
use poll_promise::Promise;
use semver::Version;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{filter, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt, Layer};

use super::WalksnailOsdTool;
use crate::util::build_info::Build;

impl WalksnailOsdTool {
    #[must_use]
    pub const fn all_files_loaded(&self) -> bool {
        if !self.video_loaded() || self.artlynk_extraction_promise.is_some() {
            return false;
        }
        if self.osd_loaded() && !self.font_loaded() {
            return false;
        }
        self.osd_loaded() || self.srt_file.is_some()
    }

    #[must_use]
    pub const fn video_loaded(&self) -> bool {
        self.video_file.is_some() && self.video_info.is_some()
    }

    #[must_use]
    pub const fn osd_loaded(&self) -> bool {
        self.osd_file.is_some()
    }

    #[must_use]
    pub const fn srt_loaded(&self) -> bool {
        self.srt_file.is_some()
    }

    #[must_use]
    pub const fn font_loaded(&self) -> bool {
        self.font_file.is_some()
    }

    pub fn import_video_file(&mut self, file_handles: &[PathBuf]) {
        self.pending_batch_render = false;
        if let Some(video_file) = filter_file_with_extention(file_handles, "mp4") {
            let old_video_file = self.video_file.clone();
            let old_osd_file = self.osd_file.as_ref().map(|o| o.file_path.clone());
            let old_srt_file = self.srt_file.as_ref().map(|s| s.file_path.clone());

            self.video_file = Some(video_file.clone());
            self.video_info = VideoInfo::get(video_file, &self.dependencies.ffprobe_path).ok();
            self.osd_file = None;
            self.srt_file = None;
            self.artlynk_extraction_promise = None;
            self.pending_batch_render = false;

            if let Some(video_info) = &self.video_info {
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    clippy::cast_precision_loss
                )]
                let bitrate = (video_info.bitrate as f32 / 1_000_000.0).round() as u32;
                self.render_settings.bitrate_mbps = bitrate;
            }

            // Try to load the matching OSD and SRT files
            let video_duration = self.video_info.as_ref().map(|v| v.duration);
            let mut osd_to_import = find_matching_file_with_extension(video_file, "osd", video_duration);
            if let (Some(old_video), Some(old_osd)) = (old_video_file.clone(), old_osd_file) {
                if old_video.file_stem() != old_osd.file_stem() {
                    if let Some(next_osd) = find_next_osd_file(&old_osd) {
                        tracing::info!(
                            "Differently named video/OSD pair detected, loading next OSD in sequence: {:?}",
                            next_osd
                        );
                        osd_to_import = Some(next_osd);
                    }
                }
            }
            if let Some(osd_to_import) = osd_to_import {
                self.import_osd_file(&[osd_to_import]);
            }

            let mut srt_to_import = find_matching_file_with_extension(video_file, "srt", video_duration);
            if let (Some(old_video), Some(old_srt)) = (old_video_file, old_srt_file) {
                if old_video.file_stem() != old_srt.file_stem() {
                    if let Some(next_srt) = find_next_srt_file(&old_srt) {
                        tracing::info!(
                            "Differently named video/SRT pair detected, loading next SRT in sequence: {:?}",
                            next_srt
                        );
                        srt_to_import = Some(next_srt);
                    }
                }
            }
            if let Some(srt_to_import) = srt_to_import {
                self.import_srt_file(&[srt_to_import]);
            }

            // Check if duration matches
            if let (Some(video_info), Some(srt_file)) = (&self.video_info, &self.srt_file) {
                let diff = (video_info.duration.as_secs_f32() - srt_file.duration.as_secs_f32()).abs();
                if diff > 1.0 {
                    tracing::warn!(
                        "Duration mismatch between video ({:?}) and SRT ({:?})!",
                        video_info.duration,
                        srt_file.duration
                    );
                } else {
                    tracing::info!("Video and SRT duration match: {:?}", video_info.duration);
                }
            }

            self.auto_select_font();

            // If no .osd file was loaded, try Artlynk extraction from video SEI data
            // For Artlynk, we also trigger extraction if the loaded OSD file doesn't match the video name exactly
            // (prevents batch processing from reusing the previous video's OSD file via fallback matching)
            let is_artlynk = self
                .srt_file
                .as_ref()
                .is_some_and(|s| s.srt_type == backend::srt::SrtType::Artlynk);
            let osd_matches_video = self
                .osd_file
                .as_ref()
                .is_some_and(|o| o.file_path.file_stem() == video_file.file_stem());

            if self.osd_file.is_none() || (is_artlynk && !osd_matches_video) {
                if is_artlynk && !osd_matches_video {
                    tracing::info!("Artlynk detected: Clearing mismatched OSD file and triggering extraction.");
                    self.osd_file = None;
                }
                let ffmpeg_path = self.dependencies.ffmpeg_path.clone();
                let video_path = video_file.clone();

                self.artlynk_extraction_promise = Some(Promise::spawn_thread("Artlynk extraction", move || {
                    backend::osd::artlynk::extract_osd_from_video(&ffmpeg_path, &video_path)
                }));
            }
            if let Some(parent) = video_file.parent() {
                if let Ok(entries) = std::fs::read_dir(parent) {
                    let mut mp4_files: Vec<PathBuf> = entries
                        .flatten()
                        .map(|e| e.path())
                        .filter(|p| {
                            p.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4"))
                                && !p
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .is_some_and(|n| n.to_lowercase().ends_with("_with_osd.mp4"))
                        })
                        .collect();
                    mp4_files.sort();
                    if let Some(idx) = mp4_files.iter().position(|p| p == video_file) {
                        self.batch_progress = Some((idx + 1, mp4_files.len()));
                    } else {
                        self.batch_progress = None;
                    }
                }
            }
        }
    }

    pub fn import_osd_file(&mut self, file_handles: &[PathBuf]) {
        if let Some(osd_file_path) = filter_file_with_extention(file_handles, "osd") {
            self.osd_file = OsdFile::open(osd_file_path.clone()).ok();
            self.osd_preview.preview_frame = 1;
        }
    }

    pub fn import_srt_file(&mut self, file_handles: &[PathBuf]) {
        if let Some(srt_file_path) = filter_file_with_extention(file_handles, "srt") {
            self.srt_file = SrtFile::open(srt_file_path.clone()).ok();

            if let Some(srt_file) = &self.srt_file {
                if let Some(profile) = self.srt_profiles.get(&srt_file.srt_type) {
                    tracing::info!("Applying SRT profile for {}", srt_file.srt_type);
                    self.srt_options = profile.clone();
                } else {
                    let file_name = srt_file_path
                        .file_name()
                        .map(|f| f.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    if file_name.starts_with("avatar") || file_name.starts_with("ascent") {
                        tracing::info!("Applying Avatar/Ascent SRT defaults");
                        self.srt_options = SrtOptions::walksnail_optimized();
                    } else {
                        tracing::info!("Applying Artlynk/Default SRT defaults");
                        self.srt_options = SrtOptions::default();
                    }
                }
            }

            self.srt_options.show_distance &= self.srt_file.as_ref().is_none_or(|s| s.has_distance);
            self.config_changed = Some(Instant::now());
        }
    }

    pub fn import_font_file(&mut self, file_handles: &[PathBuf]) {
        if let Some(font_file_path) = filter_file_with_extention(file_handles, "png") {
            self.font_file = FontFile::open(font_file_path.clone()).ok();
            self.font_manually_selected = true;

            if let Some(osd_file) = &self.osd_file {
                let srt_type = self.srt_file.as_ref().map(|s| s.srt_type);
                let profile_key = (osd_file.fc_firmware.clone(), srt_type);
                self.font_profiles
                    .insert(profile_key, font_file_path.to_string_lossy().to_string());
            }

            self.config_changed = Some(Instant::now());
        }
    }

    pub fn auto_select_font(&mut self) {
        if let (Some(video_info), Some(osd_file)) = (&self.video_info, &self.osd_file) {
            let character_size = backend::overlay::get_character_size(video_info.width, video_info.height);
            let srt_type = self.srt_file.as_ref().map(|s| s.srt_type);
            let profile_key = (osd_file.fc_firmware.clone(), srt_type);

            if let Some(saved_path_str) = self.font_profiles.get(&profile_key) {
                let saved_path = PathBuf::from(saved_path_str);
                if self.font_file.as_ref().is_none_or(|f| f.file_path != saved_path) {
                    if let Ok(font) = FontFile::open(saved_path.clone()) {
                        tracing::info!("Applying saved font profile for {:?}: {:?}", profile_key, saved_path);
                        self.font_file = Some(font);
                        self.font_manually_selected = true;
                        return;
                    }
                } else {
                    return;
                }
            }

            // Only auto-select if no font loaded, or the current font is not a good match.
            // If the user manually selected a font, we only change it if the resolution is incompatible.
            let should_auto_select = match &self.font_file {
                None => {
                    tracing::info!("Auto-select: No font loaded, setting should_auto_select = true");
                    true
                }
                Some(f) => {
                    if self.font_manually_selected {
                        let size_mismatch = f.character_size != character_size;

                        let file_name = f
                            .file_path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_uppercase())
                            .unwrap_or_default();
                        let is_firmware_match = match &osd_file.fc_firmware {
                            backend::osd::FcFirmware::Betaflight
                            | backend::osd::FcFirmware::Kiss
                            | backend::osd::FcFirmware::KissUltra => {
                                file_name.starts_with("WS_BTFL_")
                                    || file_name.starts_with("WS_BFX4_")
                                    || file_name.starts_with("BF_")
                                    || file_name.starts_with("FONT_")
                            }
                            backend::osd::FcFirmware::Inav => {
                                file_name.starts_with("WS_INAV_")
                                    || file_name.starts_with("WS_INAV9_")
                                    || file_name.starts_with("INAV_")
                            }
                            backend::osd::FcFirmware::ArduPilot => {
                                file_name.starts_with("WS_ARDU_") || file_name.starts_with("ARDU_")
                            }
                            _ => true,
                        };
                        let firmware_mismatch = !is_firmware_match;
                        let mismatch = size_mismatch || firmware_mismatch;

                        if mismatch {
                            tracing::info!(
                                "Auto-select: Manual font loaded but mismatch (size_mismatch: {}, firmware_mismatch: {}), setting should_auto_select = true",
                                size_mismatch,
                                firmware_mismatch
                            );
                        } else {
                            tracing::info!("Auto-select: Manual font loaded and matches, skipping auto-selection");
                        }
                        mismatch
                    } else {
                        tracing::info!(
                            "Auto-select: Auto-selected font loaded, allowing re-selection for better match"
                        );
                        true
                    }
                }
            };

            if should_auto_select {
                if let Some(font) = backend::font::font_picker::find_font_in_folder(
                    &self.userfont_path,
                    &osd_file.fc_firmware,
                    &character_size,
                    osd_file.version.as_deref(),
                    osd_file.file_path.file_name().and_then(|n| n.to_str()),
                ) {
                    // Only update if it's actually a different file
                    if self.font_file.as_ref().is_none_or(|f| f.file_path != font.file_path) {
                        tracing::info!(
                            "Auto-selecting new font: {:?} (Old was: {:?})",
                            font.file_path,
                            self.font_file.as_ref().map(|f| &f.file_path)
                        );
                        self.font_file = Some(font);
                        self.font_manually_selected = false;
                    }
                }
            }
        }
    }
}

pub fn filter_file_with_extention<'a>(files: &'a [PathBuf], extention: &'a str) -> Option<&'a PathBuf> {
    files.iter().find_map(|f| {
        f.extension().and_then(|e| {
            if e.to_string_lossy() == extention {
                Some(f)
            } else {
                None
            }
        })
    })
}

#[tracing::instrument(ret, level = "info")]
pub fn find_matching_file_with_extension(
    path: &PathBuf,
    extension: &str,
    target_duration: Option<Duration>,
) -> Option<PathBuf> {
    let file_name = path.file_stem().unwrap_or_default();
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let direct_match = parent.join(file_name).with_extension(extension);

    if direct_match.exists() {
        return Some(direct_match);
    }

    // Fallback: search for the file with the closest time or duration match in the same directory
    if let Ok(entries) = std::fs::read_dir(parent) {
        let files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)))
            .collect();

        if files.is_empty() {
            return None;
        }

        // 1. Try time matching (Priority fallback)
        if let Ok(metadata) = std::fs::metadata(path) {
            if let Ok(target_time) = metadata.modified() {
                let mut best_time_match = None;
                let mut min_time_diff = Duration::from_secs(u64::MAX);

                for file in &files {
                    if let Ok(m) = std::fs::metadata(file) {
                        if let Ok(modified) = m.modified() {
                            let diff = if modified > target_time {
                                modified.duration_since(target_time).unwrap_or(Duration::from_secs(u64::MAX))
                            } else {
                                target_time.duration_since(modified).unwrap_or(Duration::from_secs(u64::MAX))
                            };

                            if diff < min_time_diff {
                                min_time_diff = diff;
                                best_time_match = Some(file.clone());
                            }
                        }
                    }
                }

                // If the closest file is within a reasonable threshold (e.g. 10 seconds), use it.
                if let Some(match_path) = best_time_match {
                    if min_time_diff < Duration::from_secs(10) {
                        tracing::info!(
                            "Found match by time: {:?} (diff: {:?})",
                            match_path.file_name().unwrap_or_default(),
                            min_time_diff
                        );
                        return Some(match_path);
                    }
                }
            }
        }

        // 2. Try duration matching (Secondary fallback)
        if let Some(target) = target_duration {
            let mut best_match = None;
            let mut min_diff = f32::MAX;

            for file in &files {
                let duration = if extension.eq_ignore_ascii_case("osd") {
                    OsdFile::open(file.clone()).ok().map(|o| o.duration)
                } else if extension.eq_ignore_ascii_case("srt") {
                    SrtFile::open(file.clone()).ok().map(|s| s.duration)
                } else {
                    None
                };

                if let Some(dur) = duration {
                    let diff = (dur.as_secs_f32() - target.as_secs_f32()).abs();
                    if diff < min_diff {
                        min_diff = diff;
                        best_match = Some(file.clone());
                    }
                }
            }
            if let Some(match_path) = best_match {
                return Some(match_path);
            }
        }

        // Final fallback if all matching failed or wasn't possible
        let mut sorted_files = files;
        sorted_files.sort();
        return sorted_files.first().cloned();
    }

    None
}

pub fn find_next_srt_file(current_srt: &Path) -> Option<PathBuf> {
    find_next_file_with_extension(current_srt, "srt")
}

pub fn find_next_osd_file(current_osd: &Path) -> Option<PathBuf> {
    find_next_file_with_extension(current_osd, "osd")
}

fn find_next_file_with_extension(current_file: &Path, extension: &str) -> Option<PathBuf> {
    if let Some(parent) = current_file.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)))
                .collect();
            files.sort();

            if let Some(idx) = files.iter().position(|p| p == current_file) {
                if idx + 1 < files.len() {
                    return Some(files[idx + 1].clone());
                }
            }
        }
    }
    None
}

pub fn separator_with_space(ui: &mut Ui, space: f32) {
    ui.scope(|ui| {
        ui.visuals_mut().widgets.noninteractive.bg_stroke.width = 0.5;
        ui.add(Separator::default().spacing(space));
    });
}

pub fn format_minutes_seconds(duration: &Duration) -> String {
    let minutes = duration.as_secs() / 60;
    let seconds = duration.as_secs() % 60;
    format!("{minutes}:{seconds:0>2}")
}

pub fn get_output_video_path(input_video_path: &Path) -> PathBuf {
    let input_video_file_name = input_video_path
        .file_stem()
        .map_or_else(|| "output".to_string(), |s| s.to_string_lossy().to_string());
    let output_video_file_name = format!("{input_video_file_name}_with_osd.mp4");
    let mut output_video_path = input_video_path.parent().unwrap_or(Path::new("")).to_path_buf();
    output_video_path.push(output_video_file_name);
    output_video_path
}

pub fn set_style(ctx: &egui::Context) {
    use egui::{
        FontFamily::{Monospace, Proportional},
        Style,
    };
    let mut style = Style::clone(&ctx.style());
    style.text_styles = [
        (TextStyle::Small, FontId::new(9.0, Proportional)),
        (TextStyle::Body, FontId::new(15.0, Proportional)),
        (TextStyle::Button, FontId::new(15.0, Proportional)),
        (TextStyle::Heading, FontId::new(17.0, Proportional)),
        (TextStyle::Monospace, FontId::new(14.0, Monospace)),
        (TextStyle::Name("Tooltip".into()), FontId::new(14.0, Proportional)),
    ]
    .into();
    style.spacing.window_margin = Margin {
        left: 20.0,
        right: 20.0,
        top: 6.0,
        bottom: 20.0,
    };
    ctx.set_style(style);
}

pub fn tooltip_text(text: &str) -> RichText {
    RichText::new(text).font(FontId::new(14.0, FontFamily::Proportional))
}

pub fn set_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "inter-regular".to_owned(),
        egui::FontData::from_static(include_bytes!("../../resources/fonts/Inter-Regular.ttf")),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "inter-regular".to_owned());

    ctx.set_fonts(fonts);
}

#[allow(clippy::from_over_into)]
impl Into<AppConfig> for &mut WalksnailOsdTool {
    fn into(self) -> AppConfig {
        AppConfig {
            osd_options: self.osd_options.clone(),
            srt_options: self.srt_options.clone(),
            srt_profiles: self.srt_profiles.clone(),
            font_profiles: self.font_profiles.clone(),
            render_options: self.render_settings.clone(),
            app_update: backend::util::AppUpdate {
                check_on_startup: self.app_update.check_on_startup,
            },
            font_path: self
                .font_file
                .as_ref()
                .map(|f| f.file_path.clone())
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            userfont_path: self.userfont_path.to_string_lossy().to_string(),
            batch_processing: self.batch_processing,
        }
    }
}

pub fn init_tracing() -> Option<WorkerGuard> {
    directories::ProjectDirs::from("rs", "", "walksnail-osd-tool").map(|dir| {
        let log_dir = dir.data_dir();

        std::fs::remove_file(log_dir.join("walksnail-osd-tool.log")).ok();

        let file_appender = tracing_appender::rolling::never(log_dir, "walksnail-osd-tool.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

        let stdout_log = tracing_subscriber::fmt::layer()
            .pretty()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_filter(filter::LevelFilter::INFO);
        let file_log = tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .compact()
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
            .with_writer(non_blocking)
            .with_filter(filter::LevelFilter::INFO);
        tracing_subscriber::registry().with(stdout_log).with(file_log).init();

        guard
    })
}

pub fn get_dependency_path(dependency: &str) -> PathBuf {
    let cur_exe = current_exe().unwrap_or_default();
    let exe_dir = cur_exe.parent().unwrap_or(Path::new(""));

    if cfg!(all(target_os = "macos", feature = "macos-app-bundle")) {
        // Folder structure:
        // |
        // +-- MacOS
        //     +-- walksnail-osd-tool
        //     +-- ffmpeg
        //     +-- ffprobe
        exe_dir.join(dependency)
    } else if cfg!(all(target_os = "windows", feature = "windows-installer")) {
        // Folder structure:
        // |
        // +-- bin
        // |   +-- walksnail-osd-tool.exe
        // +-- ffmpeg
        //     +-- ffmpeg.exe
        //     +-- ffprobe.exe
        exe_dir
            .parent()
            .unwrap_or(Path::new(""))
            .join("ffmpeg")
            .join(dependency)
    } else {
        dependency.into()
    }
}

pub mod build_info {
    use std::fmt::Display;

    use semver::Version;

    pub enum Build {
        Release {
            version: Version,
            #[allow(unused)] // For some reason this field gets flagged as unused
            commit: String,
        },
        Dev {
            commit: String,
        },
        Unknown,
    }

    impl Display for Build {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Release { version, .. } => write!(f, "{version}"),
                Self::Dev { commit } => write!(f, "dev ({commit})"),
                Self::Unknown => write!(f, "Unknown"),
            }
        }
    }

    pub fn get_version() -> Build {
        let version: Option<Version> = option_env!("GIT_VERSION").and_then(|s| Version::parse(s).ok());
        let short_hash: Option<&'static str> = option_env!("GIT_COMMIT_HASH");

        match (version, short_hash.map(std::string::ToString::to_string)) {
            (Some(version), Some(commit)) => Build::Release { version, commit },
            (None, Some(commit)) => Build::Dev { commit },
            _ => Build::Unknown,
        }
    }

    #[must_use]
    pub const fn get_compiler() -> &'static str {
        env!("VERGEN_RUSTC_SEMVER")
    }

    #[must_use]
    pub const fn get_target() -> &'static str {
        env!("VERGEN_CARGO_TARGET_TRIPLE")
    }
}

#[tracing::instrument(ret)]
pub fn check_updates() -> Result<Option<GitHubReleaseItem>, LookupError> {
    if let Build::Release {
        version: current_version,
        ..
    } = build_info::get_version()
    {
        let github = github_release_check::GitHub::new().unwrap();
        let releases = github.query("avsaase/walksnail-osd-tool")?;
        let update_target = releases
            .iter()
            .find(|release| {
                Version::parse(release.tag_name.trim_start_matches('v'))
                    .is_ok_and(|version| should_update_to_version(&current_version, &version))
            })
            .cloned();
        Ok(update_target)
    } else {
        Ok(None)
    }
}

fn should_update_to_version(current_version: &Version, to_version: &Version) -> bool {
    if to_version <= current_version {
        return false;
    }

    let version_is_full_release = to_version.pre.is_empty();
    if version_is_full_release {
        return true;
    }

    let current_version_is_prerelease = !current_version.pre.is_empty();
    if current_version_is_prerelease {
        return to_version.major == current_version.major && to_version.minor == current_version.minor;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(version: &str) -> Version {
        Version::parse(version).unwrap()
    }

    #[test]
    fn update_to_new_release() {
        let current_version = version("0.1.0");
        let new_version = version("0.2.0");
        assert!(should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn not_update_to_older_release() {
        let current_version = version("0.2.0");
        let new_version = version("0.1.0");
        assert!(!should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn update_from_prerelease_to_full_release() {
        let current_version = version("0.1.0-beta.2");
        let new_version = version("0.1.0");
        assert!(should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn update_from_prerelease_to_new_prerelease() {
        let current_version = version("0.1.0-beta.1");
        let new_version = version("0.1.0-beta.3");
        assert!(should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn not_update_from_prerelease_to_older_prerelease() {
        let current_version = version("0.1.0-beta.3");
        let new_version = version("0.1.0-beta.2");
        assert!(!should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn not_update_from_prerelease_to_prerelease_in_new_cyce() {
        let current_version = version("0.1.0-beta.3");
        let new_version = version("0.2.0-beta.2");
        assert!(!should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn not_update_from_release_to_prerelease_of_new_release() {
        let current_version = version("0.1.0");
        let new_version = version("0.2.0-beta.2");
        assert!(!should_update_to_version(&current_version, &new_version));
    }

    #[test]
    fn not_update_to_same_release() {
        let current_version = version("0.1.0");
        assert!(!should_update_to_version(&current_version, &current_version));
    }
}
