use egui::{vec2, Align2, Button, Frame, Label, RichText, Sense, Ui, ViewportCommand, Visuals, Window};

use super::WalksnailOsdTool;

impl WalksnailOsdTool {
    pub fn render_top_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                self.import_files(ui, ctx);
                self.reset_files(ui);
                ui.add_space(30.0);
                ui.label(RichText::new(&self.app_version).weak());
                ui.add_space(ui.available_width() - 55.0);
                self.toggle_light_dark_theme(ui, ctx);
                self.about_window(ui, ctx);
            });
            ui.add_space(3.0);
        });
    }

    fn import_files(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if ui
            .add_enabled(self.render_status.is_not_in_progress(), Button::new("Open files"))
            .clicked()
        {
            if let Some(file_handles) = rfd::FileDialog::new()
                .add_filter("Avatar files", &["mp4", "osd", "png", "srt"])
                .pick_files()
            {
                tracing::info!("Opened files {:?}", file_handles);
                self.import_video_file(&file_handles);
                self.import_osd_file(&file_handles);
                self.auto_select_bundled_font();
                self.import_font_file(&file_handles);
                self.import_srt_file(&file_handles);

                self.auto_center_horizontal();
                self.update_osd_preview(ctx);
                self.auto_resize_window(ctx);
                self.render_status.reset();
            }
        }

        // Collect dropped files
        let file_handles = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .flat_map(|f| f.path.clone())
                .collect::<Vec<_>>()
        });
        if !file_handles.is_empty() {
            tracing::info!("Dropped files {:?}", file_handles);
            self.import_video_file(&file_handles);
            self.import_osd_file(&file_handles);
            self.auto_select_bundled_font();
            self.import_font_file(&file_handles);
            self.import_srt_file(&file_handles);
            self.auto_center_horizontal();
            self.update_osd_preview(ctx);
            self.auto_resize_window(ctx);
            self.render_status.reset();
        }
    }

    fn reset_files(&mut self, ui: &mut Ui) {
        if ui
            .add_enabled(self.render_status.is_not_in_progress(), Button::new("Reset files"))
            .clicked()
        {
            self.video_file = None;
            self.video_info = None;
            self.osd_file = None;
            self.font_file = None;
            self.srt_file = None;
            self.osd_preview.texture_handle = None;
            self.osd_preview.preview_frame = 1;
            self.render_status.reset();
            tracing::info!("Reset files");
        }
    }

    fn auto_select_bundled_font(&mut self) {
        if let (Some(video_info), Some(osd_file)) = (&self.video_info, &self.osd_file) {
            let character_size = backend::overlay::get_character_size(video_info.width, video_info.height);

            // Only auto-select if no font loaded, or the current font is a bundled one
            // (i.e., user hasn't manually picked a .png font)
            let should_auto_select = match &self.font_file {
                None => true,
                Some(f) => f.file_path.to_string_lossy().contains("(bundled)"),
            };

            if should_auto_select {
                if let Some(font) = backend::font::bundled_fonts::get_bundled_font(
                    &osd_file.fc_firmware,
                    &character_size,
                ) {
                    tracing::info!(
                        "Auto-selected bundled font: {:?} for firmware {:?}, resolution {:?}",
                        font.file_path, osd_file.fc_firmware, character_size
                    );
                    self.font_file = Some(font);
                }
            }
        }
    }

    fn auto_resize_window(&self, ctx: &egui::Context) {
        if let Some(video_info) = &self.video_info {
            // Side panel width + padding
            let side_panel_width = 285.0_f32;
            // Desired preview width in the central panel
            let preview_width = 700.0_f32;
            let total_width = side_panel_width + preview_width;

            // Calculate preview height based on video aspect ratio
            let aspect_ratio = video_info.width as f32 / video_info.height as f32;
            let preview_height = (preview_width - 20.0) / aspect_ratio;

            // Add space for OSD options, SRT options, rendering options, top/bottom panels, and margins
            let ui_chrome_height = 600.0_f32;
            let total_height = (preview_height + ui_chrome_height).min(1200.0);

            // Clamp to reasonable bounds
            let width = total_width.clamp(800.0, 1600.0);
            let height = total_height.clamp(600.0, 1200.0);

            ctx.send_viewport_cmd(ViewportCommand::InnerSize(vec2(width, height)));
        }
    }

    fn toggle_light_dark_theme(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let icon = if self.dark_mode { "☀" } else { "🌙" };
        if ui.add(Button::new(icon).frame(false)).clicked() {
            let mut visuals = if self.dark_mode {
                Visuals::light()
            } else {
                Visuals::dark()
            };
            visuals.indent_has_left_vline = false;
            ctx.set_visuals(visuals);
            self.dark_mode = !self.dark_mode;
        }
    }

    fn about_window(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        if ui.add(Button::new(RichText::new("ℹ")).frame(false)).clicked() {
            self.about_window_open = !self.about_window_open;
        }

        let frame = Frame::window(&ctx.style());
        if self.about_window_open {
            Window::new("About")
                .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
                .frame(frame)
                .open(&mut self.about_window_open)
                .auto_sized()
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.add_space(10.0);

                    egui::Grid::new("about").spacing(vec2(10.0, 5.0)).show(ui, |ui| {
                        ui.label("Author:");
                        ui.label("Alexander van Saase");
                        ui.end_row();

                        ui.label("Version:");
                        let version = &self.app_version;
                        if ui
                            .add(Label::new(version).sense(Sense::click()))
                            .on_hover_text_at_pointer("Double-click to copy to clipboard")
                            .double_clicked()
                        {
                            ui.output_mut(|o| o.copied_text.clone_from(version));
                        }
                        ui.end_row();

                        ui.label("Target:");
                        ui.label(&self.target);
                        ui.end_row();

                        ui.label("License:");
                        ui.hyperlink_to(
                            "General Public License v3.0",
                            "https://github.com/ksielick/walksnail-osd-tool/blob/master/LICENSE.md",
                        );
                        ui.end_row();
                    });

                    ui.add_space(10.0);

                    ui.hyperlink_to("Buy me a coffee", "https://www.buymeacoffee.com/avsaase");
                });
        }
    }
}
