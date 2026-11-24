//! Month grid with today highlighted, days with reminders marked, and a quick
//! way to add a reminder to the selected day.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::reminders::{Reminders, Repeat};
use crate::theme::{self, Icon, Kind};

const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Weeks of a month in a Monday-first grid. Empty slots are `None`.
pub fn month_grid(year: i32, month: u32) -> Vec<[Option<NaiveDate>; 7]> {
    let first = NaiveDate::from_ymd_opt(year, month, 1).expect("a valid month");
    let mut slot = first.weekday().num_days_from_monday() as usize;
    let mut weeks = Vec::new();
    let mut week = [None; 7];

    for day in 1..=days_in_month(year, month) {
        if slot == 7 {
            weeks.push(week);
            week = [None; 7];
            slot = 0;
        }
        week[slot] = NaiveDate::from_ymd_opt(year, month, day);
        slot += 1;
    }
    if slot > 0 {
        weeks.push(week);
    }

    weeks
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .expect("a valid month")
        .pred_opt()
        .expect("a day before the first")
        .day()
}

/// `year`/`month` after moving `delta` months, with year rollover.
fn shifted_month(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let total = year * 12 + (month as i32 - 1) + delta;
    (total.div_euclid(12), total.rem_euclid(12) as u32 + 1)
}

#[derive(Debug)]
pub struct CalendarView {
    year: i32,
    month: u32,
    selected: NaiveDate,
    new_title: String,
    new_time: String,
    new_repeat: Repeat,
    error: Option<String>,
}

impl Default for CalendarView {
    fn default() -> Self {
        let today = Local::now().date_naive();
        Self {
            year: today.year(),
            month: today.month(),
            selected: today,
            new_title: String::new(),
            new_time: String::new(),
            new_repeat: Repeat::Once,
            error: None,
        }
    }
}

impl CalendarView {
    fn go_to_month(&mut self, date: NaiveDate) {
        (self.year, self.month) = (date.year(), date.month());
    }

    /// Selected day plus the time typed in the quick-add form.
    fn new_reminder_when(&self) -> Option<DateTime<Local>> {
        let time = NaiveTime::parse_from_str(self.new_time.trim(), "%H:%M").ok()?;
        self.selected
            .and_time(time)
            .and_local_timezone(Local)
            .single()
    }

    /// Draws the tab. Returns `true` if the reminders should be saved.
    pub fn ui(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(2.0);
                self.header(ui);
                ui.add_space(6.0);
                self.grid(ui, reminders);
                ui.add_space(10.0);
                changed |= self.day_panel(ui, reminders);
                ui.add_space(8.0);
            });

        changed
    }

    /// Month name with previous/next arrows and a "Today" button.
    fn header(&mut self, ui: &mut egui::Ui) {
        let today = Local::now().date_naive();
        let spacing = ui.spacing().item_spacing.x;
        let title_width = 170.0;
        let width = 34.0 * 2.0 + title_width + 72.0 + spacing * 3.0;

        ui.horizontal(|ui| {
            ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
            if theme::icon_button(ui, Icon::ChevronLeft, 34.0, false).clicked() {
                (self.year, self.month) = shifted_month(self.year, self.month, -1);
            }
            let (rect, _) = ui.allocate_exact_size(Vec2::new(title_width, 34.0), Sense::hover());
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{} {}", MONTHS[self.month as usize - 1], self.year),
                FontId::proportional(19.0),
                theme::TEXT,
            );
            if theme::icon_button(ui, Icon::ChevronRight, 34.0, false).clicked() {
                (self.year, self.month) = shifted_month(self.year, self.month, 1);
            }
            if theme::button(ui, "Today", Kind::Ghost).clicked() {
                self.go_to_month(today);
                self.selected = today;
            }
        });
    }

    /// The month grid. Slides sideways when the month changes.
    fn grid(&mut self, ui: &mut egui::Ui, reminders: &Reminders) {
        let today = Local::now().date_naive();
        let weeks = month_grid(self.year, self.month);
        let width = ui.available_width();
        let gap = 6.0;
        let cell_w = (width - gap * 6.0) / 7.0;
        let cell_h = (cell_w * 0.8).clamp(32.0, 64.0);
        let header_h = 22.0;
        let height = header_h + weeks.len() as f32 * (cell_h + gap);

        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
        let index = (self.year * 12 + self.month as i32) as f32;
        let slide = theme::animate(ui.ctx(), egui::Id::unique("calendar-month"), index, 0.3);
        let offset = (slide - index).clamp(-1.0, 1.0);
        let shift = Vec2::new(-offset * 36.0, 0.0);
        let opacity = 1.0 - offset.abs() * 0.8;
        let painter = ui.painter().clone().with_clip_rect(rect.expand(2.0));
        let cell_at = |row: usize, col: usize| {
            Rect::from_min_size(
                rect.min
                    + Vec2::new(
                        col as f32 * (cell_w + gap),
                        header_h + row as f32 * (cell_h + gap),
                    ),
                Vec2::new(cell_w, cell_h),
            )
        };

        for (col, name) in WEEKDAYS.iter().enumerate() {
            let x = rect.left() + col as f32 * (cell_w + gap) + cell_w / 2.0;
            painter.text(
                Pos2::new(x, rect.top() + header_h / 2.0 - 3.0),
                egui::Align2::CENTER_CENTER,
                name.to_uppercase(),
                FontId::proportional(11.0),
                theme::TEXT_DIM,
            );
        }

        let font = FontId::proportional((cell_h * 0.36).clamp(13.0, 18.0));
        for (row, week) in weeks.iter().enumerate() {
            for (col, day) in week.iter().enumerate() {
                let Some(date) = *day else { continue };
                let cell = cell_at(row, col);
                let response = ui
                    .interact(
                        cell,
                        egui::Id::unique(("calendar-day", date)),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if response.clicked() {
                    self.selected = date;
                }

                let hover = ui
                    .ctx()
                    .animate_bool_responsive(response.id.with("hover"), response.hovered());
                let selected = ui.ctx().animate_bool_with_time_and_easing(
                    response.id.with("selected"),
                    self.selected == date,
                    0.2,
                    egui::emath::easing::cubic_out,
                );
                let is_today = date == today;
                let cell = cell.translate(shift);
                let fade = |color: Color32| color.gamma_multiply(opacity);

                let fill = if is_today {
                    theme::TEXT
                } else {
                    theme::SURFACE.lerp_to_gamma(theme::SURFACE_HI, hover)
                };
                painter.rect_filled(cell, 8.0, fade(fill));
                if selected > 0.0 && !is_today {
                    painter.rect_stroke(
                        cell.shrink(1.0 - selected),
                        8.0,
                        Stroke::new(1.5, fade(theme::TEXT.gamma_multiply(selected))),
                        StrokeKind::Inside,
                    );
                }

                let weekend = col >= 5;
                let color = if is_today {
                    theme::BG
                } else if weekend {
                    theme::TEXT_DIM
                } else {
                    theme::TEXT
                };
                painter.text(
                    cell.center() - Vec2::new(0.0, 3.0),
                    egui::Align2::CENTER_CENTER,
                    date.day().to_string(),
                    font.clone(),
                    fade(color),
                );
                if reminders.on(date).next().is_some() {
                    let dot = if is_today { theme::BG } else { theme::TEXT };
                    painter.circle_filled(
                        Pos2::new(cell.center().x, cell.bottom() - cell_h * 0.16),
                        2.5,
                        fade(dot),
                    );
                }
            }
        }
    }

    /// The selected day's reminders and the quick-add form.
    fn day_panel(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(self.selected.format("%A, %-d %B %Y").to_string())
                        .size(16.0)
                        .color(theme::TEXT),
                );

                let rows: Vec<_> = reminders.on(self.selected).cloned().collect();
                if rows.is_empty() {
                    ui.label(egui::RichText::new("Nothing on this day.").color(theme::TEXT_DIM));
                }
                for reminder in rows {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  \u{00b7}  {}",
                            reminder.when.format("%H:%M"),
                            reminder.title
                        ))
                        .color(theme::TEXT),
                    );
                }

                ui.add_space(6.0);
                theme::section_label(ui, "Add to this day");
                let width = ui.available_width();
                ui.columns(2, |columns| {
                    let half = columns[0].available_width();
                    theme::text_field(&mut columns[0], &mut self.new_time, "HH:MM", half);
                    theme::text_field(&mut columns[1], &mut self.new_title, "New reminder", half);
                });
                theme::segmented(
                    ui,
                    egui::Id::unique("calendar-repeat"),
                    &mut self.new_repeat,
                    &REPEATS,
                    Vec2::new(width, 32.0),
                    8.0,
                );
                ui.add_space(4.0);
                if theme::button(ui, "Add", Kind::Primary).clicked() {
                    match self.new_reminder_when() {
                        Some(when) if !self.new_title.trim().is_empty() => {
                            reminders.add(
                                self.new_title.trim().to_string(),
                                String::new(),
                                when,
                                self.new_repeat,
                            );
                            self.new_title.clear();
                            self.new_time.clear();
                            self.new_repeat = Repeat::Once;
                            self.error = None;
                            changed = true;
                        }
                        _ => self.error = Some("Enter a time (HH:MM) and a title.".to_string()),
                    }
                }
                if let Some(error) = &self.error {
                    theme::error_text(ui, error);
                }
            });
        });
        changed
    }
}

const REPEATS: [(Repeat, &str); 3] = [
    (Repeat::Once, "Once"),
    (Repeat::Daily, "Daily"),
    (Repeat::Weekly, "Weekly"),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    #[test]
    fn a_month_starts_on_its_real_weekday() {
        let weeks = month_grid(2026, 1); // 1 January 2026 was a Thursday
        assert_eq!(weeks[0][0], None);
        assert_eq!(weeks[0][1], None);
        assert_eq!(weeks[0][2], None);
        assert_eq!(weeks[0][3], Some(date(2026, 1, 1)));
    }

    #[test]
    fn leap_february_has_29_days() {
        let days: Vec<_> = month_grid(2024, 2)
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        assert_eq!(days.len(), 29);
        assert_eq!(days[28], date(2024, 2, 29));
    }

    #[test]
    fn a_normal_february_has_28_days() {
        let days: Vec<_> = month_grid(2025, 2)
            .into_iter()
            .flatten()
            .flatten()
            .collect();
        assert_eq!(days.len(), 28);
        assert!(!days.iter().any(|day| day.day() == 29));
    }

    #[test]
    fn every_day_appears_once_in_order() {
        for month in 1..=12 {
            let days: Vec<_> = month_grid(2026, month)
                .into_iter()
                .flatten()
                .flatten()
                .collect();
            let expected: Vec<_> = (1..=days_in_month(2026, month))
                .map(|day| date(2026, month, day))
                .collect();
            assert_eq!(days, expected, "month {month}");
        }
    }

    #[test]
    fn a_month_has_four_to_six_weeks() {
        for year in 2024..=2027 {
            for month in 1..=12 {
                let weeks = month_grid(year, month);
                assert!((4..=6).contains(&weeks.len()), "{year}-{month}");
                assert!(weeks.iter().all(|week| week.len() == 7));
            }
        }
    }

    #[test]
    fn month_navigation_rolls_over_the_year() {
        assert_eq!(shifted_month(2026, 1, -1), (2025, 12));
        assert_eq!(shifted_month(2026, 12, 1), (2027, 1));
        assert_eq!(shifted_month(2026, 6, 25), (2028, 7));
    }
}
