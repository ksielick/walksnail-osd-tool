use ffmpeg_sidecar::event::FfmpegProgress;

#[derive(Debug)]
pub enum FromFfmpegMessage {
    DecoderFatalError(String),
    EncoderFatalError(String),
    Progress(FfmpegProgress),
    DecoderFinished,
    EncoderFinished,
}

#[derive(Debug)]
pub enum ToFfmpegMessage {
    AbortRender,
}
