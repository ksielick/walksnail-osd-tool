use std::time::Instant;

use backend::{overlay::get_character_size, util::Coordinates};
use egui::{
    vec2, Button, CentralPanel, Checkbox, CollapsingHeader, Color32, CursorIcon, Grid, Image, Rect, RichText,
    ScrollArea, Sense, Slider, Stroke, Ui, Vec2,
};

use crate::{
    util::{separator_with_space, tooltip_text},
    WalksnailOsdTool,
};

impl WalksnailOsdTool {
    pub fn render_central_panel(&mut self, ctx: &egui::Context) {
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.style_mut().spacing.slider_width = self.ui_dimensions.osd_position_sliders_length;

                self.osd_options(ui, ctx);

                separator_with_space(ui, 10.0);

                self.srt_options(ui, ctx);

                separator_with_space(ui, 10.0);

                self.osd_preview(ui, ctx);

                separator_with_space(ui, 10.0);

                self.rendering_options(ui, ctx);
            });
        });
    }

    fn osd_options(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut changed = false;

        CollapsingHeader::new(RichText::new("OSD Options").heading())
            .default_open(true)
            .show_unindented(ui, |ui| {
                if self.artlynk_extraction_promise.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("Scanning for OSD data...").color(Color32::DEBUG_COLOR));
                    });
                    ui.add_space(5.0);
                }
                Grid::new("osd_options")
                    .min_col_width(self.ui_dimensions.options_column1_width)
                    .show(ui, |ui| {
                        ui.label("Horizontal position")
                            .on_hover_text(tooltip_text("Horizontal position of the flight controller OSD (pixels from the left edge of the video)."));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.osd_options.position.x, -500..=700).text("Pixels"))
                                .changed();

                            if ui.button("Center").clicked() {
                                self.auto_center_horizontal();
                                changed |= true;
                            }

                            if ui.button("Reset").clicked() {
                                self.osd_options.position.x = 0;
                                changed |= true;
                            }
                        });
                        ui.end_row();

                        //

                        ui.label("Vertical position")
                            .on_hover_text(tooltip_text("Vertical position of the flight controller OSD (pixels from the top of the video).").small());
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.osd_options.position.y, -200..=700).text("Pixels"))
                                .changed();

                            if ui.button("Center").clicked() {
                                if let (Some(video_info), Some(osd_file), Some(_)) =
                                    (&self.video_info, &self.osd_file, &self.font_file)
                                {
                                    let is_4_3 = (video_info.width as f32 / video_info.height as f32) < 1.5;
                                    let effective_width = if self.render_settings.pad_4_3_to_16_9 && is_4_3 {
                                        video_info.height * 16 / 9
                                    } else {
                                        video_info.width
                                    };
                                    let base_char_size = get_character_size(effective_width, video_info.height);
                                    let scale_factor = self.osd_options.scale / 100.0;
                                    let scaled_char_height = (base_char_size.height() as f32 * scale_factor).round() as i32;

                                    let frame = osd_file
                                        .frames
                                        .get(self.osd_preview.preview_frame as usize - 1)
                                        .unwrap();
                                    let min_y = frame.glyphs.iter().map(|g| g.grid_position.y).min().unwrap() as i32;
                                    let max_y = frame.glyphs.iter().map(|g| g.grid_position.y).max().unwrap() as i32;
                                    let pixel_range = (max_y - min_y + 1) * scaled_char_height;
                                    self.osd_options.position.y = (video_info.height as i32 - pixel_range) / 2 - min_y * scaled_char_height;
                                    changed |= true
                                }
                            }

                            if ui.button("Reset").clicked() {
                                self.osd_options.position.y = 0;
                                changed |= true
                            }
                        });
                        ui.end_row();

                        ui.label("Mask")
                            .on_hover_text(tooltip_text("Click edit to select OSD elements on the preview that should not be rendered on the video. This can be useful to hide GPS coordinates, etc."));
                        ui.horizontal(|ui| {
                            let txt = if !self.osd_preview.mask_edit_mode_enabled || !self.all_files_loaded() {"Edit"} else {"Save"};
                            if ui.add_enabled(self.all_files_loaded(), Button::new(txt))
                                .on_disabled_hover_text(tooltip_text("First load the input files")).clicked() {
                                self.osd_preview.mask_edit_mode_enabled = !self.osd_preview.mask_edit_mode_enabled;
                            }
                            if ui.button("Reset").clicked() {
                                self.osd_options.reset_mask();
                                self.config_changed = Instant::now().into();
                                self.update_osd_preview(ctx);
                            }
                            let masked_positions = self.osd_options.masked_grid_positions.len();
                            ui.label(format!("{masked_positions} positions masked"));
                        });
                        ui.end_row();

                        ui.label("OSD size")
                            .on_hover_text(tooltip_text("Scale of the OSD characters as a percentage. 100% is the default size for the video resolution."));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.osd_options.scale, 50.0..=200.0).fixed_decimals(0).text("%"))
                                .changed();
                            if ui.button("Reset").clicked() {
                                self.osd_options.scale = 100.0;
                                changed |= true;
                            }
                        });
                        ui.end_row();

                        ui.label("Adjust playback speed")
                            .on_hover_text(tooltip_text("Attempt to correct for wrong OSD timestamps in <=32.37.10 firmwares that causes video and OSD to get out of sync."));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Checkbox::without_text(&mut self.osd_options.adjust_playback_speed))
                                .changed()
                        });
                    });
            });

        if changed {
            self.update_osd_preview(ctx);
            self.config_changed = Some(Instant::now());
        }
    }

    fn srt_options(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut changed = false;

        CollapsingHeader::new(RichText::new("SRT Options").heading())
            .default_open(true)
            .show_unindented(ui, |ui| {
                Grid::new("srt_options")
                    .min_col_width(self.ui_dimensions.options_column1_width)
                    .show(ui, |ui| {
                        ui.label("Horizontal position").on_hover_text(tooltip_text(
                            "Horizontal position of the SRT data (% of the total video width from the left edge).",
                        ));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.srt_options.position.x, 0.0..=100.0).fixed_decimals(1))
                                .changed();

                            if ui.button("Reset").clicked() {
                                self.srt_options.position.x = 1.5;
                                changed |= true;
                            }
                        });
                        ui.end_row();

                        ui.label("Vertical position").on_hover_text(tooltip_text(
                            "Vertical position of the SR data (% of video height from the top edge).",
                        ));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.srt_options.position.y, 0.0..=100.0).fixed_decimals(1))
                                .changed();

                            if ui.button("Reset").clicked() {
                                self.srt_options.position.y = 95.0;
                                changed |= true;
                            }
                        });
                        ui.end_row();

                        ui.label("Size")
                            .on_hover_text(tooltip_text("Font size of the SRT data."));
                        ui.horizontal(|ui| {
                            changed |= ui
                                .add(Slider::new(&mut self.srt_options.scale, 10.0..=60.0).fixed_decimals(1))
                                .changed();

                            if ui.button("Reset").clicked() {
                                self.srt_options.scale = 34.0;
                                changed |= true;
                            }
                        });
                        ui.end_row();

                        ui.label("SRT data").on_hover_text(tooltip_text(
                            "Select data from the SRT file to be rendered on the video.",
                        ));
                        let options = &mut self.srt_options;
                        let srt_file = self.srt_file.as_ref();
                        let has_time = srt_file.map(|s| s.has_flight_time).unwrap_or(true);
                        let has_sbat = srt_file.map(|s| s.has_sky_bat).unwrap_or(true);
                        let has_gbat = srt_file.map(|s| s.has_ground_bat).unwrap_or(true);
                        let has_signal = srt_file.map(|s| s.has_signal).unwrap_or(true);
                        let has_latency = srt_file.map(|s| s.has_latency).unwrap_or(true);
                        let has_bitrate = srt_file.map(|s| s.has_bitrate).unwrap_or(true);
                        let has_distance = srt_file.map(|s| s.has_distance).unwrap_or(true);
                        let has_channel = srt_file.map(|s| s.has_channel).unwrap_or(true);
                        let has_hz = srt_file.map(|s| s.has_hz).unwrap_or(false);
                        let has_sp = srt_file.map(|s| s.has_sp).unwrap_or(false);
                        let has_gp = srt_file.map(|s| s.has_gp).unwrap_or(false);
                        let has_air_temp = srt_file.map(|s| s.has_air_temp).unwrap_or(false);
                        let has_gnd_temp = srt_file.map(|s| s.has_gnd_temp).unwrap_or(false);
                        let has_sty_mode = srt_file.map(|s| s.has_sty_mode).unwrap_or(false);

                        ui.horizontal_wrapped(|ui| {
                            if has_time { changed |= ui.checkbox(&mut options.show_time, "FlightTime").changed(); }
                            if has_sbat { changed |= ui.checkbox(&mut options.show_sbat, "SBat").changed(); }
                            if has_gbat { changed |= ui.checkbox(&mut options.show_gbat, "GBat").changed(); }
                            if has_signal { changed |= ui.checkbox(&mut options.show_signal, "Signal").changed(); }
                            if has_latency { changed |= ui.checkbox(&mut options.show_latency, "Latency").changed(); }
                            if has_bitrate { changed |= ui.checkbox(&mut options.show_bitrate, "Bitrate").changed(); }
                            if has_distance { changed |= ui.checkbox(&mut options.show_distance, "Distance").changed(); }
                            if has_channel { changed |= ui.checkbox(&mut options.show_channel, "CH").changed(); }
                            if has_hz { changed |= ui.checkbox(&mut options.show_hz, "Hz").changed(); }
                            if has_sp { changed |= ui.checkbox(&mut options.show_sp, "Sp").changed(); }
                            if has_gp { changed |= ui.checkbox(&mut options.show_gp, "Gp").changed(); }
                            if has_air_temp { changed |= ui.checkbox(&mut options.show_air_temp, "AirTemp").changed(); }
                            if has_gnd_temp { changed |= ui.checkbox(&mut options.show_gnd_temp, "GndTemp").changed(); }
                            if has_sty_mode { changed |= ui.checkbox(&mut options.show_sty_mode, "STYMode").changed(); }
                        });
                        ui.end_row();
                    });
            });

        if changed {
            self.update_osd_preview(ctx);
            self.config_changed = Some(Instant::now());
        }
    }

    fn osd_preview(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        CollapsingHeader::new(RichText::new("Preview").heading())
            .default_open(true)
            .show_unindented(ui, |ui| {
                if let (Some(handle), Some(video_info)) = (&self.osd_preview.texture_handle, &self.video_info) {
                    let padding = 20.0;
                    let preview_width = ui.available_width() - padding;
                    let aspect_ratio = video_info.width as f32 / video_info.height as f32;
                    let preview_height = preview_width / aspect_ratio;
                    let texture_handle = handle.clone();

                    ui.vertical_centered(|ui| {
                        let image =
                            Image::new(&texture_handle).fit_to_exact_size(Vec2::new(preview_width, preview_height));
                        let rect = ui.add(image.bg_fill(Color32::LIGHT_GRAY)).rect;

                        if self.osd_preview.mask_edit_mode_enabled {
                            self.draw_grid(ui, ctx, rect);
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Preview frame").on_hover_text(tooltip_text(
                            "The selected frame is also used for centering the OSD under OSD Options.",
                        ));
                        let preview_frame_slider = ui.add(
                            Slider::new(
                                &mut self.osd_preview.preview_frame,
                                1..=self.osd_file.as_ref().map(|f| f.frame_count).unwrap_or(1),
                            )
                            .smart_aim(false),
                        );
                        if preview_frame_slider.changed() {
                            self.update_osd_preview(ctx);
                        }
                    });
                }
            });
    }

    fn draw_grid(&mut self, ui: &mut Ui, ctx: &egui::Context, image_rect: Rect) {
        let video_width = self.video_info.as_ref().unwrap().width as f32;
        let video_height = self.video_info.as_ref().unwrap().height as f32;

        let top_left = image_rect.left_top();
        let preview_width = image_rect.width();
        let preview_height = image_rect.height();

        let grid_width = preview_width * 0.99375;
        let grid_height = preview_height;
        let cell_width = grid_width / 53.0;
        let cell_height = grid_height / 20.0;

        let painter = ui.painter_at(image_rect);

        let horizontal_offset = self.osd_options.position.x as f32 / video_width * preview_width;
        let vertical_offset = self.osd_options.position.y as f32 / video_height * preview_height;

        let response = ui
            .allocate_rect(image_rect, Sense::click())
            .on_hover_cursor(CursorIcon::Crosshair);

        for i in 0..53 {
            for j in 0..20 {
                let rect = Rect::from_min_size(
                    top_left
                        + vec2(i as f32 * cell_width, j as f32 * cell_height)
                        + vec2(horizontal_offset, vertical_offset),
                    vec2(cell_width, cell_height),
                );

                let grid_position = Coordinates::new(i, j);
                let masked = self.osd_options.get_mask(&grid_position);
                if masked {
                    painter.rect_filled(rect, 0.0, Color32::RED.gamma_multiply(0.5));
                }

                if let Some(hover_pos) = ctx.pointer_hover_pos() {
                    if rect.contains(hover_pos) {
                        painter.rect_filled(rect, 0.0, Color32::RED.gamma_multiply(0.2));
                    }
                }

                if response.clicked() {
                    if let Some(click_pos) = ctx.pointer_interact_pos() {
                        if rect.contains(click_pos) {
                            self.osd_options.toggle_mask(grid_position);
                            self.update_osd_preview(ctx);
                            self.config_changed = Instant::now().into();
                        }
                    }
                }
            }
        }

        let line_stroke = Stroke::new(1.0, Color32::GRAY.gamma_multiply(0.5));

        for i in 0..=53 {
            let x = top_left.x + i as f32 * cell_width + horizontal_offset;
            let y_min = image_rect.y_range().min + vertical_offset;
            let y_max = image_rect.y_range().max + vertical_offset;
            painter.vline(x, y_min..=y_max, line_stroke);
        }
        for i in 0..=20 {
            let x_min = image_rect.x_range().min + horizontal_offset;
            let x_max = image_rect.x_range().max + horizontal_offset;
            let y = top_left.y + i as f32 * cell_height + vertical_offset;
            painter.hline(x_min..=x_max, y, line_stroke);
        }
    }

    fn rendering_options(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let mut changed = false;
        let mut pad_toggled = false;
        CollapsingHeader::new(RichText::new("Rendering Options").heading())
            .default_open(true)
            .show_unindented(ui, |ui| {
                let selectable_encoders = self
                    .encoders
                    .iter()
                    .filter(|e| self.render_settings.show_undetected_encoders || e.detected)
                    .collect::<Vec<_>>();

                Grid::new("render_options")
                    .min_col_width(self.ui_dimensions.options_column1_width)
                    .show(ui, |ui| {
                        ui.label("Encoder")
                            .on_hover_text(tooltip_text("Encoder used for rendering. In some cases not all available encoders are detected. Check the box to also show these."));
                        ui.horizontal(|ui| {
                            let selection = egui::ComboBox::from_id_source("encoder").width(350.0).show_index(
                                ui,
                                &mut self.render_settings.selected_encoder_idx,
                                selectable_encoders.len(),
                                |i| {
                                    selectable_encoders
                                        .get(i)
                                        .map(|e| e.to_string())
                                        .unwrap_or("None".to_string())
                                },
                            );
                            if selection.changed() {
                                // This is a little hacky but it's nice to have a single struct that keeps track of all render settings
                                self.render_settings.encoder =
                                    (*selectable_encoders.get(self.render_settings.selected_encoder_idx).unwrap()).clone();
                                changed |= true;
                            }

                            if ui
                                .add(Checkbox::without_text(&mut self.render_settings.show_undetected_encoders))
                                .changed() {
                                    self.render_settings.selected_encoder_idx = 0;
                                    self.render_settings.encoder =
                                        (*selectable_encoders.first().unwrap()).clone();
                                    changed |= true;
                            }
                        });
                        ui.end_row();

                        ui.label("Encoding bitrate").on_hover_text(tooltip_text("Target bitrate of the rendered video."));
                        changed |= ui.add(Slider::new(&mut self.render_settings.bitrate_mbps, 0..=160).text("Mbps")).changed();
                        ui.end_row();

                        ui.label("Upscale").on_hover_text(tooltip_text("Upscale the output video to get better quality after uploading to YouTube."));
                        let upscale_targets = [
                            backend::ffmpeg::UpscaleTarget::None,
                            backend::ffmpeg::UpscaleTarget::P1440,
                            backend::ffmpeg::UpscaleTarget::P2160,
                        ];
                        let mut selected_upscale_idx = match self.render_settings.upscale {
                            backend::ffmpeg::UpscaleTarget::None => 0,
                            backend::ffmpeg::UpscaleTarget::P1440 => 1,
                            backend::ffmpeg::UpscaleTarget::P2160 => 2,
                        };
                        let upscale_selection = egui::ComboBox::from_id_source("upscale")
                            .width(100.0)
                            .show_index(
                                ui,
                                &mut selected_upscale_idx,
                                upscale_targets.len(),
                                |i| upscale_targets[i].to_string(),
                            );
                        if upscale_selection.changed() {
                            self.render_settings.upscale = upscale_targets[selected_upscale_idx];
                            changed |= true;
                        }
                        ui.end_row();

                        let is_4_3 = self.video_info.as_ref().map(|v| (v.width as f32 / v.height as f32) < 1.5).unwrap_or(false);
                        if is_4_3 {
                            ui.label("Pad 4:3 to 16:9").on_hover_text(tooltip_text("Add black bars on the sides to transform 4:3 video into 16:9."));
                            let pad_changed = ui.add(Checkbox::without_text(&mut self.render_settings.pad_4_3_to_16_9)).changed();
                            if pad_changed {
                                pad_toggled = true;
                                changed |= true;
                            }
                            ui.end_row();
                        }

                        ui.label("Chroma key").on_hover_text(tooltip_text("Render the video with a chroma key instead of the input video so the OSD can be overlay in video editing software."));
                        ui.horizontal(|ui| {
                            changed |= ui.add(Checkbox::without_text(&mut self.render_settings.use_chroma_key)).changed();
                            changed |= ui.color_edit_button_rgb(&mut self.render_settings.chroma_key).changed();
                        });
                        ui.end_row();
                    });
            });

        if pad_toggled {
            self.auto_center_horizontal();
        }
        if changed {
            self.update_osd_preview(ctx);
            self.config_changed = Some(Instant::now());
        }
    }
    pub fn auto_center_horizontal(&mut self) {
        if let (Some(video_info), Some(osd_file), Some(_)) = (&self.video_info, &self.osd_file, &self.font_file) {
            let is_4_3 = (video_info.width as f32 / video_info.height as f32) < 1.5;
            let effective_width = if self.render_settings.pad_4_3_to_16_9 && is_4_3 {
                video_info.height * 16 / 9
            } else {
                video_info.width
            };
            let base_char_size = get_character_size(effective_width, video_info.height);
            let scale_factor = self.osd_options.scale / 100.0;
            let scaled_char_width = (base_char_size.width() as f32 * scale_factor).round() as i32;

            let frame = osd_file
                .frames
                .get(self.osd_preview.preview_frame as usize - 1)
                .unwrap();
            let min_x = frame.glyphs.iter().map(|g| g.grid_position.x).min().unwrap() as i32;
            let max_x = frame.glyphs.iter().map(|g| g.grid_position.x).max().unwrap() as i32;
            let pixel_range = (max_x - min_x + 1) * scaled_char_width;
            self.osd_options.position.x = (video_info.width as i32 - pixel_range) / 2 - min_x * scaled_char_width;
        }
    }
}
