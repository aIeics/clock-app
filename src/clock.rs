use std::f32::consts::TAU;
use std::time::Duration;

use chrono::{DateTime, Local, Timelike};
use eframe::egui::{self, Color32, FontId, Pos2, Stroke, Vec2};

use crate::settings::Settings;
use crate::theme;

/// Live local time and date, plus the clock format options.
#[derive(Default)]
pub struct ClockView;

impl ClockView {
    /// Draws the clock and its format switches.
    ///
    /// Returns `true` when a setting changed, so the caller can save it.
    pub fn ui(&mut self, ui: &mut egui::Ui, settings: &mut Settings) -> bool {
        let now = Local::now();
        let mut changed = false;

        let avail = ui.available_size();
        // Everything scales with the window: the dial takes about half the
        // height, and the text follows the dial.
        let diameter = (avail.x * 0.8).min(avail.y * 0.5).clamp(120.0, 440.0);
        let font_size = (diameter * 0.21).clamp(34.0, 92.0);

        ui.add_space(((avail.y - diameter - font_size * 3.2 - 80.0) * 0.35).max(4.0));
        let (rect, painter) =
            ui.allocate_painter(Vec2::new(avail.x, diameter + 24.0), egui::Sense::hover());
        paint_dial(&painter, rect.rect.center(), diameter / 2.0, &now, settings);

        // Digital time, with AM/PM set smaller beside it.
        let text = format_time(&now, settings);
        let (main, suffix) = match text.split_once(' ') {
            Some((main, suffix)) => (main.to_string(), Some(suffix.to_string())),
            None => (text, None),
        };
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(avail.x, font_size * 1.15), egui::Sense::hover());
        let painter = ui.painter();
        let big = FontId::proportional(font_size);
        let small = FontId::proportional(font_size * 0.34);
        let suffix_width = suffix.as_ref().map_or(0.0, |suffix| {
            painter
                .layout_no_wrap(suffix.clone(), small.clone(), theme::TEXT)
                .size()
                .x
                + 8.0
        });
        let center = rect.center() - Vec2::new(suffix_width / 2.0, 0.0);
        let width = theme::tabular_text(painter, center, &main, big, theme::TEXT);
        if let Some(suffix) = suffix {
            painter.text(
                Pos2::new(
                    center.x + width / 2.0 + 8.0,
                    rect.center().y + font_size * 0.2,
                ),
                egui::Align2::LEFT_BOTTOM,
                suffix,
                small,
                theme::TEXT_DIM,
            );
        }

        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new(format_date(&now))
                    .size((font_size * 0.3).clamp(14.0, 24.0))
                    .color(theme::TEXT_DIM),
            );
            ui.add_space(18.0);
        });

        theme::card()
            .inner_margin(egui::Margin::symmetric(18, 12))
            .show(ui, |ui| {
                ui.columns(2, |columns| {
                    columns[0].vertical_centered(|ui| {
                        changed |= theme::toggle(ui, &mut settings.clock_24h, "24-hour").changed();
                    });
                    columns[1].vertical_centered(|ui| {
                        changed |=
                            theme::toggle(ui, &mut settings.show_seconds, "Seconds").changed();
                    });
                });
            });

        // Sleep until the next visible change instead of repainting continuously.
        // Only this tab schedules repaints, so the app is idle on every other tab.
        ui.ctx()
            .request_repaint_after(until_next_tick(&now, settings.show_seconds));
        changed
    }
}

/// An analog clock face. The hands jump with each tick (no sweeping), so the
/// view still only repaints when the time shown changes.
fn paint_dial(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    now: &DateTime<Local>,
    settings: &Settings,
) {
    // `turn` is a fraction of a full circle, starting at 12 o'clock.
    let at = |turn: f32, length: f32| center + Vec2::angled(turn * TAU - TAU / 4.0) * length;

    painter.circle(
        center,
        radius,
        theme::SURFACE,
        Stroke::new(1.0, theme::BORDER),
    );

    // The seconds ring fills up over each minute.
    let second = now.second() as f32;
    if settings.show_seconds {
        const STEPS: usize = 120;
        let filled = ((second + 1.0) / 60.0 * STEPS as f32) as usize;
        let points: Vec<Pos2> = (0..=filled.min(STEPS))
            .map(|i| at(i as f32 / STEPS as f32, radius - 4.0))
            .collect();
        painter.line(points, Stroke::new(2.0, theme::TEXT));
    }

    for tick in 0..60 {
        let turn = tick as f32 / 60.0;
        let (inner, stroke) = if tick % 5 == 0 {
            (radius * 0.72, Stroke::new(2.5, theme::TEXT))
        } else {
            (
                radius * 0.78,
                Stroke::new(1.0, theme::TEXT_DIM.gamma_multiply(0.5)),
            )
        };
        painter.line_segment([at(turn, inner), at(turn, radius * 0.82)], stroke);
    }

    let minute = now.minute() as f32
        + if settings.show_seconds {
            second / 60.0
        } else {
            0.0
        };
    let hour = (now.hour() % 12) as f32 + minute / 60.0;

    let hand = |turn: f32, length: f32, width: f32, color: Color32| {
        painter.line_segment(
            [at(turn + 0.5, radius * 0.08), at(turn, length)],
            Stroke::new(width, color),
        );
        painter.circle_filled(at(turn, length), width / 2.0, color);
    };
    hand(hour / 12.0, radius * 0.45, 6.0, theme::TEXT);
    hand(minute / 60.0, radius * 0.66, 4.0, theme::TEXT);
    if settings.show_seconds {
        let turn = second / 60.0;
        painter.line_segment(
            [at(turn + 0.5, radius * 0.18), at(turn, radius * 0.76)],
            Stroke::new(1.5, theme::TEXT_DIM),
        );
    }
    painter.circle_filled(center, 5.0, theme::TEXT);
    painter.circle_filled(center, 2.0, theme::BG);
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
