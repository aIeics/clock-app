// Release builds on Windows run without a console window.
// Debug builds keep it, so panics and logs stay visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod autostart;
mod calendar;
mod clock;
mod icon;
mod instance;
mod popout;
mod reminders;
mod settings;
mod storage;
mod theme;
mod timer;
mod tray;

use eframe::egui;

fn main() -> eframe::Result {
    // A second launch only brings the running Clock to the front.
    let running = match instance::claim() {
        instance::Launch::First(running) => running,
        instance::Launch::AlreadyRunning => return Ok(()),
    };

    // Started at login ("Start with Windows"): stay hidden in the tray.
    let start_in_tray = std::env::args().any(|arg| arg == autostart::TRAY_FLAG);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clock")
            .with_inner_size([440.0, 620.0])
            .with_min_inner_size([380.0, 480.0])
            .with_icon(icon::app_icon())
            .with_visible(!start_in_tray),
        ..Default::default()
    };

    eframe::run_native(
        "clock-app",
        options,
        Box::new(move |cc| Ok(Box::new(app::ClockApp::new(cc, running, start_in_tray)))),
    )
}
