use std::collections::HashMap;

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

/// Compute the scaled glyph for a given character index, or return None if the
/// character doesn't exist in the font.
#[inline]
fn get_scaled_glyph(
    font: &font::FontFile,
    character_index: u16,
    base_character_size: &CharacterSize,
    scaled_width: u32,
    scaled_height: u32,
) -> Option<RgbaImage> {
    font.get_character(character_index as usize).map(|character_image| {
        if scaled_width != base_character_size.width() || scaled_height != base_character_size.height() {
            resize(character_image, scaled_width, scaled_height, FilterType::Lanczos3)
        } else {
            character_image.clone()
        }
    })
}

/// Overlay OSD glyphs onto a frame image (single-use, no caching).
/// Used by the OSD preview path where only a single frame is rendered.
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
        if let Some(scaled_image) =
            get_scaled_glyph(font, character.index, &base_character_size, scaled_width, scaled_height)
        {
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

/// Overlay OSD glyphs onto a frame image with a glyph cache.
/// The cache persists across frames so each unique glyph index is resized only once.
#[inline]
pub fn overlay_osd_cached(
    image: &mut RgbaImage,
    osd_frame: &osd::Frame,
    font: &font::FontFile,
    osd_options: &OsdOptions,
    offset: (i32, i32),
    glyph_cache: &mut HashMap<u16, RgbaImage>,
) {
    let base_character_size = get_character_size(image.width(), image.height());
    let scale_factor = osd_options.scale / 100.0;
    let scaled_width = (base_character_size.width() as f32 * scale_factor).round() as u32;
    let scaled_height = (base_character_size.height() as f32 * scale_factor).round() as u32;

    for character in &osd_frame.glyphs {
        if character.index == 0 || osd_options.get_mask(&character.grid_position) {
            continue;
        }

        let scaled_image = glyph_cache.entry(character.index).or_insert_with(|| {
            get_scaled_glyph(font, character.index, &base_character_size, scaled_width, scaled_height)
                .unwrap_or_else(|| RgbaImage::new(scaled_width, scaled_height))
        });

        let grid_position = &character.grid_position;
        overlay(
            image,
            scaled_image,
            (grid_position.x as i32 * scaled_width as i32 + osd_options.position.x + offset.0).into(),
            (grid_position.y as i32 * scaled_height as i32 + osd_options.position.y + offset.1).into(),
        );
    }
}
