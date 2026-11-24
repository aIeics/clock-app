//! Month grid with today highlighted, days with reminders marked, and a quick
//! way to add a reminder to the selected day.

use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime};
use eframe::egui;

use crate::reminders::{Reminders, Repeat};

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
        let today = Local::now().date_naive();
        let mut changed = false;

        ui.horizontal(|ui| {
            if ui.add(egui::Button::new("<").small()).clicked() {
                (self.year, self.month) = shifted_month(self.year, self.month, -1);
            }
            ui.heading(format!("{} {}", MONTHS[self.month as usize - 1], self.year));
            if ui.add(egui::Button::new(">").small()).clicked() {
                (self.year, self.month) = shifted_month(self.year, self.month, 1);
            }
            if ui.button("Today").clicked() {
                self.go_to_month(today);
                self.selected = today;
            }
        });

        egui::Grid::new("month").num_columns(7).show(ui, |ui| {
            for name in WEEKDAYS {
                ui.strong(name);
            }
            ui.end_row();

            for week in month_grid(self.year, self.month) {
                for day in week {
                    let Some(date) = day else {
                        ui.label("");
                        continue;
                    };

                    let marked = reminders.on(date).next().is_some();
                    let label = if marked {
                        format!("{} •", date.day())
                    } else {
                        date.day().to_string()
                    };

                    let mut button = egui::Button::new(label)
                        .min_size(egui::Vec2::new(36.0, 22.0))
                        .selected(self.selected == date);
                    if date == today {
                        button = button.fill(egui::Color32::from_rgb(59, 130, 246));
                    }
                    if ui.add(button).clicked() {
                        self.selected = date;
                    }
                }
                ui.end_row();
            }
        });

        ui.separator();
        ui.heading(self.selected.format("%-d %B %Y").to_string());
        let rows: Vec<_> = reminders.on(self.selected).cloned().collect();
        if rows.is_empty() {
            ui.label("Nothing on this day.");
        } else {
            for reminder in rows {
                ui.horizontal(|ui| {
                    ui.label(reminder.when.format("%H:%M").to_string());
                    ui.label(reminder.title);
                });
            }
        }

        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Time");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_time)
                    .hint_text("HH:MM")
                    .desired_width(58.0),
            );
            ui.label("Title");
            ui.add(
                egui::TextEdit::singleline(&mut self.new_title)
                    .hint_text("New reminder")
                    .desired_width(130.0),
            );
        });
        ui.horizontal(|ui| {
            egui::ComboBox::from_label("Repeat")
                .selected_text(self.new_repeat.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.new_repeat, Repeat::Once, Repeat::Once.label());
                    ui.selectable_value(&mut self.new_repeat, Repeat::Daily, Repeat::Daily.label());
                    ui.selectable_value(
                        &mut self.new_repeat,
                        Repeat::Weekly,
                        Repeat::Weekly.label(),
                    );
                });
            if ui.button("Add").clicked() {
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
        });
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(239, 68, 68), error.clone());
        }

        changed
    }
}

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
