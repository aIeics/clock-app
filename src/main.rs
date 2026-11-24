// Release builds on Windows run without a console window.
// Debug builds keep it, so panics and logs stay visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod calendar;
mod clock;
mod icon;
mod popout;
mod reminders;
mod settings;
mod storage;
mod theme;
mod timer;
mod tray;

use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clock")
            .with_inner_size([440.0, 620.0])
            .with_min_inner_size([380.0, 480.0])
            .with_icon(icon::app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "clock-app",
        options,
        Box::new(|cc| Ok(Box::new(app::ClockApp::new(cc)))),
    )
}
