//! The Reminders tab: a small form above a scrollable list.

use chrono::Local;
use eframe::egui::{self, Sense, Stroke, Vec2};

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

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.add_space(4.0);
                self.banner(ui);
                changed |= self.form(ui, reminders);
                ui.add_space(10.0);
                changed |= self.list(ui, reminders);
                ui.add_space(8.0);
            });

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

    /// The form for a new reminder, or the one being edited.
    fn form(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        theme::card().show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                let heading = if self.editing.is_some() {
                    "Edit reminder"
                } else {
                    "New reminder"
                };
                theme::section_label(ui, heading);

                let width = ui.available_width();
                theme::text_field(ui, &mut self.title, "What to remember", width);
                ui.columns(2, |columns| {
                    let width = columns[0].available_width();
                    theme::text_field(
                        &mut columns[0],
                        &mut self.when_text,
                        "YYYY-MM-DD HH:MM",
                        width,
                    );
                    theme::text_field(&mut columns[1], &mut self.note, "Note (optional)", width);
                });
                theme::segmented(
                    ui,
                    egui::Id::unique("reminders-repeat"),
                    &mut self.repeat,
                    &REPEATS,
                    Vec2::new(width, 32.0),
                    8.0,
                );

                ui.add_space(4.0);
                let editing = self.editing.is_some();
                let spacing = ui.spacing().item_spacing.x;
                let row = if editing { 64.0 * 2.0 + spacing } else { 64.0 };
                let mut submit = false;
                let mut cancel = false;
                ui.horizontal(|ui| {
                    ui.add_space(((ui.available_width() - row) / 2.0).max(0.0));
                    let label = if editing { "Save" } else { "Add" };
                    submit = theme::button(ui, label, Kind::Primary).clicked();
                    if editing {
                        cancel = theme::button(ui, "Cancel", Kind::Ghost).clicked();
                    }
                });
                if cancel {
                    self.clear_form();
                }
                if submit {
                    changed |= self.submit(reminders);
                }
                if let Some(error) = &self.error {
                    theme::error_text(ui, error);
                }
            });
        });
        changed
    }

    /// Every reminder as a row with edit and delete buttons.
    fn list(&mut self, ui: &mut egui::Ui, reminders: &mut Reminders) -> bool {
        let mut changed = false;
        // Clone the rows first: editing or deleting needs a mutable borrow of
        // `reminders` while we draw them.
        let rows: Vec<Reminder> = reminders.items().to_vec();
        theme::section_label(ui, &format!("Upcoming \u{00b7} {}", rows.len()));

        if rows.is_empty() {
            ui.add_space(16.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), Sense::hover());
            theme::paint_icon(ui.painter(), rect, Icon::Bell, theme::TEXT_DIM);
            ui.label(egui::RichText::new("No reminders yet.").color(theme::TEXT_DIM));
            return false;
        }

        for reminder in rows {
            let editing = self.editing == Some(reminder.id);
            let border = if editing { theme::TEXT } else { theme::BORDER };
            theme::card()
                .stroke(Stroke::new(1.0, border))
                .inner_margin(egui::Margin::symmetric(14, 10))
                .corner_radius(12)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            ui.label(
                                egui::RichText::new(&reminder.title)
                                    .color(theme::TEXT)
                                    .size(15.0)
                                    .strong(),
                            );
                            let mut when = schedule::format_when(&reminder.when);
                            if reminder.repeat != Repeat::Once {
                                when.push_str(" \u{00b7} ");
                                when.push_str(reminder.repeat.label());
                            }
                            ui.label(egui::RichText::new(when).color(theme::TEXT_DIM).size(12.5));
                            if !reminder.note.is_empty() {
                                ui.label(
                                    egui::RichText::new(&reminder.note)
                                        .color(theme::TEXT_DIM)
                                        .size(12.5)
                                        .italics(),
                                );
                            }
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if theme::icon_button(ui, Icon::Trash, 30.0, false)
                                .on_hover_text("Delete")
                                .clicked()
                            {
                                reminders.remove(reminder.id);
                                changed = true;
                                if editing {
                                    self.clear_form();
                                }
                            }
                            if theme::icon_button(ui, Icon::Edit, 30.0, false)
                                .on_hover_text("Edit")
                                .clicked()
                            {
                                self.fill_form(&reminder);
                            }
                        });
                    });
                });
        }
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
