//! The Reminders tab: a small form above a scrollable list.

use chrono::{Datelike, Local, NaiveDate};
use eframe::egui::{self, Color32, Pos2, Sense, Stroke, Vec2};

use super::schedule;
use super::{Fired, Reminder, Reminders, Repeat};
use crate::theme::{self, Icon, Kind};

/// Form and transient UI state. The reminder data itself lives in `Reminders`.
#[derive(Debug, Default)]
pub struct RemindersView {
    title: String,
    note: String,
    when_text: String,
    repeat: Repeat,
    editing: Option<u64>,
    error: Option<String>,
    banner: Option<Fired>,
    sheet_open: bool,
    focus_title: bool,
}

impl RemindersView {
    /// Called from `App::logic` when one or more reminders become due.
    pub fn show_fired(&mut self, fired: Fired) {
        self.banner = Some(fired);
    }

    fn clear_form(&mut self) {
        self.title.clear();
        self.note.clear();
        self.when_text.clear();
        self.repeat = Repeat::Once;
        self.editing = None;
        self.error = None;
    }

    fn fill_form(&mut self, reminder: &Reminder) {
        self.title = reminder.title.clone();
        self.note = reminder.note.clone();
        self.when_text = reminder.when.format("%Y-%m-%d %H:%M").to_string();
        self.repeat = reminder.repeat;
        self.editing = Some(reminder.id);
        self.error = None;
    }

    /// Returns `true` when the list changed and should be saved.
    fn submit(&mut self, reminders: &mut Reminders) -> bool {
        let Some(when) = schedule::parse_when(&self.when_text, &Local::now()) else {
            self.error = Some("Use YYYY-MM-DD HH:MM or HH:MM.".to_string());
            return false;
        };

        let title = self.title.trim().to_string();
        if title.is_empty() {
            self.error = Some("Give the reminder a title.".to_string());
            return false;
        }

        let changed = match self.editing {
            Some(id) => reminders.update(id, title, self.note.clone(), when, self.repeat),
            None => {
                reminders.add(title, self.note.clone(), when, self.repeat);
                true
            }
        };

        self.clear_form();
        changed
    }

    /// Draws the tab. Returns `true` if the caller should save the reminders.
    pub fn ui(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        let column = ui.max_rect();

        self.banner(ui);
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new("Upcoming")
                        .size(18.0)
                        .color(theme::TEXT),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if theme::icon_button(ui, Icon::Plus, 32.0, true)
                        .on_hover_text("New reminder")
                        .clicked()
                    {
                        self.open_sheet(None);
                    }
                });
            },
        );
        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    changed |= self.timeline(ui, reminders);
                });
            });

        changed |= self.sheet(ui, column, reminders);
        changed
    }

    /// The "due" or "missed" alert. Slides in once when it appears.
    fn banner(&mut self, ui: &mut egui::Ui) {
        let shown = ui.ctx().animate_bool_with_time_and_easing(
            egui::Id::unique("reminders-banner"),
            self.banner.is_some(),
            0.3,
            egui::emath::easing::cubic_out,
        );
        let Some(fired) = &self.banner else {
            return;
        };

        let mut dismiss = false;
        ui.scope(|ui| {
            ui.multiply_opacity(shown);
            ui.add_space((1.0 - shown) * -12.0);
            theme::card()
                .stroke(Stroke::new(1.0, theme::DANGER))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
                        theme::paint_icon(ui.painter(), rect, Icon::Bell, theme::DANGER);
                        let prefix = if fired.missed { "Missed" } else { "Due" };
                        ui.label(
                            egui::RichText::new(format!("{prefix}: {}", fired.title))
                                .color(theme::TEXT)
                                .strong(),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            dismiss = theme::icon_button(ui, Icon::Close, 26.0, false)
                                .on_hover_text("Dismiss")
                                .clicked();
                        });
                    });
                });
            ui.add_space(10.0);
        });
        if dismiss {
            self.banner = None;
        }
    }

    /// Opens the slide-up form, empty or filled with `reminder` for editing.
    fn open_sheet(&mut self, reminder: Option<&Reminder>) {
        match reminder {
            Some(reminder) => self.fill_form(reminder),
            None => self.clear_form(),
        }
        self.sheet_open = true;
        self.focus_title = true;
    }

    /// Reminders on a vertical line, grouped by day.
    fn timeline(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        let mut rows: Vec<Reminder> = reminders.items().to_vec();
        rows.sort_by_key(|reminder| reminder.when);

        if rows.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
                theme::paint_icon(ui.painter(), rect, Icon::Bell, theme::TEXT_DIM);
                ui.add_space(6.0);
                ui.label(egui::RichText::new("No reminders yet.").color(theme::TEXT));
                ui.label(egui::RichText::new("Press + to add one.").color(theme::TEXT_DIM));
            });
            return false;
        }

        let today = Local::now().date_naive();
        let mut index = 0;
        while index < rows.len() {
            let day = rows[index].when.date_naive();
            let group: Vec<Reminder> = rows[index..]
                .iter()
                .take_while(|reminder| reminder.when.date_naive() == day)
                .cloned()
                .collect();
            index += group.len();

            theme::section_label(ui, &day_label(day, today));
            let left = ui.cursor().left();
            let top = ui.cursor().top();
            let mut dots = Vec::new();
            for reminder in group {
                let editing = self.editing == Some(reminder.id);
                let row = ui.horizontal(|ui| {
                    ui.add_space(22.0);
                    let (time_rect, _) =
                        ui.allocate_exact_size(Vec2::new(44.0, 18.0), Sense::hover());
                    ui.painter().text(
                        time_rect.left_center(),
                        egui::Align2::LEFT_CENTER,
                        reminder.when.format("%H:%M").to_string(),
                        egui::FontId::proportional(12.5),
                        theme::TEXT_DIM,
                    );
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        let title = egui::RichText::new(&reminder.title).size(14.5);
                        ui.label(if editing {
                            title.color(theme::TEXT).underline()
                        } else {
                            title.color(theme::TEXT)
                        });
                        if reminder.repeat != Repeat::Once {
                            ui.label(
                                egui::RichText::new(reminder.repeat.label())
                                    .size(11.5)
                                    .color(theme::TEXT_DIM),
                            );
                        }
                        if !reminder.note.is_empty() {
                            ui.label(
                                egui::RichText::new(&reminder.note)
                                    .size(11.5)
                                    .italics()
                                    .color(theme::TEXT_DIM),
                            );
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if theme::icon_plain(ui, Icon::Trash, 24.0)
                            .on_hover_text("Delete")
                            .clicked()
                        {
                            reminders.remove(reminder.id);
                            changed = true;
                            if editing {
                                self.clear_form();
                                self.sheet_open = false;
                            }
                        }
                        if theme::icon_plain(ui, Icon::Edit, 24.0)
                            .on_hover_text("Edit")
                            .clicked()
                        {
                            self.open_sheet(Some(&reminder));
                        }
                    });
                });
                dots.push(Pos2::new(left + 4.5, row.response.rect.top() + 9.0));
                ui.add_space(6.0);
            }

            // The line and its dots, drawn behind the rows' text.
            let bottom = ui.cursor().top() - 6.0;
            let painter = ui.painter();
            painter.line_segment(
                [Pos2::new(left + 4.5, top), Pos2::new(left + 4.5, bottom)],
                Stroke::new(1.0, Color32::from_rgb(42, 42, 42)),
            );
            for dot in dots {
                painter.circle(dot, 4.0, theme::BG, Stroke::new(1.0, theme::TEXT));
            }
            ui.add_space(10.0);
        }

        changed
    }

    /// The form that slides up from the bottom of the window.
    fn sheet(&mut self, ui: &mut egui::Ui, column: egui::Rect, reminders: &mut Reminders) -> bool {
        let ctx = ui.ctx().clone();
        let shown = ctx.animate_bool_with_time_and_easing(
            egui::Id::unique("reminders-sheet"),
            self.sheet_open,
            0.3,
            egui::emath::easing::cubic_out,
        );
        if shown == 0.0 {
            return false;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.sheet_open = false;
        }

        // Dim everything behind the sheet; a click there closes it.
        let screen = ctx.content_rect();
        let dismissed = egui::Area::new(egui::Id::unique("reminders-sheet-backdrop"))
            .order(egui::Order::Middle)
            .movable(false)
            .fixed_pos(screen.min)
            .show(&ctx, |ui| {
                let (rect, response) = ui.allocate_exact_size(screen.size(), Sense::click());
                ui.painter().rect_filled(
                    rect,
                    0.0,
                    Color32::from_black_alpha((160.0 * shown) as u8),
                );
                response.clicked()
            })
            .inner;
        if dismissed {
            self.sheet_open = false;
        }

        let height_id = egui::Id::unique("reminders-sheet-height");
        let height: f32 = ctx.data(|data| data.get_temp(height_id).unwrap_or(280.0));
        let pos = Pos2::new(
            column.left(),
            screen.bottom() + (1.0 - shown) * (height + 8.0),
        );
        let mut changed = false;
        let sheet = egui::Area::new(egui::Id::unique("reminders-sheet-area"))
            .order(egui::Order::Foreground)
            .movable(false)
            .constrain(false)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(pos)
            .show(&ctx, |ui| {
                ui.set_width(column.width());
                egui::Frame::new()
                    .fill(Color32::from_rgb(12, 12, 12))
                    .stroke(Stroke::new(1.0, theme::BORDER))
                    .inner_margin(egui::Margin::same(16))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        changed = self.form(ui, reminders);
                    });
            });
        ctx.data_mut(|data| data.insert_temp(height_id, sheet.response.rect.height()));
        changed
    }

    /// The fields inside the sheet.
    fn form(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        let heading = if self.editing.is_some() {
            "Edit reminder"
        } else {
            "New reminder"
        };
        theme::section_label(ui, heading);

        let width = ui.available_width();
        let title = theme::text_field(ui, &mut self.title, "What to remember", width);
        if self.focus_title {
            title.request_focus();
            self.focus_title = false;
        }
        ui.columns(2, |columns| {
            let half = columns[0].available_width();
            theme::text_field(
                &mut columns[0],
                &mut self.when_text,
                "YYYY-MM-DD HH:MM",
                half,
            );
            theme::text_field(&mut columns[1], &mut self.note, "Note (optional)", half);
        });
        theme::segmented(
            ui,
            egui::Id::unique("reminders-repeat"),
            &mut self.repeat,
            &REPEATS,
            Vec2::new(width, 30.0),
            0.0,
        );
        if let Some(error) = &self.error {
            theme::error_text(ui, error);
        }

        ui.add_space(2.0);
        let mut save = false;
        let mut cancel = false;
        ui.allocate_ui_with_layout(
            Vec2::new(width, 34.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                let label = if self.editing.is_some() {
                    "Save"
                } else {
                    "Add"
                };
                save = theme::button(ui, label, Kind::Primary).clicked();
                cancel = theme::button(ui, "Cancel", Kind::Ghost).clicked();
            },
        );
        save |= title.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));

        if cancel {
            self.clear_form();
            self.sheet_open = false;
        }
        if save {
            changed = self.submit(reminders);
            if self.error.is_none() {
                self.sheet_open = false;
            }
        }
        changed
    }
}

/// "Today", "Tomorrow", a weekday name within the next week, or a date.
fn day_label(day: NaiveDate, today: NaiveDate) -> String {
    match (day - today).num_days() {
        0 => "Today".to_string(),
        1 => "Tomorrow".to_string(),
        -1 => "Yesterday".to_string(),
        2..=6 => day.format("%A").to_string(),
        _ if day.year() == today.year() => day.format("%a %-d %B").to_string(),
        _ => day.format("%a %-d %B %Y").to_string(),
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
    use chrono::{LocalResult, NaiveDate, TimeZone};

    fn at(day: u32, hour: u32, minute: u32) -> chrono::DateTime<Local> {
        let naive = NaiveDate::from_ymd_opt(2026, 11, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap();
        match Local.from_local_datetime(&naive) {
            LocalResult::Single(time) => time,
            other => panic!("test time is not unambiguous here: {other:?}"),
        }
    }

    #[test]
    fn day_labels_read_naturally() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 25).unwrap();
        let day = |m, d| NaiveDate::from_ymd_opt(2026, m, d).unwrap();
        assert_eq!(day_label(today, today), "Today");
        assert_eq!(day_label(day(11, 26), today), "Tomorrow");
        assert_eq!(day_label(day(11, 24), today), "Yesterday");
        assert_eq!(day_label(day(11, 28), today), "Saturday");
        assert_eq!(day_label(day(12, 9), today), "Wed 9 December");
        assert_eq!(
            day_label(NaiveDate::from_ymd_opt(2027, 1, 4).unwrap(), today),
            "Mon 4 January 2027"
        );
    }

    #[test]
    fn submitting_adds_a_reminder_and_clears_the_form() {
        let mut view = RemindersView::default();
        let mut reminders = Reminders::default();

        view.title = "Call mum".to_string();
        view.when_text = "2026-12-01 09:00".to_string();
        view.repeat = Repeat::Weekly;

        assert!(view.submit(&mut reminders));
        assert_eq!(reminders.items().len(), 1);
        assert_eq!(reminders.items()[0].title, "Call mum");
        assert_eq!(reminders.items()[0].repeat, Repeat::Weekly);
        assert!(view.title.is_empty());
        assert_eq!(view.editing, None);
    }

    #[test]
    fn submitting_reports_bad_input_without_changing_the_list() {
        let mut view = RemindersView::default();
        let mut reminders = Reminders::default();

        view.when_text = "tomorrow".to_string();
        assert!(!view.submit(&mut reminders));
        assert!(reminders.items().is_empty());
        assert!(view.error.is_some());

        view.when_text = "2026-12-01 09:00".to_string();
        view.title = "  ".to_string();
        assert!(!view.submit(&mut reminders));
        assert!(reminders.items().is_empty());
    }

    #[test]
    fn editing_updates_the_chosen_reminder() {
        let mut view = RemindersView::default();
        let mut reminders = Reminders::default();
        let id = reminders.add("Old".into(), String::new(), at(25, 9, 0), Repeat::Once);

        view.fill_form(&reminders.items()[0]);
        assert_eq!(view.editing, Some(id));
        view.title = "New".to_string();
        view.when_text = "2026-11-26 10:00".to_string();
        assert!(view.submit(&mut reminders));

        assert_eq!(reminders.items().len(), 1);
        assert_eq!(reminders.items()[0].title, "New");
        assert_eq!(
            reminders.items()[0].when.format("%H:%M").to_string(),
            "10:00"
        );
    }
}
