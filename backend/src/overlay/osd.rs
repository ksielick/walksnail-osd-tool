use image::{
    imageops::{overlay, resize, FilterType},
    RgbaImage,
};

use crate::{
    font::{self, CharacterSize},
    osd::{self, OsdOptions},
};

pub fn get_character_size(width: u32, height: u32) -> CharacterSize {
    let is_4_3 = (width as f32 / height as f32) < 1.5;
    match height {
        540 => CharacterSize::Race,
        720 => CharacterSize::Small,
        1080 => {
            if is_4_3 {
                CharacterSize::Small
            } else {
                CharacterSize::Large
            }
        }
        1440 => {
            if is_4_3 {
                CharacterSize::Large
            } else {
                CharacterSize::XLarge
            }
        }
        2160 => CharacterSize::Ultra,
        _ => CharacterSize::Large,
    }
}

#[inline]
pub fn overlay_osd(
    image: &mut RgbaImage,
    osd_frame: &osd::Frame,
    font: &font::FontFile,
    osd_options: &OsdOptions,
    offset: (i32, i32),
) {
    let base_character_size = get_character_size(image.width(), image.height());
    let scale_factor = osd_options.scale / 100.0;
    let scaled_width = (base_character_size.width() as f32 * scale_factor).round() as u32;
    let scaled_height = (base_character_size.height() as f32 * scale_factor).round() as u32;

    for character in &osd_frame.glyphs {
        if character.index == 0 || osd_options.get_mask(&character.grid_position) {
            continue;
        }
        if let Some(character_image) = font.get_character(character.index as usize, &base_character_size) {
            let scaled_image =
                if scaled_width != base_character_size.width() || scaled_height != base_character_size.height() {
                    resize(&character_image, scaled_width, scaled_height, FilterType::Lanczos3)
                } else {
                    character_image
                };
            let grid_position = &character.grid_position;
            overlay(
                image,
                &scaled_image,
                (grid_position.x as i32 * scaled_width as i32 + osd_options.position.x + offset.0).into(),
                (grid_position.y as i32 * scaled_height as i32 + osd_options.position.y + offset.1).into(),
            )
        }
    }
}
