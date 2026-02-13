use backend::{
    font,
    osd::{self, OsdOptions},
    overlay::{overlay_osd, overlay_srt_data},
    srt::{self, SrtOptions},
};
use image::RgbaImage;

#[tracing::instrument(skip(osd_frame, srt_frame, font), level = "debug")]
pub fn create_osd_preview(
    width: u32,
    height: u32,
    osd_frame: &osd::Frame,
    srt_frame: Option<&srt::SrtFrame>,
    font: &font::FontFile,
    srt_font: &rusttype::Font,
    osd_options: &OsdOptions,
    srt_options: &SrtOptions,
    pad_4_3_to_16_9: bool,
) -> RgbaImage {
    let is_4_3 = (width as f32 / height as f32) < 1.5;
    let (final_width, final_height, x_offset) = if pad_4_3_to_16_9 && is_4_3 {
        (height * 16 / 9, height, (height * 16 / 9 - width) / 2)
    } else {
        (width, height, 0)
    };

    let mut image = RgbaImage::new(final_width, final_height);

    // If we're not padding, the image is just width x height transparent
    // In preview we probably want to see where the video would be
    // But Render is usually on top of video.
    // For preview, we just want to ensure OSD/SRT are positioned correctly.

    overlay_osd(&mut image, osd_frame, font, osd_options, (x_offset as i32, 0));
    if let Some(srt_frame) = srt_frame {
        if let Some(srt_data) = &srt_frame.data {
            overlay_srt_data(&mut image, srt_data, srt_font, srt_options, (x_offset as i32, 0));
        }
    }

    image
}

#[tracing::instrument(level = "debug")]
pub fn calculate_horizontal_offset(width: u32, osd_frame: &osd::Frame, character_size: &font::CharacterSize) -> i32 {
    let min_x_grid = osd_frame.glyphs.iter().map(|g| g.grid_position.x).min().unwrap();
    let max_x_grid = osd_frame.glyphs.iter().map(|g| g.grid_position.x).max().unwrap();
    let pixel_range = (max_x_grid - min_x_grid + 1) * character_size.width();
    let offset = (width - pixel_range) / 2 - min_x_grid * character_size.width();
    offset as i32
}

#[tracing::instrument(level = "debug")]
pub fn calculate_vertical_offset(height: u32, osd_frame: &osd::Frame, character_size: &font::CharacterSize) -> i32 {
    let min_y_grid = osd_frame.glyphs.iter().map(|g| g.grid_position.y).min().unwrap();
    let max_y_grid = osd_frame.glyphs.iter().map(|g| g.grid_position.y).max().unwrap();
    let pixel_range = (max_y_grid - min_y_grid + 1) * character_size.height();
    let offset = (height - pixel_range) / 2 - min_y_grid * character_size.height();
    offset as i32
}
