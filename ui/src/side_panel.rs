use backend::font::FontType;
use egui::{text::LayoutJob, CollapsingHeader, Color32, RichText, TextFormat, TextStyle, Ui};
use egui_extras::{Column, TableBuilder};

use super::WalksnailOsdTool;
use crate::util::{format_minutes_seconds, separator_with_space};

impl WalksnailOsdTool {
    pub fn render_sidepanel(&mut self, ctx: &egui::Context) {
        let panel_width =
            self.ui_dimensions.file_info_column1_width + self.ui_dimensions.file_info_column2_width + 40.0;
        egui::SidePanel::left("side_panel")
            .default_width(270.0)
            .min_width(panel_width)
            .max_width(1000.0)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_space(10.0);
                    self.video_info(ui);
                    separator_with_space(ui, 15.0);
                    self.osd_info(ui);
                    separator_with_space(ui, 15.0);
                    self.srt_info(ui);
                    separator_with_space(ui, 15.0);
                    self.font_info(ui, ctx);
                });
            });
    }

    fn video_info(&self, ui: &mut Ui) {
        let video_info = self.video_info.as_ref();
        let file_loaded = video_info.is_some();

        CollapsingHeader::new(RichText::new("Video file").heading())
            .icon(move |ui, opennes, response| circle_icon(ui, opennes, response, file_loaded))
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id("video_info", |ui| {
                    TableBuilder::new(ui)
                        .column(Column::exact(self.ui_dimensions.file_info_column1_width))
                        .column(
                            Column::remainder()
                                .at_least(self.ui_dimensions.file_info_column2_width)
                                .clip(true),
                        )
                        .auto_shrink([false, true])
                        .body(|mut body| {
                            let row_height = self.ui_dimensions.file_info_row_height;
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("File name:");
                                });
                                row.col(|ui| {
                                    if let Some(video_file) = &self.video_file {
                                        ui.label(
                                            video_file
                                                .file_name()
                                                .unwrap_or_default()
                                                .to_string_lossy(),
                                        );
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Resolution:");
                                });
                                row.col(|ui| {
                                    if let (Some(width), Some(height)) =
                                        (video_info.map(|i| i.width), video_info.map(|i| i.height))
                                    {
                                        ui.label(format!("{width}x{height}"));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Frame rate:");
                                });
                                row.col(|ui| {
                                    if let Some(frame_rate) = video_info.map(|i| i.frame_rate) {
                                        ui.label(format!("{frame_rate:.2} fps"));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Bitrate:");
                                });
                                row.col(|ui| {
                                    if let Some(bitrate) = video_info.map(|i| i.bitrate) {
                                        #[allow(clippy::cast_precision_loss)]
                                        let bitrate_mbps = bitrate as f32 / 1_000_000.0;
                                        ui.label(format!("{bitrate_mbps:.2} Mbps"));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Duration:");
                                });
                                row.col(|ui| {
                                    if let Some(duration) = video_info.map(|i| i.duration) {
                                        ui.label(format_minutes_seconds(&duration));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });
                        });
                });
            });
    }

    fn osd_info(&self, ui: &mut Ui) {
        let osd_file = self.osd_file.as_ref();
        let file_loaded = osd_file.is_some();

        CollapsingHeader::new(RichText::new("OSD file").heading())
            .icon(move |ui, opennes, response| circle_icon(ui, opennes, response, file_loaded))
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id("osd_info", |ui| {
                    TableBuilder::new(ui)
                        .column(Column::exact(self.ui_dimensions.file_info_column1_width))
                        .column(
                            Column::remainder()
                                .at_least(self.ui_dimensions.file_info_column2_width)
                                .clip(true),
                        )
                        .body(|mut body| {
                            let row_height = self.ui_dimensions.file_info_row_height;
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("File name:");
                                });
                                row.col(|ui| {
                                    if let Some(osd_file) = osd_file {
                                        ui.label(
                                            osd_file
                                                .file_path
                                                .file_name()
                                                .map_or_else(|| "-".to_string(), |f| f.to_string_lossy().into_owned()),
                                        );
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("FC firmware:");
                                });
                                row.col(|ui| {
                                    if let Some(osd_file) = osd_file {
                                        let fw = osd_file.fc_firmware.to_string();
                                        if osd_file.fc_firmware == backend::osd::FcFirmware::Unknown {
                                            ui.label(RichText::new(format!("{fw} !")).color(Color32::RED))
                                                .on_hover_text("FC firmware could not be detected. Auto-selection of fonts may not work correctly.");
                                        } else {
                                            ui.label(fw);
                                        }
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Frames:");
                                });
                                row.col(|ui| {
                                    if let Some(osd_file) = osd_file {
                                        if osd_file.is_empty() {
                                            ui.label(RichText::new(format!("{} (Empty/Corrupted !)", osd_file.frame_count)).color(Color32::RED))
                                                .on_hover_text("This OSD file contains no visible data (all frames are empty). It might be corrupted or the VTX failed to record OSD data.");
                                        } else {
                                            ui.label(osd_file.frame_count.to_string());
                                        }
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Duration:");
                                });
                                row.col(|ui| {
                                    if let Some(duration) = osd_file.map(|i| i.duration) {
                                        ui.label(format_minutes_seconds(&duration));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });
                        });
                });
            });
    }

    pub fn srt_info(&self, ui: &mut Ui) {
        let srt_file = self.srt_file.as_ref();
        let file_loaded = srt_file.is_some();

        CollapsingHeader::new(RichText::new("SRT file").heading())
            .icon(move |ui, opennes, response| circle_icon(ui, opennes, response, file_loaded))
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id("srt_info", |ui| {
                    TableBuilder::new(ui)
                        .column(Column::exact(self.ui_dimensions.file_info_column1_width))
                        .column(
                            Column::remainder()
                                .at_least(self.ui_dimensions.file_info_column2_width)
                                .clip(true),
                        )
                        .body(|mut body| {
                            let row_height = self.ui_dimensions.file_info_row_height;
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("File name:");
                                });
                                row.col(|ui| {
                                    if let Some(srt_file) = srt_file {
                                        ui.label(
                                            srt_file
                                                .file_path
                                                .file_name()
                                                .map_or_else(|| "-".to_string(), |f| f.to_string_lossy().into_owned()),
                                        );
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Duration:");
                                });
                                row.col(|ui| {
                                    if let Some(duration) = srt_file.map(|i| i.duration) {
                                        let video_duration = self.video_info.as_ref().map(|v| v.duration);

                                        let mut job = LayoutJob::default();
                                        let style = ui.style();
                                        let font_id = style.text_styles.get(&TextStyle::Body).unwrap().clone();

                                        let mut duration_color = style.visuals.text_color();
                                        let mut warning = None;

                                        if let Some(v_dur) = video_duration {
                                            if (v_dur.as_secs_f32() - duration.as_secs_f32()).abs() > 1.0 {
                                                duration_color = Color32::RED;
                                                warning = Some("Mismatch with video duration!");
                                            }
                                        }

                                        job.append(
                                            &format_minutes_seconds(&duration),
                                            0.0,
                                            TextFormat::simple(font_id.clone(), duration_color),
                                        );

                                        if let Some(msg) = warning {
                                            job.append(" !", 5.0, TextFormat::simple(font_id, Color32::RED));
                                            ui.label(job).on_hover_text(RichText::new(msg).color(Color32::RED));
                                        } else {
                                            ui.label(job);
                                        }
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });
                        });
                });
            });
    }

    #[allow(clippy::too_many_lines)]
    fn font_info(&mut self, ui: &mut Ui, ctx: &egui::Context) {
        let file_loaded = self.font_file.is_some();

        CollapsingHeader::new(RichText::new("Font file").heading())
            .icon(move |ui, opennes, response| circle_icon(ui, opennes, response, file_loaded))
            .default_open(true)
            .show(ui, |ui| {
                ui.push_id("font_info", |ui| {
                    TableBuilder::new(ui)
                        .column(Column::exact(self.ui_dimensions.file_info_column1_width))
                        .column(
                            Column::remainder()
                                .at_least(self.ui_dimensions.file_info_column2_width)
                                .clip(true),
                        )
                        .body(|mut body| {
                            let row_height = self.ui_dimensions.file_info_row_height;
                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Font folder:");
                                });
                                row.col(|ui| {
                                    ui.label(self.userfont_path.to_string_lossy());
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("File name:");
                                });
                                row.col(|ui| {
                                    let font_data = self.font_file.as_ref().map(|f| {
                                        (
                                            f.file_path
                                                .file_name()
                                                .map_or_else(|| "-".to_string(), |n| n.to_string_lossy().to_string()),
                                            f.character_size.clone(),
                                            f.file_path.clone(),
                                        )
                                    });

                                    if let Some((file_name, size, current_path)) = font_data {
                                        let folder = self.userfont_path.clone();
                                        let firmware = self.osd_file.as_ref().map(|f| f.fc_firmware.clone());

                                        ui.menu_button(file_name, |ui| {
                                            let compatible_fonts = backend::font::font_picker::find_compatible_fonts(
                                                &folder,
                                                &size,
                                                firmware.as_ref(),
                                            );
                                            for path in compatible_fonts {
                                                let name =
                                                    path.file_name().map(|f| f.to_string_lossy()).unwrap_or_default();
                                                let selected = path == current_path;
                                                if ui.selectable_label(selected, name).clicked() {
                                                    if let Ok(new_font) = backend::font::FontFile::open(path) {
                                                        self.font_file = Some(new_font);
                                                        self.font_manually_selected = true;
                                                        self.auto_center_horizontal();
                                                        self.update_osd_preview(ctx);
                                                        self.auto_resize_window(ctx);
                                                        ui.close_menu();
                                                    }
                                                }
                                            }
                                        });
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Font size:");
                                });
                                row.col(|ui| {
                                    if let Some(font_file) = &self.font_file {
                                        ui.label(font_file.character_size.to_string());
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });

                            body.row(row_height, |mut row| {
                                row.col(|ui| {
                                    ui.label("Characters:");
                                });
                                row.col(|ui| {
                                    if let Some(font_file) = &self.font_file {
                                        ui.label(format!(
                                            "{}{}",
                                            font_file.character_count,
                                            if font_file.font_type == FontType::FourColor {
                                                " (4 colors)"
                                            } else {
                                                ""
                                            }
                                        ));
                                    } else {
                                        ui.label("-");
                                    }
                                });
                            });
                        });
                });
            });
    }
}

fn circle_icon(ui: &egui::Ui, _openness: f32, response: &egui::Response, loaded: bool) {
    let stroke = ui.style().interact(response).fg_stroke;
    let radius = 3.0;
    if loaded {
        ui.painter().circle_filled(response.rect.center(), radius, stroke.color);
    } else {
        ui.painter().circle_stroke(response.rect.center(), radius - 0.5, stroke);
    }
}
