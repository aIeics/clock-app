//! One week across the top; the selected day's reminders on an hour-by-hour
//! line below, plus a quick way to add a reminder to that day.

use chrono::{DateTime, Datelike, Days, Local, NaiveDate, NaiveTime, Timelike};
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

use crate::reminders::{Reminders, Repeat};
use crate::theme::{self, Icon, Kind};

/// The Monday of the week `date` is in.
pub fn week_start(date: NaiveDate) -> NaiveDate {
    date - Days::new(u64::from(date.weekday().num_days_from_monday()))
}

/// Seven consecutive days from `start`.
pub fn week(start: NaiveDate) -> [NaiveDate; 7] {
    std::array::from_fn(|i| start + Days::new(i as u64))
}

/// First and last hour rows of the day view: 07 to 21, stretched to fit any
/// reminder outside that range.
pub fn hour_span(hours: impl IntoIterator<Item = u32>) -> (u32, u32) {
    hours.into_iter().fold((7, 21), |(first, last), hour| {
        (first.min(hour), last.max(hour.min(23)))
    })
}

#[derive(Debug)]
pub struct CalendarView {
    week: NaiveDate,
    selected: NaiveDate,
    new_title: String,
    new_time: String,
    error: Option<String>,
    /// egui time when the selected day last changed, for the entrance animation.
    changed_at: f64,
    scroll_pending: bool,
}

impl Default for CalendarView {
    fn default() -> Self {
        let today = Local::now().date_naive();
        Self {
            week: week_start(today),
            selected: today,
            new_title: String::new(),
            new_time: String::new(),
            error: None,
            changed_at: f64::NEG_INFINITY,
            scroll_pending: true,
        }
    }
}

impl CalendarView {
    fn select(&mut self, ui: &egui::Ui, date: NaiveDate) {
        if date != self.selected {
            self.selected = date;
            self.changed_at = ui.input(|input| input.time);
            self.scroll_pending = true;
        }
        self.week = week_start(date);
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
        self.header(ui);
        ui.add_space(4.0);
        self.strip(ui, reminders);
        ui.add_space(8.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 16.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| theme::section_label(ui, &self.selected.format("%A %-d %B").to_string()),
        );
        let day_height = (ui.available_height() - 58.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(day_height)
            .auto_shrink([false, false])
            .show(ui, |ui| self.day(ui, reminders));
        ui.add_space(8.0);
        self.quick_add(ui, reminders)
    }

    /// Month name with previous/next week arrows and a "Today" button.
    fn header(&mut self, ui: &mut egui::Ui) {
        let today = Local::now().date_naive();
        // Name the month that most of the week falls in.
        let middle = self.week + Days::new(3);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 34.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(middle.format("%B %Y").to_string())
                        .size(17.0)
                        .color(theme::TEXT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::button(ui, "Today", Kind::Ghost).clicked() {
                        self.select(ui, today);
                    }
                    if theme::icon_button(ui, Icon::ChevronRight, 32.0, false)
                        .on_hover_text("Next week")
                        .clicked()
                    {
                        self.select(ui, self.selected + Days::new(7));
                    }
                    if theme::icon_button(ui, Icon::ChevronLeft, 32.0, false)
                        .on_hover_text("Previous week")
                        .clicked()
                    {
                        self.select(ui, self.selected - Days::new(7));
                    }
                });
            },
        );
    }

    /// The seven days of the week. Slides sideways when the week changes.
    fn strip(&mut self, ui: &mut egui::Ui, reminders: &Reminders) {
        let today = Local::now().date_naive();
        let width = ui.available_width();
        let gap = 4.0;
        let cell_w = (width - gap * 6.0) / 7.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 58.0), Sense::hover());

        let index = (self.week.num_days_from_ce() / 7) as f32;
        let slide = theme::animate(ui.ctx(), egui::Id::unique("calendar-week"), index, 0.3);
        let offset = (slide - index).clamp(-1.0, 1.0);
        let shift = Vec2::new(-offset * 30.0, 0.0);
        let fade = 1.0 - offset.abs() * 0.8;
        let painter = ui.painter().with_clip_rect(rect.expand(2.0));

        for (i, date) in week(self.week).into_iter().enumerate() {
            let cell = Rect::from_min_size(
                rect.min + Vec2::new(i as f32 * (cell_w + gap), 0.0),
                Vec2::new(cell_w, rect.height()),
            );
            let response = ui
                .interact(cell, egui::Id::unique(("calendar-day", i)), Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if response.clicked() {
                self.select(ui, date);
            }

            let selected = ui
                .ctx()
                .animate_bool_responsive(response.id.with("selected"), date == self.selected);
            let hover = ui
                .ctx()
                .animate_bool_responsive(response.id.with("hover"), response.hovered());
            let cell = cell.translate(shift);
            let border = if date == today {
                Color32::from_rgb(90, 90, 90)
            } else {
                theme::BG.lerp_to_gamma(theme::BORDER, hover)
            };
            painter.rect_filled(cell, 0.0, theme::BG.lerp_to_gamma(theme::TEXT, selected));
            painter.rect_stroke(
                cell,
                0.0,
                Stroke::new(1.0, border.gamma_multiply(fade)),
                StrokeKind::Inside,
            );

            let ink = theme::TEXT.lerp_to_gamma(theme::BG, selected);
            let dim = theme::TEXT_DIM.lerp_to_gamma(theme::BG, selected);
            let weekday = date.format("%a").to_string()[..2].to_uppercase();
            painter.text(
                Pos2::new(cell.center().x, cell.top() + 13.0),
                egui::Align2::CENTER_CENTER,
                weekday,
                FontId::proportional(9.5),
                dim.gamma_multiply(fade),
            );
            painter.text(
                Pos2::new(cell.center().x, cell.top() + 31.0),
                egui::Align2::CENTER_CENTER,
                date.day().to_string(),
                FontId::proportional(17.0),
                ink.gamma_multiply(fade),
            );
            if reminders.on(date).next().is_some() {
                painter.rect_filled(
                    Rect::from_center_size(
                        Pos2::new(cell.center().x, cell.bottom() - 9.0),
                        Vec2::splat(4.0),
                    ),
                    0.0,
                    ink.gamma_multiply(fade),
                );
            }
        }
    }

    /// The selected day as hour rows with its reminders placed on them.
    fn day(&mut self, ui: &mut egui::Ui, reminders: &Reminders) {
        let mut items: Vec<_> = reminders.on(self.selected).cloned().collect();
        items.sort_by_key(|reminder| reminder.when);
        let (first, last) = hour_span(items.iter().map(|reminder| reminder.when.hour()));
        let row = 28.0;
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(width, (last - first + 1) as f32 * row + 4.0),
            Sense::hover(),
        );
        let painter = ui.painter();

        for hour in first..=last {
            let y = rect.top() + (hour - first) as f32 * row;
            painter.line_segment(
                [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
                Stroke::new(1.0, Color32::from_rgb(24, 24, 24)),
            );
            painter.text(
                Pos2::new(rect.left() + 2.0, y + 3.0),
                egui::Align2::LEFT_TOP,
                format!("{hour:02}"),
                FontId::proportional(10.5),
                Color32::from_rgb(110, 110, 110),
            );
        }

        let block_h = 24.0;
        let mut previous_bottom = f32::NEG_INFINITY;
        let mut first_block = None;
        for (i, reminder) in items.iter().enumerate() {
            let hours = reminder.when.hour() as f32 + reminder.when.minute() as f32 / 60.0;
            // Push a block down when it would overlap the one before it.
            let top = (rect.top() + (hours - first as f32) * row + 2.0).max(previous_bottom + 2.0);
            previous_bottom = top + block_h;

            let shown = theme::since(ui.ctx(), self.changed_at + i as f64 * 0.06, 0.35);
            let block = Rect::from_min_max(
                Pos2::new(rect.left() + 36.0, top + (1.0 - shown) * 6.0),
                Pos2::new(rect.right() - 4.0, top + (1.0 - shown) * 6.0 + block_h),
            );
            first_block.get_or_insert(block);
            painter.rect_filled(
                block,
                0.0,
                Color32::from_rgb(17, 17, 17).gamma_multiply(shown),
            );
            painter.rect_filled(
                Rect::from_min_size(block.min, Vec2::new(2.0, block_h)),
                0.0,
                theme::TEXT.gamma_multiply(shown),
            );
            let title = painter.layout_no_wrap(
                reminder.title.clone(),
                FontId::proportional(12.5),
                theme::TEXT.gamma_multiply(shown),
            );
            let title_w = title.size().x;
            painter.with_clip_rect(block).galley(
                Pos2::new(block.left() + 10.0, block.center().y - title.size().y / 2.0),
                title,
                theme::TEXT,
            );
            let mut detail = reminder.when.format("%H:%M").to_string();
            if reminder.repeat != Repeat::Once {
                detail.push_str(" \u{00b7} ");
                detail.push_str(reminder.repeat.label());
            }
            painter.with_clip_rect(block).text(
                Pos2::new(block.left() + 18.0 + title_w, block.center().y),
                egui::Align2::LEFT_CENTER,
                detail,
                FontId::proportional(11.5),
                theme::TEXT_DIM.gamma_multiply(shown),
            );
        }

        if items.is_empty() {
            let block = Rect::from_min_size(
                Pos2::new(rect.left() + 36.0, rect.top() + 2.0 * row + 2.0),
                Vec2::new(rect.width() - 40.0, block_h),
            );
            painter.rect_stroke(
                block,
                0.0,
                Stroke::new(1.0, theme::BORDER),
                StrokeKind::Inside,
            );
            painter.text(
                Pos2::new(block.left() + 10.0, block.center().y),
                egui::Align2::LEFT_CENTER,
                "Nothing on this day",
                FontId::proportional(12.5),
                theme::TEXT_DIM,
            );
        }

        if self.scroll_pending {
            self.scroll_pending = false;
            if let Some(block) = first_block {
                ui.scroll_to_rect(block.expand(row), Some(egui::Align::TOP));
            }
        }
    }

    /// Time, title and a + button that add a reminder to the selected day.
    fn quick_add(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        let width = ui.available_width();
        let mut add = false;
        ui.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                theme::text_field(ui, &mut self.new_time, "HH:MM", 64.0);
                let spacing = ui.spacing().item_spacing.x;
                let title_w = ui.available_width() - 32.0 - spacing;
                let title = theme::text_field(ui, &mut self.new_title, "Add to this day", title_w);
                add |= title.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
                add |= theme::icon_button(ui, Icon::Plus, 32.0, true)
                    .on_hover_text("Add reminder")
                    .clicked();
            },
        );
        if add {
            match self.new_reminder_when() {
                Some(when) if !self.new_title.trim().is_empty() => {
                    reminders.add(
                        self.new_title.trim().to_string(),
                        String::new(),
                        when,
                        Repeat::Once,
                    );
                    self.new_title.clear();
                    self.new_time.clear();
                    self.error = None;
                    self.changed_at = ui.input(|input| input.time);
                    changed = true;
                }
                _ => self.error = Some("Enter a time (HH:MM) and a title.".to_string()),
            }
        }
        if let Some(error) = &self.error {
            theme::error_text(ui, error);
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
    fn a_week_starts_on_monday() {
        // 25 November 2026 is a Wednesday.
        assert_eq!(week_start(date(2026, 11, 25)), date(2026, 11, 23));
        assert_eq!(week_start(date(2026, 11, 23)), date(2026, 11, 23));
        assert_eq!(week_start(date(2026, 11, 29)), date(2026, 11, 23));
    }

    #[test]
    fn a_week_can_cross_months_and_years() {
        // 1 January 2027 is a Friday.
        assert_eq!(week_start(date(2027, 1, 1)), date(2026, 12, 28));
        let days = week(date(2026, 12, 28));
        assert_eq!(days[0], date(2026, 12, 28));
        assert_eq!(days[6], date(2027, 1, 3));
    }

    #[test]
    fn a_week_is_seven_consecutive_days() {
        let days = week(date(2024, 2, 26));
        for pair in days.windows(2) {
            assert_eq!(pair[1] - pair[0], chrono::TimeDelta::days(1));
        }
        assert_eq!(days[3], date(2024, 2, 29)); // leap day
    }

    #[test]
    fn the_day_view_covers_seven_to_nine_by_default() {
        assert_eq!(hour_span([]), (7, 21));
        assert_eq!(hour_span([9, 13, 18]), (7, 21));
    }

    #[test]
    fn the_day_view_stretches_to_fit_early_and_late_reminders() {
        assert_eq!(hour_span([5, 12]), (5, 21));
        assert_eq!(hour_span([23]), (7, 23));
        assert_eq!(hour_span([0, 23]), (0, 23));
    }
}
