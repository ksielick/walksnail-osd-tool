use std::{path::PathBuf, time::Duration};

use derivative::Derivative;

use super::{
    error::SrtFileError,
    frame::{AscentSrtFrameData, SrtDebugFrameData, SrtFrame},
    SrtFrameData,
};

#[derive(Derivative)]
#[derivative(Debug)]
pub struct SrtFile {
    pub file_path: PathBuf,
    pub has_signal: bool,
    pub has_channel: bool,
    pub has_flight_time: bool,
    pub has_sky_bat: bool,
    pub has_ground_bat: bool,
    pub has_latency: bool,
    pub has_bitrate: bool,
    pub has_distance: bool,
    pub has_hz: bool,
    pub has_sp: bool,
    pub has_gp: bool,
    pub has_air_temp: bool,
    pub has_gnd_temp: bool,
    pub has_sty_mode: bool,
    pub has_debug: bool,
    pub duration: Duration,
    #[derivative(Debug = "ignore")]
    pub frames: Vec<SrtFrame>,
}

impl SrtFile {
    #[tracing::instrument(ret, err)]
    pub fn open(path: PathBuf) -> Result<Self, SrtFileError> {
        let mut has_signal = false;
        let mut has_channel = false;
        let mut has_flight_time = false;
        let mut has_sky_bat = false;
        let mut has_ground_bat = false;
        let mut has_latency = false;
        let mut has_bitrate = false;
        let mut has_distance = false;
        let mut has_hz = false;
        let mut has_sp = false;
        let mut has_gp = false;
        let mut has_air_temp = false;
        let mut has_gnd_temp = false;
        let mut has_sty_mode = false;
        let mut has_debug = false;

        let srt_frames = srtparse::from_file(&path)?
            .iter()
            .map(|i| -> Result<SrtFrame, SrtFileError> {
                let debug_data = i.text.parse::<SrtDebugFrameData>().ok();
                let mut data = i.text.parse::<AscentSrtFrameData>().ok().map(|a| a.into());

                if data.is_none() {
                    data = i.text.parse::<SrtFrameData>().ok();
                }

                if debug_data.is_some() {
                    has_debug = true;
                }
                if let Some(data) = &data {
                    has_signal |= data.signal.is_some();
                    has_channel |= data.channel.is_some();
                    has_flight_time |= data.flight_time.is_some();
                    has_sky_bat |= data.sky_bat.is_some();
                    has_ground_bat |= data.ground_bat.is_some();
                    has_latency |= data.latency.is_some();
                    has_bitrate |= data.bitrate_mbps.is_some();
                    has_distance |= data.distance.is_some();
                    has_hz |= data.hz.is_some();
                    has_sp |= data.sp.is_some();
                    has_gp |= data.gp.is_some();
                    has_air_temp |= data.air_temp.is_some();
                    has_gnd_temp |= data.gnd_temp.is_some();
                    has_sty_mode |= data.sty_mode.is_some();
                }

                Ok(SrtFrame {
                    start_time_secs: i.start_time.into_duration().as_secs_f32(),
                    end_time_secs: i.end_time.into_duration().as_secs_f32(),
                    data,
                    debug_data,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        let duration = Duration::from_secs_f32(srt_frames.last().unwrap().end_time_secs);

        Ok(Self {
            file_path: path,
            has_signal,
            has_channel,
            has_flight_time,
            has_sky_bat,
            has_ground_bat,
            has_latency,
            has_bitrate,
            has_distance,
            has_hz,
            has_sp,
            has_gp,
            has_air_temp,
            has_gnd_temp,
            has_sty_mode,
            has_debug,
            duration,
            frames: srt_frames,
        })
    }
}
