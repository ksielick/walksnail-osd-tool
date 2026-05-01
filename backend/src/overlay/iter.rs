pub use std::{iter::Peekable, vec::IntoIter};

use crossbeam_channel::{Receiver, Sender};
use ffmpeg_sidecar::{
    child::FfmpegChild,
    event::{FfmpegEvent, LogLevel, OutputVideoFrame},
    iter::FfmpegIterator,
};

use crate::{
    ffmpeg::{FromFfmpegMessage, ToFfmpegMessage},
    osd, srt,
};

#[allow(clippy::struct_field_names)]
pub struct FrameRenderData {
    pub video_frame: OutputVideoFrame,
    pub osd_frame: Option<osd::Frame>,
    pub srt_frame: Option<srt::SrtFrame>,
}

pub struct FrameOverlayIter {
    decoder_iter: FfmpegIterator,
    decoder_process: FfmpegChild,
    osd_frames_iter: Peekable<IntoIter<osd::Frame>>,
    srt_frames_iter: Peekable<IntoIter<srt::SrtFrame>>,
    osd_playback_speed_factor: f32,
    current_osd_frame: Option<osd::Frame>,
    current_srt_frame: Option<srt::SrtFrame>,
    ffmpeg_sender: Sender<FromFfmpegMessage>,
    ffmpeg_receiver: Receiver<ToFfmpegMessage>,
}

impl FrameOverlayIter {
    #[tracing::instrument(skip(decoder_iter, decoder_process, osd_frames), level = "debug")]
    pub fn new(
        decoder_iter: FfmpegIterator,
        decoder_process: FfmpegChild,
        osd_frames: Vec<osd::Frame>,
        srt_frames: Vec<srt::SrtFrame>,
        osd_playback_speed_factor: f32,
        ffmpeg_sender: Sender<FromFfmpegMessage>,
        ffmpeg_receiver: Receiver<ToFfmpegMessage>,
    ) -> Self {
        let mut osd_frames_iter = osd_frames.into_iter();
        let mut srt_frames_iter = srt_frames.into_iter();
        let first_osd_frame = osd_frames_iter.next();
        let first_srt_frame = srt_frames_iter.next();
        Self {
            decoder_iter,
            decoder_process,
            osd_frames_iter: osd_frames_iter.peekable(),
            srt_frames_iter: srt_frames_iter.peekable(),
            osd_playback_speed_factor,
            current_osd_frame: first_osd_frame,
            current_srt_frame: first_srt_frame,
            ffmpeg_sender,
            ffmpeg_receiver,
        }
    }
}

impl Iterator for FrameOverlayIter {
    type Item = FrameRenderData;

    fn next(&mut self) -> Option<Self::Item> {
        //  On every iteration check if the render should be stopped
        while matches!(self.ffmpeg_receiver.try_recv(), Ok(ToFfmpegMessage::AbortRender)) {
            self.decoder_process.quit().unwrap();
        }

        self.decoder_iter.find_map(|e| match e {
            FfmpegEvent::OutputFrame(video_frame) => {
                // For every video frame check if frame time is later than the next OSD frame time.
                // If so advance the iterator over the OSD frames so we use the correct OSD frame
                // for this video frame
                if let Some(next_osd_frame) = self.osd_frames_iter.peek() {
                    #[allow(clippy::cast_precision_loss)]
                    let next_osd_frame_secs = next_osd_frame.time_millis as f32 / 1000.0;
                    if video_frame.timestamp > next_osd_frame_secs * self.osd_playback_speed_factor {
                        self.current_osd_frame = self.osd_frames_iter.next();
                    }
                }

                if let Some(next_srt_frame) = self.srt_frames_iter.peek() {
                    let next_srt_start_time_secs = next_srt_frame.start_time_secs;
                    if video_frame.timestamp > next_srt_start_time_secs {
                        self.current_srt_frame = self.srt_frames_iter.next();
                    }
                }

                Some(FrameRenderData {
                    video_frame,
                    osd_frame: self.current_osd_frame.clone(),
                    srt_frame: self.current_srt_frame.clone(),
                })
            }
            other_event => {
                // We handle decoder events manually here to suppress Progress updates from the decoder.
                // Since we have a parallel pipeline, the decoder is ahead of the encoder.
                // We only want the UI to show the Encoder's progress (what is actually written).
                match other_event {
                    FfmpegEvent::Log(level, e) => {
                        // Do NOT parse progress from logs here
                        match level {
                            LogLevel::Fatal => {
                                tracing::error!("ffmpeg fatal error: {}", &e);
                                self.ffmpeg_sender
                                    .send(FromFfmpegMessage::DecoderFatalError(e))
                                    .unwrap();
                            }
                            LogLevel::Warning | LogLevel::Error => {
                                tracing::warn!("ffmpeg log: {}", e);
                            }
                            _ => {}
                        }
                    }
                    FfmpegEvent::Done | FfmpegEvent::LogEOF => {
                        self.ffmpeg_sender.send(FromFfmpegMessage::DecoderFinished).unwrap();
                    }
                    _ => {}
                }
                None
            }
        })
    }
}
