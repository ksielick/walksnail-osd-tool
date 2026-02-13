use image::{Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};

use crate::srt::{SrtFrameData, SrtOptions};

#[inline]
pub fn overlay_srt_data(
    image: &mut RgbaImage,
    srt_data: &SrtFrameData,
    font: &rusttype::Font,
    srt_options: &SrtOptions,
) {
    let mut segments = Vec::new();

    if srt_options.show_time {
        let minutes = srt_data.flight_time / 60;
        let seconds = srt_data.flight_time % 60;
        segments.push(format!("Time:{}:{:0>2}", minutes, seconds));
    }

    if srt_options.show_sbat {
        segments.push(format!("SBat:{: >4.1}V", srt_data.sky_bat));
    }

    if srt_options.show_gbat {
        segments.push(format!("GBat:{: >4.1}V", srt_data.ground_bat));
    }

    if srt_options.show_signal {
        segments.push(format!("Signal:{}", srt_data.signal));
    }

    if srt_options.show_channel {
        segments.push(format!("CH:{}", srt_data.channel));
    }

    if srt_options.show_hz {
        if let Some(hz) = srt_data.hz {
            segments.push(format!("Hz:{}", hz));
        }
    }

    if srt_options.show_sp {
        if let Some(sp) = srt_data.sp {
            segments.push(format!("Sp:{}", sp));
        }
    }

    if srt_options.show_gp {
        if let Some(gp) = srt_data.gp {
            segments.push(format!("Gp:{}", gp));
        }
    }

    if srt_options.show_latency {
        segments.push(format!("Latency:{: >3}ms", srt_data.latency));
    }

    if srt_options.show_bitrate {
        segments.push(format!("Bitrate:{: >4.1}Mbps", srt_data.bitrate_mbps));
    }

    if srt_options.show_distance {
        let distance = srt_data.distance;
        if distance > 999 {
            let km = distance as f32 / 1000.0;
            segments.push(format!("Distance:{:.2}km", km));
        } else {
            segments.push(format!("Distance:{: >3}m", srt_data.distance));
        }
    }

    if segments.is_empty() {
        return;
    }

    let image_dimensions = image.dimensions();
    let x_pos_pct = srt_options.position.x / 100.0;
    let y_pos_pct = srt_options.position.y / 100.0;
    let scale_val = srt_options.scale / 1080.0 * image_dimensions.1 as f32;
    let scale = rusttype::Scale::uniform(scale_val);

    let x_start = (x_pos_pct * image_dimensions.0 as f32) as i32;
    let y_start = (y_pos_pct * image_dimensions.1 as f32) as i32;

    let padding_px = (10.0 / 1080.0 * image_dimensions.1 as f32) as i32;
    let max_width = (image_dimensions.0 as i32 - x_start - padding_px).max(100);

    let mut lines = Vec::new();
    let mut current_line = String::new();

    let separator = "  ";

    for segment in segments {
        let potential_line = if current_line.is_empty() {
            segment.clone()
        } else {
            format!("{}{}{}", current_line, separator, segment)
        };

        let (total_width, _) = text_size(scale, font, &potential_line);

        if total_width > max_width && !current_line.is_empty() {
            lines.push(current_line);
            current_line = segment;
        } else {
            current_line = potential_line;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    let line_height = (scale_val * 1.2) as i32;
    let text_color = Rgba([240u8, 240u8, 240u8, 240u8]);
    let shadow_color = Rgba([0u8, 0u8, 0u8, 180u8]);

    for (i, line) in lines.iter().enumerate() {
        let x = x_start;
        let y = y_start + (i as i32 * line_height);

        // Draw shadow (1px offset)
        draw_text_mut(image, shadow_color, x + 1, y + 1, scale, font, line);

        // Draw main text
        draw_text_mut(image, text_color, x, y, scale, font, line);
    }
}
