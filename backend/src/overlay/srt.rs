use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};

use crate::srt::{SrtFrameData, SrtOptions};

#[inline]
#[allow(
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap
)]
pub fn overlay_srt_data(
    image: &mut RgbaImage,
    srt_data: &SrtFrameData,
    font: &impl ab_glyph::Font,
    srt_options: &SrtOptions,
    offset: (i32, i32),
) {
    let mut segments = Vec::new();

    if srt_options.show_time {
        if let Some(flight_time) = srt_data.flight_time {
            let minutes = flight_time / 60;
            let seconds = flight_time % 60;
            segments.push(format!("Time:{minutes}:{seconds:0>2}"));
        }
    }

    if srt_options.show_sbat {
        if let Some(sky_bat) = srt_data.sky_bat {
            segments.push(format!("SBat:{sky_bat: >4.1}V"));
        }
    }

    if srt_options.show_gbat {
        if let Some(ground_bat) = srt_data.ground_bat {
            segments.push(format!("GBat:{ground_bat: >4.1}V"));
        }
    }

    if srt_options.show_signal {
        if let Some(signal) = srt_data.signal {
            if !srt_data.is_debug {
                segments.push(format!("Signal:{signal}"));
            }
        }
    }

    if srt_options.show_mcs {
        if let Some(signal) = srt_data.signal {
            if srt_data.is_debug {
                segments.push(format!("MCS:{signal}"));
            }
        }
    }

    if srt_options.show_channel {
        if let Some(channel) = &srt_data.channel {
            segments.push(format!("CH:{channel}"));
        }
    }

    if srt_options.show_hz {
        if let Some(hz) = srt_data.hz {
            segments.push(format!("Hz:{hz}"));
        }
    }

    if srt_options.show_sp {
        if let Some(sp) = srt_data.sp {
            segments.push(format!("Sp:{sp}"));
        }
    }

    if srt_options.show_gp {
        if let Some(gp) = srt_data.gp {
            segments.push(format!("Gp:{gp}"));
        }
    }

    if srt_options.show_air_temp {
        if let Some(temp) = srt_data.air_temp {
            segments.push(format!("AirTemp:{temp}"));
        }
    }

    if srt_options.show_gnd_temp {
        if let Some(temp) = srt_data.gnd_temp {
            segments.push(format!("GndTemp:{temp}"));
        }
    }

    if srt_options.show_stemp {
        if let Some(temp) = srt_data.stemp {
            segments.push(format!("Stemp:{temp}"));
        }
    }

    if srt_options.show_gtemp {
        if let Some(temp) = srt_data.gtemp {
            segments.push(format!("Gtemp:{temp}"));
        }
    }

    if srt_options.show_ssnr {
        if let Some(ssnr) = srt_data.ssnr {
            segments.push(format!("SSNR:{ssnr}"));
        }
    }

    if srt_options.show_gsnr {
        if let Some(gsnr) = srt_data.gsnr {
            segments.push(format!("GSNR:{gsnr}"));
        }
    }

    if srt_options.show_serr {
        if let Some(serr) = srt_data.serr {
            segments.push(format!("Serr:{serr}"));
        }
    }

    if srt_options.show_gerr {
        if let Some(gerr) = srt_data.gerr {
            segments.push(format!("Gerr:{gerr}"));
        }
    }

    if srt_options.show_sty_mode {
        if let Some(mode) = srt_data.sty_mode {
            segments.push(format!("STYMode:{mode}"));
        }
    }

    if srt_options.show_latency {
        if let Some(latency) = srt_data.latency {
            segments.push(format!("Latency:{latency: >3}ms"));
        }
    }

    if srt_options.show_bitrate {
        if let Some(bitrate_mbps) = srt_data.bitrate_mbps {
            segments.push(format!("Bitrate:{bitrate_mbps: >4.1}Mbps"));
        }
    }

    if srt_options.show_distance {
        if let Some(distance) = srt_data.distance {
            if distance > 999 {
                let km = distance as f32 / 1000.0;
                segments.push(format!("Distance:{km:.2}km"));
            } else {
                segments.push(format!("Distance:{distance: >3}m"));
            }
        }
    }

    if segments.is_empty() {
        return;
    }

    let image_dimensions = image.dimensions();
    let x_pos_pct = srt_options.position.x / 100.0;
    let y_pos_pct = srt_options.position.y / 100.0;
    let scale_val = srt_options.scale / 1080.0 * image_dimensions.1 as f32;
    let scale = ab_glyph::PxScale::from(scale_val);

    let x_start = (x_pos_pct * image_dimensions.0 as f32) as i32;
    let y_start = (y_pos_pct * image_dimensions.1 as f32) as i32;

    let padding_px = (10.0 / 1080.0 * image_dimensions.1 as f32) as i32;
    #[allow(clippy::cast_possible_wrap)]
    let max_width = (image_dimensions.0 as i32 - x_start - padding_px).max(100);

    let mut lines = Vec::new();
    let mut current_line = String::new();

    let separator = "  ";

    for segment in segments {
        let potential_line = if current_line.is_empty() {
            segment.clone()
        } else {
            format!("{current_line}{separator}{segment}")
        };

        let (total_width, _) = text_size(scale, font, &potential_line);

        #[allow(clippy::cast_sign_loss)]
        if total_width > max_width as u32 && !current_line.is_empty() {
            lines.push(current_line);
            current_line = segment;
        } else {
            current_line = potential_line;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    #[allow(clippy::cast_possible_truncation)]
    let line_height = (scale_val * 1.2) as i32;
    let text_color = Rgba([240u8, 240u8, 240u8, 240u8]);
    let shadow_color = Rgba([0u8, 0u8, 0u8, 180u8]);

    for (i, line) in lines.iter().enumerate() {
        let x = x_start + offset.0;
        let y = y_start + (i as i32 * line_height) + offset.1;

        // Draw shadow (1px offset)
        draw_text_mut(image, shadow_color, x + 1, y + 1, scale, font, line);

        // Draw main text
        draw_text_mut(image, text_color, x, y, scale, font, line);
    }
}
