use std::time::Duration;

use chrono::{DateTime, Local, Timelike};
use eframe::egui;

use crate::settings::Settings;

/// Live local time and date, plus the clock format options.
#[derive(Default)]
pub struct ClockView;

impl ClockView {
    /// Draws the clock and its format checkboxes.
    ///
    /// Returns `true` when a setting changed, so the caller can save it.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> bool {
        let now = Local::now();
        let mut changed = false;

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.22);
            ui.label(
                egui::RichText::new(format_time(&now, settings))
                    .size(56.0)
                    .monospace(),
            );
            ui.label(egui::RichText::new(format_date(&now)).size(18.0));

            ui.add_space(16.0);
            ui.horizontal(|ui| {
                changed |= ui.checkbox(&mut settings.clock_24h, "24-hour").changed();
                changed |= ui.checkbox(&mut settings.show_seconds, "Seconds").changed();
            });
        });

        // Sleep until the next visible change instead of repainting continuously.
        // Only this tab schedules repaints, so the app is idle on every other tab.
        ui.ctx()
            .request_repaint_after(until_next_tick(&now, settings.show_seconds));
        changed
    }
}

/// `09:05:07` in 24-hour mode, `9:05:07 AM` in 12-hour mode.
pub fn format_time(now: &DateTime<Local>, settings: &Settings) -> String {
    let format = match (settings.clock_24h, settings.show_seconds) {
        (true, true) => "%H:%M:%S",
        (true, false) => "%H:%M",
        (false, true) => "%-I:%M:%S %p",
        (false, false) => "%-I:%M %p",
    };
    now.format(format).to_string()
}

/// `Thursday, 1 January 2026`.
pub fn format_date(now: &DateTime<Local>) -> String {
    now.format("%A, %-d %B %Y").to_string()
}

/// How long until the clock's text changes: the next second, or the next minute
/// when seconds are hidden.
pub fn until_next_tick(now: &DateTime<Local>, show_seconds: bool) -> Duration {
    let nanos = u64::from(now.nanosecond());
    let until_next_second = Duration::from_nanos(1_000_000_000 - nanos % 1_000_000_000);

    if show_seconds {
        until_next_second
    } else {
        let seconds = u64::from(now.second());
        Duration::from_secs(59 - seconds) + until_next_second
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{LocalResult, NaiveDate, TimeZone};

    fn at(hour: u32, minute: u32, second: u32) -> DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2026, 1, 1)
            .unwrap()
            .and_hms_opt(hour, minute, second)
            .unwrap();
        match Local.from_local_datetime(&naive) {
            LocalResult::Single(time) => time,
            other => panic!("test time is not unambiguous here: {other:?}"),
        }
    }

    fn settings(clock_24h: bool, show_seconds: bool) -> Settings {
        Settings {
            clock_24h,
            show_seconds,
        }
    }

    #[test]
    fn formats_24_hour_time() {
        assert_eq!(format_time(&at(9, 5, 7), &settings(true, true)), "09:05:07");
        assert_eq!(format_time(&at(9, 5, 7), &settings(true, false)), "09:05");
        assert_eq!(
            format_time(&at(23, 59, 0), &settings(true, true)),
            "23:59:00"
        );
    }

    #[test]
    fn formats_12_hour_time() {
        assert_eq!(
            format_time(&at(9, 5, 7), &settings(false, true)),
            "9:05:07 AM"
        );
        assert_eq!(
            format_time(&at(9, 5, 7), &settings(false, false)),
            "9:05 AM"
        );
        assert_eq!(
            format_time(&at(0, 0, 0), &settings(false, false)),
            "12:00 AM"
        );
        assert_eq!(
            format_time(&at(12, 0, 0), &settings(false, false)),
            "12:00 PM"
        );
        assert_eq!(
            format_time(&at(13, 30, 0), &settings(false, false)),
            "1:30 PM"
        );
    }

    #[test]
    fn formats_the_date() {
        assert_eq!(format_date(&at(12, 0, 0)), "Thursday, 1 January 2026");
    }

    #[test]
    fn waits_for_the_next_second() {
        let now = at(12, 0, 0);
        assert_eq!(until_next_tick(&now, true), Duration::from_secs(1));

        let now = at(12, 0, 59);
        assert_eq!(until_next_tick(&now, true), Duration::from_secs(1));
    }

    #[test]
    fn waits_for_the_next_minute_when_seconds_are_hidden() {
        let now = at(12, 0, 0);
        assert_eq!(until_next_tick(&now, false), Duration::from_secs(60));

        let now = at(12, 0, 30);
        assert_eq!(until_next_tick(&now, false), Duration::from_secs(30));

        let now = at(12, 59, 59);
        assert_eq!(until_next_tick(&now, false), Duration::from_secs(1));
    }
}
