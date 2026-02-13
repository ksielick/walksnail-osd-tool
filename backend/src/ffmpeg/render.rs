use std::{io::Write, path::PathBuf, thread};

use crossbeam_channel::{Receiver, Sender};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    command::FfmpegCommand,
    event::{FfmpegEvent, LogLevel},
};

use super::{
    error::FfmpegError, render_settings::RenderSettings, Encoder, FromFfmpegMessage, ToFfmpegMessage, UpscaleTarget,
    VideoInfo,
};
use crate::{
    font,
    osd::{self, OsdOptions},
    overlay::FrameOverlayIter,
    srt::{self, SrtOptions},
};

#[tracing::instrument(skip(osd_frames, srt_frames, font_file), err)]
pub fn start_video_render(
    ffmpeg_path: &PathBuf,
    input_video: &PathBuf,
    output_video: &PathBuf,
    osd_frames: Vec<osd::Frame>,
    srt_frames: Vec<srt::SrtFrame>,
    font_file: font::FontFile,
    srt_font: rusttype::Font<'static>,
    osd_options: &OsdOptions,
    srt_options: &SrtOptions,
    video_info: &VideoInfo,
    render_settings: &RenderSettings,
) -> Result<(Sender<ToFfmpegMessage>, Receiver<FromFfmpegMessage>), FfmpegError> {
    let mut decoder_process = spawn_decoder(ffmpeg_path, input_video)?;

    let mut encoder_process = spawn_encoder(
        ffmpeg_path,
        video_info.width,
        video_info.height,
        video_info.frame_rate,
        render_settings.bitrate_mbps,
        &render_settings.encoder,
        output_video,
        render_settings.upscale,
    )?;

    // Channels to communicate with ffmpeg handler thread
    let (from_ffmpeg_tx, from_ffmpeg_rx) = crossbeam_channel::unbounded();
    let (to_ffmpeg_tx, to_ffmpeg_rx) = crossbeam_channel::unbounded();

    // Iterator over decoded video and OSD frames
    let frame_overlay_iter = FrameOverlayIter::new(
        decoder_process
            .iter()
            .expect("Failed to create `FfmpegIterator` for decoder"),
        decoder_process,
        osd_frames,
        srt_frames,
        font_file,
        srt_font,
        osd_options,
        srt_options,
        from_ffmpeg_tx.clone(),
        to_ffmpeg_rx,
        if render_settings.use_chroma_key {
            Some(render_settings.chroma_key)
        } else {
            None
        },
    );

    // On another thread run the decoder iterator to completion and feed the output to the encoder's stdin
    let mut encoder_stdin = encoder_process.take_stdin().expect("Failed to get `stdin` for encoder");
    thread::Builder::new()
        .name("Decoder handler".into())
        .spawn(move || {
            tracing::info_span!("Decoder handler thread").in_scope(|| {
                frame_overlay_iter.for_each(|f| {
                    if let Err(e) = encoder_stdin.write_all(&f.data) {
                        tracing::error!("Failed to write to encoder stdin: {}", e);
                    }
                });
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
pub fn spawn_decoder(ffmpeg_path: &PathBuf, input_video: &PathBuf) -> Result<FfmpegChild, FfmpegError> {
    let decoder = FfmpegCommand::new_with_path(ffmpeg_path)
        .create_no_window()
        .input(input_video.to_str().unwrap())
        .args(["-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .spawn()?;
    Ok(decoder)
}

#[tracing::instrument(skip(ffmpeg_path))]
pub fn spawn_encoder(
    ffmpeg_path: &PathBuf,
    width: u32,
    height: u32,
    frame_rate: f32,
    bitrate_mbps: u32,
    video_encoder: &Encoder,
    output_video: &PathBuf,
    upscale: UpscaleTarget,
) -> Result<FfmpegChild, FfmpegError> {
    let mut encoder_command = FfmpegCommand::new_with_path(ffmpeg_path);

    encoder_command
        .create_no_window()
        .format("rawvideo")
        .pix_fmt("rgba")
        .size(width, height)
        .rate(frame_rate)
        .input("-");

    match upscale {
        UpscaleTarget::P1440 => {
            encoder_command.args(["-vf", "scale=2560x1440:flags=bicubic"]);
        }
        UpscaleTarget::P2160 => {
            encoder_command.args(["-vf", "scale=3840x2160:flags=bicubic"]);
        }
        UpscaleTarget::None => {}
    }

    encoder_command
        .pix_fmt("yuv420p")
        .codec_video(&video_encoder.name)
        .args(["-b:v", &format!("{}M", bitrate_mbps)])
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
        if c.is_digit(10) || c == '.' || c == '-' {
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
