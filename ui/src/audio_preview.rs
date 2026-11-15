use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use backend::ffmpeg::{extract_audio_f32, process_audio_samples, RenderSettings, VideoInfo};
use poll_promise::Promise;

const PREVIEW_DURATION_SECS: f64 = 5.0;

#[derive(Clone)]
pub struct RawPreviewClip {
    pub video_path: PathBuf,
    pub start_ms: u64,
    pub sample_rate: u32,
    pub channels: u16,
    pub raw_samples: Vec<f32>,
}

impl RawPreviewClip {
    #[allow(clippy::cast_precision_loss)]
    fn duration_secs(&self) -> f32 {
        let frames = self.raw_samples.len() / usize::from(self.channels.max(1));
        frames as f32 / self.sample_rate.max(1) as f32
    }
}

pub struct AudioPreviewState {
    pub play_original: bool,
    pub extract_promise: Option<Promise<Result<RawPreviewClip, String>>>,
    pub cached_clip: Option<RawPreviewClip>,
    playback: Option<ActivePlayback>,
    started_at: Option<Instant>,
    duration_secs: f32,
    pub last_error: Option<String>,
}

impl Default for AudioPreviewState {
    fn default() -> Self {
        Self {
            play_original: false,
            extract_promise: None,
            cached_clip: None,
            playback: None,
            started_at: None,
            duration_secs: 0.0,
            last_error: None,
        }
    }
}

impl AudioPreviewState {
    #[must_use]
    pub fn is_loading(&self) -> bool {
        self.extract_promise.is_some()
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.playback.as_ref().is_some_and(ActivePlayback::is_playing)
    }

    #[must_use]
    pub fn elapsed_secs(&self) -> Option<f32> {
        if self.is_playing() {
            self.started_at
                .map(|t| t.elapsed().as_secs_f32().min(self.duration_secs))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn duration_secs(&self) -> f32 {
        self.duration_secs
    }

    pub fn stop(&mut self) {
        self.extract_promise = None;
        self.playback = None;
        self.started_at = None;
    }

    pub fn clear_cache(&mut self) {
        self.stop();
        self.cached_clip = None;
        self.last_error = None;
    }

    #[must_use]
    pub fn preview_start_secs(preview_frame: u32, video_info: &VideoInfo) -> f64 {
        let fps = f64::from(video_info.frame_rate.max(1.0));
        let raw_start = f64::from(preview_frame.saturating_sub(1)) / fps;
        let max_start = (video_info.duration.as_secs_f64() - 1.0).max(0.0);
        raw_start.clamp(0.0, max_start)
    }

    pub fn toggle(
        &mut self,
        ffmpeg_path: &Path,
        video_path: &Path,
        video_info: &VideoInfo,
        preview_frame: u32,
        render_settings: &RenderSettings,
    ) {
        if self.is_playing() || self.is_loading() {
            self.stop();
            return;
        }
        self.start(ffmpeg_path, video_path, video_info, preview_frame, render_settings);
    }

    pub fn restart_if_playing(&mut self, render_settings: &RenderSettings) {
        if !self.is_playing() {
            return;
        }
        if let Some(clip) = self.cached_clip.clone() {
            self.play_clip(&clip, render_settings);
        }
    }

    fn start(
        &mut self,
        ffmpeg_path: &Path,
        video_path: &Path,
        video_info: &VideoInfo,
        preview_frame: u32,
        render_settings: &RenderSettings,
    ) {
        self.last_error = None;
        let start_secs = Self::preview_start_secs(preview_frame, video_info);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let start_ms = (start_secs * 1000.0).round() as u64;

        if let Some(clip) = self.cached_clip.clone() {
            if clip.video_path == video_path && clip.start_ms == start_ms {
                self.play_clip(&clip, render_settings);
                return;
            }
        }

        let ffmpeg = ffmpeg_path.to_path_buf();
        let video = video_path.to_path_buf();
        let sample_rate = video_info.audio_sample_rate.unwrap_or(8000);
        let channels = video_info.audio_channels.unwrap_or(1);

        self.extract_promise = Some(Promise::spawn_thread("audio_preview_extract", move || {
            let raw_samples = extract_audio_f32(
                &ffmpeg,
                &video,
                sample_rate,
                channels,
                Some(start_secs),
                Some(PREVIEW_DURATION_SECS),
            )
            .map_err(|e| e.to_string())?;

            if raw_samples.is_empty() {
                return Err("No audio samples found in this segment".to_string());
            }

            Ok(RawPreviewClip {
                video_path: video,
                start_ms,
                sample_rate,
                channels,
                raw_samples,
            })
        }));
    }

    pub fn poll(&mut self, ctx: &egui::Context, render_settings: &RenderSettings) {
        if let Some(ref playback) = self.playback {
            if !playback.is_playing() {
                self.playback = None;
                self.started_at = None;
            }
        }

        if let Some(promise) = self.extract_promise.take() {
            match promise.try_take() {
                Ok(Ok(clip)) => {
                    self.cached_clip = Some(clip.clone());
                    self.play_clip(&clip, render_settings);
                }
                Ok(Err(err)) => {
                    self.last_error = Some(err);
                }
                Err(pending) => {
                    self.extract_promise = Some(pending);
                }
            }
        }

        if self.is_playing() || self.is_loading() {
            ctx.request_repaint();
        }
    }

    fn play_clip(&mut self, clip: &RawPreviewClip, render_settings: &RenderSettings) {
        let samples = if render_settings.audio_cleanup && !self.play_original {
            process_audio_samples(
                &clip.raw_samples,
                clip.sample_rate,
                clip.channels,
                render_settings.audio_filter_strength,
                render_settings.audio_gain_db,
            )
        } else {
            // Clamp raw decoded floats to [-1.0, 1.0] to match standard player output clipping.
            clip.raw_samples.iter().map(|s| s.clamp(-1.0, 1.0)).collect()
        };

        match ActivePlayback::start(clip.sample_rate, clip.channels, samples) {
            Ok(playback) => {
                self.duration_secs = clip.duration_secs();
                self.started_at = Some(Instant::now());
                self.playback = Some(playback);
                self.last_error = None;
            }
            Err(err) => {
                self.playback = None;
                self.started_at = None;
                self.last_error = Some(err);
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct ActivePlayback {
    _stream: rodio::OutputStream,
    sink: rodio::Sink,
}

#[cfg(not(target_os = "linux"))]
impl ActivePlayback {
    fn start(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Result<Self, String> {
        let (stream, stream_handle) =
            rodio::OutputStream::try_default().map_err(|e| format!("Audio device unavailable: {e}"))?;
        let sink = rodio::Sink::try_new(&stream_handle).map_err(|e| format!("Audio sink error: {e}"))?;
        let buffer = rodio::buffer::SamplesBuffer::new(channels, sample_rate, samples);
        sink.append(buffer);
        Ok(Self { _stream: stream, sink })
    }

    fn is_playing(&self) -> bool {
        !self.sink.empty()
    }
}

#[cfg(target_os = "linux")]
struct ActivePlayback {
    child: std::sync::Mutex<std::process::Child>,
    wav_path: PathBuf,
}

#[cfg(target_os = "linux")]
impl ActivePlayback {
    fn start(sample_rate: u32, channels: u16, samples: Vec<f32>) -> Result<Self, String> {
        let wav_path = std::env::temp_dir().join(format!("walksnail_osd_preview_{}.wav", std::process::id()));
        backend::ffmpeg::write_wav_f32(&wav_path, &samples, sample_rate, channels)
            .map_err(|e| format!("Failed to write preview WAV: {e}"))?;

        for player in ["pw-play", "paplay", "aplay"] {
            if let Ok(child) = std::process::Command::new(player).arg(&wav_path).spawn() {
                return Ok(Self {
                    child: std::sync::Mutex::new(child),
                    wav_path,
                });
            }
        }

        let _ = std::fs::remove_file(&wav_path);
        Err("No audio player found (tried pw-play, paplay, aplay)".to_string())
    }

    fn is_playing(&self) -> bool {
        self.child
            .lock()
            .map(|mut child| matches!(child.try_wait(), Ok(None)))
            .unwrap_or(false)
    }
}

#[cfg(target_os = "linux")]
impl Drop for ActivePlayback {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
        let _ = std::fs::remove_file(&self.wav_path);
    }
}
