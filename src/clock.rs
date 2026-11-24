use std::f32::consts::TAU;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};

use crate::settings::Settings;
use crate::theme;

/// What changed while drawing the Clock tab.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClockChanges {
    pub settings_changed: bool,
    pub pop_out: bool,
}

/// Live local time and date, plus the clock format options.
#[derive(Default)]
pub struct ClockView;

impl ClockView {
    /// Draws the dial, the digital time and the format controls.
    ///
    /// Returns what changed, so the caller can save settings and open the pop-out.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> ClockChanges {
        let now = Local::now();
        let mut changes = ClockChanges::default();

        // Everything scales with the window: the dial takes about half the
        // height and the text follows the dial.
        let avail = ui.available_size();
        let diameter = (avail.x * 0.78).min(avail.y * 0.56).clamp(140.0, 460.0);
        let digits = (diameter * 0.15).clamp(24.0, 64.0);

        ui.add_space(((avail.y - diameter - digits * 2.6 - 60.0) * 0.3).max(6.0));
        let (response, painter) =
            ui.allocate_painter(Vec2::new(avail.x, diameter), egui::Sense::hover());
        let second = if settings.show_seconds {
            // Seconds since midnight, so the hand never spins backwards at :00.
            let target = now.num_seconds_from_midnight() as f32;
            Some(theme::animate_with(
                ui.ctx(),
                egui::Id::unique("clock-second-hand"),
                target,
                0.22,
                egui::emath::easing::back_out,
            ))
        } else {
            None
        };
        paint_dial(
            &painter,
            response.rect.center(),
            diameter / 2.0,
            &now,
            second,
        );

        ui.add_space(digits * 0.25);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(avail.x, digits * 1.2), egui::Sense::hover());
        theme::tabular_text(
            ui.painter(),
            rect.center(),
            &format_time(&now, settings),
            theme::bold(digits),
            theme::TEXT,
        );
        ui.label(
            egui::RichText::new(format_date(&now))
                .size((digits * 0.42).clamp(12.0, 20.0))
                .color(theme::TEXT_DIM),
        );

        // Format controls sit at the bottom of the tab.
        ui.add_space((ui.available_height() - 44.0).max(12.0));
        let spacing = ui.spacing().item_spacing.x;
        let seconds_width = theme::check_width(ui, "Seconds");
        let row = 120.0 + 18.0 + seconds_width + 18.0 + 84.0;
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.add_space(((ui.available_width() - row - spacing) / 2.0).max(0.0));
                let mut format = settings.clock_24h;
                if theme::segmented(
                    ui,
                    egui::Id::unique("clock-format"),
                    &mut format,
                    &[(false, "12h"), (true, "24h")],
                    Vec2::new(120.0, 28.0),
                    0.0,
                ) {
                    settings.clock_24h = format;
                    changes.settings_changed = true;
                }
                ui.add_space(18.0 - spacing);
                changes.settings_changed |=
                    theme::check(ui, &mut settings.show_seconds, "Seconds").changed();
                ui.add_space(18.0 - spacing);
                changes.pop_out = theme::button_sized(ui, "Pop out", theme::Kind::Ghost, 84.0)
                    .on_hover_text("Open a floating clock on the desktop")
                    .clicked();
            },
        );

        // Sleep until the next visible change instead of repainting continuously.
        // Only this tab schedules repaints, so the app is idle on every other tab.
        ui.ctx()
            .request_repaint_after(until_next_tick(&now, settings.show_seconds));
        changes
    }
}

/// A plain dial with twelve marks. `second` is the (possibly animated)
/// seconds-since-midnight for the second hand, or `None` to hide it.
fn paint_dial(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    now: &DateTime<Local>,
    second: Option<f32>,
) {
    // Sizes follow a 96-unit design radius.
    let k = radius / 96.0;
    // `turn` is a fraction of a full circle, starting at 12 o'clock.
    let at = |turn: f32, length: f32| center + Vec2::angled(turn * TAU - TAU / 4.0) * length * k;

    painter.circle(
        center,
        radius,
        Color32::from_rgb(10, 10, 10),
        Stroke::new(1.0, Color32::from_rgb(38, 38, 38)),
    );

    for mark in 0..12 {
        let turn = mark as f32 / 12.0;
        let (from, to, stroke) = if mark % 3 == 0 {
            (88.0, 74.0, Stroke::new(2.5 * k, theme::TEXT))
        } else {
            (
                86.0,
                78.0,
                Stroke::new(1.5 * k, Color32::from_rgb(122, 122, 122)),
            )
        };
        painter.line_segment([at(turn, from), at(turn, to)], stroke);
    }

    let hand = |turn: f32, tail: f32, length: f32, width: f32, color: Color32| {
        let (start, end) = (at(turn + 0.5, tail), at(turn, length));
        painter.line_segment([start, end], Stroke::new(width * k, color));
        painter.circle_filled(start, width * k / 2.0, color);
        painter.circle_filled(end, width * k / 2.0, color);
    };

    let seconds = second.map_or(0.0, |s| (s % 60.0) / 60.0);
    let minute = now.minute() as f32 + if second.is_some() { seconds } else { 0.0 };
    let hour = (now.hour() % 12) as f32 + minute / 60.0;
    hand(hour / 12.0, 10.0, 46.0, 5.0, theme::TEXT);
    hand(minute / 60.0, 12.0, 70.0, 3.0, theme::TEXT);
    if let Some(second) = second {
        let turn = second / 60.0;
        painter.line_segment(
            [at(turn + 0.5, 20.0), at(turn, 78.0)],
            Stroke::new(1.2 * k, Color32::from_rgb(154, 154, 154)),
        );
    }
    painter.circle_filled(center, 4.5 * k, theme::TEXT);
    painter.circle_filled(center, 1.6 * k, theme::BG);
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
