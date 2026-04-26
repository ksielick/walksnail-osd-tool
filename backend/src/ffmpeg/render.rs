use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::PathBuf,
    sync::Arc,
    thread,
};

use crossbeam_channel::{Receiver, Sender};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};
use image::{Rgba, RgbaImage};
use rayon::prelude::*;

use super::{
    error::FfmpegError, render_settings::RenderSettings, Encoder, FromFfmpegMessage, ToFfmpegMessage, UpscaleTarget,
    VideoInfo,
};
use crate::{
    font,
    osd::{self, OsdOptions},
    overlay::{overlay_osd_cached, overlay_srt_data, FrameOverlayIter},
    srt::{self, SrtOptions},
};

#[tracing::instrument(skip(osd_frames, srt_frames, font_file), err)]
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_wrap
)]
pub fn start_video_render(
    ffmpeg_path: &PathBuf,
    input_video: &PathBuf,
    output_video: &PathBuf,
    osd_frames: Vec<osd::Frame>,
    srt_frames: Vec<srt::SrtFrame>,
    font_file: font::FontFile,
    srt_font: ab_glyph::FontArc,
    osd_options: &OsdOptions,
    srt_options: &SrtOptions,
    video_info: &VideoInfo,
    render_settings: &RenderSettings,
) -> Result<(Sender<ToFfmpegMessage>, Receiver<FromFfmpegMessage>), FfmpegError> {
    let mut decoder_process = spawn_decoder(
        ffmpeg_path,
        input_video,
        render_settings.encoder.hardware,
        video_info.frame_rate,
    )?;

    let mut encoder_process = spawn_encoder(
        ffmpeg_path,
        video_info.width,
        video_info.height,
        video_info.frame_rate,
        render_settings.bitrate_mbps,
        &render_settings.encoder,
        output_video,
        render_settings.upscale,
        render_settings.pad_4_3_to_16_9,
    )?;

    // Channels to communicate with ffmpeg handler thread
    let (from_ffmpeg_tx, from_ffmpeg_rx) = crossbeam_channel::unbounded();
    let (to_ffmpeg_tx, to_ffmpeg_rx) = crossbeam_channel::unbounded();

    let font_file = Arc::new(font_file);
    // ab_glyph::FontArc is already an internal Arc — no extra wrapping needed.
    let osd_options = Arc::new(osd_options.clone());
    let srt_options = Arc::new(srt_options.clone());
    let chroma_key_rgba = if render_settings.use_chroma_key {
        Some(Rgba([
            (render_settings.chroma_key[0] * 255.0) as u8,
            (render_settings.chroma_key[1] * 255.0) as u8,
            (render_settings.chroma_key[2] * 255.0) as u8,
            255,
        ]))
    } else {
        None
    };
    let pad_4_3_to_16_9 = render_settings.pad_4_3_to_16_9;

    // Iterator over decoded video and OSD synchronization
    let frame_overlay_iter = FrameOverlayIter::new(
        decoder_process
            .iter()
            .expect("Failed to create `FfmpegIterator` for decoder"),
        decoder_process,
        osd_frames,
        srt_frames,
        osd_options.osd_playback_speed_factor,
        from_ffmpeg_tx.clone(),
        to_ffmpeg_rx,
    );

    // Channel for parallel processed frames
    let (processed_tx, processed_rx) = crossbeam_channel::bounded::<(usize, Vec<u8>)>(32);

    // Spawn the parallel worker pool driver
    thread::Builder::new()
        .name("Parallel Render Driver".into())
        .spawn(move || {
            frame_overlay_iter
                .enumerate()
                .par_bridge()
                .map_with(
                    HashMap::new(), // Thread-local glyph cache
                    |glyph_cache, (i, render_data)| {
                        let mut video_frame = render_data.video_frame;

                        let mut frame_image = if let Some(chroma_key) = chroma_key_rgba {
                            RgbaImage::from_pixel(video_frame.width, video_frame.height, chroma_key)
                        } else {
                            RgbaImage::from_raw(video_frame.width, video_frame.height, video_frame.data).unwrap()
                        };

                        // Internal letterboxing
                        let is_4_3 = (video_frame.width as f32 / video_frame.height as f32) < 1.5;
                        let mut x_offset = 0;
                        if pad_4_3_to_16_9 && is_4_3 {
                            let final_width = video_frame.height * 16 / 9;
                            let mut padded_image =
                                RgbaImage::from_pixel(final_width, video_frame.height, Rgba([0, 0, 0, 255]));
                            x_offset = (final_width - video_frame.width) / 2;
                            image::imageops::overlay(&mut padded_image, &frame_image, i64::from(x_offset), 0);
                            frame_image = padded_image;
                            video_frame.width = final_width;
                        }

                        overlay_osd_cached(
                            &mut frame_image,
                            &render_data.osd_frame,
                            &font_file,
                            &osd_options,
                            (x_offset as i32, 0),
                            glyph_cache,
                        );

                        if let Some(current_srt_frame) = &render_data.srt_frame {
                            if let Some(srt_data) = &current_srt_frame.data {
                                overlay_srt_data(
                                    &mut frame_image,
                                    srt_data,
                                    &srt_font,
                                    &srt_options,
                                    (x_offset as i32, 0),
                                );
                            }
                        }

                        (i, frame_image.into_raw())
                    },
                )
                .for_each(|result| {
                    if processed_tx.send(result).is_err() {
                        // Receiver closed, stop processing
                    }
                });
        })?;

    // On another thread read processed frames and write to encoder
    let mut encoder_stdin = encoder_process.take_stdin().expect("Failed to get `stdin` for encoder");
    thread::Builder::new()
        .name("Decoder handler".into())
        .spawn(move || {
            tracing::info_span!("Decoder handler thread").in_scope(|| {
                let mut buffer = BTreeMap::new();
                let mut next_idx = 0;

                for (idx, data) in processed_rx {
                    buffer.insert(idx, data);
                    while let Some(data) = buffer.remove(&next_idx) {
                        if let Err(e) = encoder_stdin.write_all(&data) {
                            tracing::error!("Failed to write to encoder stdin: {}", e);
                            return; // Stop if encoder error
                        }
                        next_idx += 1;
                    }
                }
            });
        })
        .expect("Failed to spawn decoder handler thread");

    // On yet another thread run the encoder to completion
    thread::Builder::new()
        .name("Encoder handler".into())
        .spawn(move || {
            tracing::info_span!("Encoder handler thread").in_scope(|| {
                encoder_process
                    .iter()
                    .expect("Failed to create encoder iterator")
                    .for_each(|event| {
                        handle_encoder_events(event, &from_ffmpeg_tx);
                    });
            });
        })
        .expect("Failed to spawn encoder handler thread");

    Ok((to_ffmpeg_tx, from_ffmpeg_rx))
}

#[tracing::instrument(skip(ffmpeg_path))]
pub fn spawn_decoder(
    ffmpeg_path: &PathBuf,
    input_video: &PathBuf,
    use_hwaccel: bool,
    frame_rate: f32,
) -> Result<FfmpegChild, FfmpegError> {
    let mut cmd = FfmpegCommand::new_with_path(ffmpeg_path);
    cmd.create_no_window();
    if use_hwaccel {
        cmd.args(["-hwaccel", "auto"]);
    }
    cmd.input(input_video.to_str().unwrap()).args([
        "-f",
        "rawvideo",
        "-pix_fmt",
        "rgba",
        "-r",
        &frame_rate.to_string(),
        "-",
    ]);
    let decoder = cmd.spawn()?;
    Ok(decoder)
}

#[tracing::instrument(skip(ffmpeg_path))]
#[allow(clippy::cast_precision_loss)]
pub fn spawn_encoder(
    ffmpeg_path: &PathBuf,
    width: u32,
    height: u32,
    frame_rate: f32,
    bitrate_mbps: u32,
    video_encoder: &Encoder,
    output_video: &PathBuf,
    upscale: UpscaleTarget,
    pad_4_3_to_16_9: bool,
) -> Result<FfmpegChild, FfmpegError> {
    let mut encoder_command = FfmpegCommand::new_with_path(ffmpeg_path);

    let is_4_3 = (width as f32 / height as f32) < 1.5;
    let (final_width, final_height) = if pad_4_3_to_16_9 && is_4_3 {
        // Calculate 16:9 width based on height
        (height * 16 / 9, height)
    } else {
        (width, height)
    };

    encoder_command
        .create_no_window()
        .format("rawvideo")
        .pix_fmt("rgba")
        .size(final_width, final_height)
        .rate(frame_rate)
        .input("-");

    let mut filters = Vec::new();

    match upscale {
        UpscaleTarget::P1440 => {
            filters.push("scale=2560x1440:flags=bicubic".to_string());
        }
        UpscaleTarget::P2160 => {
            filters.push("scale=3840x2160:flags=bicubic".to_string());
        }
        UpscaleTarget::None => {}
    }

    if !filters.is_empty() {
        encoder_command.args(["-vf", &filters.join(",")]);
    }

    encoder_command
        .pix_fmt("yuv420p")
        .codec_video(&video_encoder.name)
        .args(["-b:v", &format!("{bitrate_mbps}M")])
        .args(&video_encoder.extra_args)
        .overwrite()
        .output(output_video.to_str().unwrap());

    let encoder = encoder_command.spawn()?;
    Ok(encoder)
}

fn manual_parse_progress(log_line: &str) -> Option<ffmpeg_sidecar::event::FfmpegProgress> {
    if !log_line.contains("frame=") || !log_line.contains("fps=") {
        return None;
    }

    let frame = parse_val(log_line, "frame=")?.parse().ok()?;
    let fps = parse_val(log_line, "fps=")?.parse().ok()?;
    let speed = parse_val(log_line, "speed=")?.parse().ok()?;

    Some(ffmpeg_sidecar::event::FfmpegProgress {
        frame,
        fps,
        speed,
        q: 0.0,
        size_kb: parse_val(log_line, "size=").and_then(|s| s.parse().ok()).unwrap_or(0),
        time: parse_val(log_line, "time=").unwrap_or_default(),
        bitrate_kbps: parse_val(log_line, "bitrate=")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0.0),
        raw_log_message: log_line.to_string(),
    })
}

fn parse_val(s: &str, key: &str) -> Option<String> {
    let start = s.find(key)? + key.len();
    let rest = &s[start..];
    let mut result = String::new();
    let mut found_content = false;
    for c in rest.chars() {
        if c.is_whitespace() {
            if found_content {
                break;
            }
            continue;
        }
        if c.is_ascii_digit() || c == '.' || c == '-' {
            result.push(c);
            found_content = true;
        } else if found_content {
            break;
        }
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Handle events from the encoder process.
///
/// # Panics
/// Panics if the `ffmpeg_sender` channel is closed.
fn handle_encoder_events(ffmpeg_event: FfmpegEvent, ffmpeg_sender: &Sender<FromFfmpegMessage>) {
    match ffmpeg_event {
        FfmpegEvent::Progress(p) => {
            ffmpeg_sender.send(FromFfmpegMessage::Progress(p)).unwrap();
        }
        FfmpegEvent::Log(_level, e) => {
            if let Some(p) = manual_parse_progress(&e) {
                ffmpeg_sender.send(FromFfmpegMessage::Progress(p)).ok();
            }
            if e.contains("Error initializing output stream") || e.contains("[error] Cannot load") {
                tracing::info!("Sending EncoderFatalError message: {}", &e);
                ffmpeg_sender.send(FromFfmpegMessage::EncoderFatalError(e)).unwrap();
            }
        }
        FfmpegEvent::LogEOF => {
            tracing::info!("ffmpeg encoder EOF reached");
            tracing::info!("Sending EncoderFinished message");
            ffmpeg_sender.send(FromFfmpegMessage::EncoderFinished).unwrap();
        }
        _ => {}
    }
}

/// Handle events from the decoder process.
///
/// # Panics
/// Panics if the `ffmpeg_sender` channel is closed.
pub fn handle_decoder_events(ffmpeg_event: FfmpegEvent, ffmpeg_sender: &Sender<FromFfmpegMessage>) {
    match ffmpeg_event {
        FfmpegEvent::Progress(p) => {
            ffmpeg_sender.send(FromFfmpegMessage::Progress(p)).unwrap();
        }
        FfmpegEvent::Log(level, e) => {
            if let Some(p) = manual_parse_progress(&e) {
                ffmpeg_sender.send(FromFfmpegMessage::Progress(p)).ok();
            }
            match level {
                LogLevel::Fatal => {
                    tracing::error!("ffmpeg fatal error: {}", &e);
                    ffmpeg_sender.send(FromFfmpegMessage::DecoderFatalError(e)).unwrap();
                }
                LogLevel::Warning | LogLevel::Error => {
                    tracing::warn!("ffmpeg log: {}", e);
                }
                _ => {}
            }
        }
        FfmpegEvent::Done | FfmpegEvent::LogEOF => {
            ffmpeg_sender.send(FromFfmpegMessage::DecoderFinished).unwrap();
        }
        _ => {}
    }
}
