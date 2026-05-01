use backend::ffmpeg::ToFfmpegMessage;
use egui::{vec2, Align, Button, Color32, Layout, ProgressBar, RichText, Ui};

use super::{util::format_minutes_seconds, WalksnailOsdTool};
use crate::render_status::Status;

impl WalksnailOsdTool {
    pub fn render_bottom_panel(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                self.start_stop_render_button(ui);
                self.render_progress(ui);
            });
            ui.add_space(2.0);
        });
    }

    fn start_stop_render_button(&mut self, ui: &mut Ui) {
        let button_size = vec2(110.0, 40.0);
        if self.render_status.is_not_in_progress() {
            if ui
                .add_enabled(
                    self.all_files_loaded(),
                    Button::new("Start render").min_size(button_size),
                )
                .on_disabled_hover_text("Load a video file and at least one subtitle source (OSD + Font, or SRT)")
                .clicked()
            {
                tracing::info!("Start render button clicked");
                self.start_render_process();
            }
        } else {
            if ui.add(Button::new("Stop render").min_size(button_size)).clicked() {
                tracing::info!("Stop render button clicked");
                if let Some(sender) = &self.to_ffmpeg_sender {
                    sender
                        .send(ToFfmpegMessage::AbortRender)
                        .map_err(|_| tracing::warn!("Failed to send abort render message"))
                        .unwrap();
                    self.render_status.stop_render();
                }
            }
        }
    }

    fn render_progress(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            // Top row: Progress bar (if applicable)
            match &self.render_status.status {
                Status::InProgress { progress_pct, .. } => {
                    ui.add(ProgressBar::new(*progress_pct).show_percentage());
                }
                Status::Completed => {
                    ui.add(ProgressBar::new(1.0).text("Done"));
                }
                Status::Cancelled { progress_pct } => {
                    ui.add(ProgressBar::new(*progress_pct).text("Cancelled"));
                }
                Status::Error { progress_pct, .. } => {
                    ui.add(ProgressBar::new(*progress_pct));
                }
                Status::Idle => {}
            }

            // Bottom row: Batch processing (left) and Render details (right)
            ui.horizontal(|ui| {
                if ui
                    .checkbox(&mut self.batch_processing, "Batch processing")
                    .on_hover_text(
                        "Automatically load and render the next MP4 file in the folder after this one finishes.",
                    )
                    .changed()
                {
                    self.config_changed = Some(std::time::Instant::now());
                }

                if self.batch_processing {
                    if let Some((current, total)) = self.batch_progress {
                        ui.label(format!(": {current}/{total}"));
                    }
                }

                if let Status::InProgress {
                    time_remaining,
                    fps,
                    speed,
                    ..
                } = &self.render_status.status
                {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.add_space(3.0);
                        let time_remaining_string = time_remaining
                            .as_ref()
                            .map_or_else(|| "––:––".into(), format_minutes_seconds);
                        ui.label(
                            RichText::new(format!(
                                "Time remaining: {time_remaining_string}, fps: {fps:.1}, speed: {speed:.3}x"
                            ))
                            .monospace(),
                        );
                    });
                } else if let Status::Error { error, .. } = &self.render_status.status {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        ui.label(RichText::new(error.clone()).color(Color32::RED));
                    });
                }
            });
        });
    }
}
