use std::time::Duration;

use chrono::{Local, Timelike};
use eframe::egui;

/// Live local time and date.
#[derive(Default)]
pub struct ClockView;

impl ClockView {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let now = Local::now();

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.25);
            ui.label(
                egui::RichText::new(now.format("%H:%M:%S").to_string())
                    .size(56.0)
                    .monospace(),
            );
            ui.label(egui::RichText::new(now.format("%A, %-d %B %Y").to_string()).size(18.0));
        });

        // Sleep until the next second boundary instead of repainting continuously.
        // Only this tab schedules repaints, so the app is idle on every other tab.
        let nanos_into_second = now.nanosecond() % 1_000_000_000;
        let until_next_second = Duration::from_nanos(u64::from(1_000_000_000 - nanos_into_second));
        ui.ctx().request_repaint_after(until_next_second);
    }
}
