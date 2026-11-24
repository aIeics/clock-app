//! The Reminders tab: a small form above a scrollable list.

use chrono::Local;
use eframe::egui;

use super::schedule;
use super::{Fired, Reminder, Reminders, Repeat};

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

        let mut dismiss = false;
        if let Some(fired) = &self.banner {
            let prefix = if fired.missed { "Missed" } else { "Due" };
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    egui::Color32::from_rgb(239, 68, 68),
                    format!("{prefix}: {}", fired.title),
                );
                dismiss = ui.add(egui::Button::new("Dismiss").small()).clicked();
            });
            ui.separator();
        }
        if dismiss {
            self.banner = None;
        }

        // Form for a new reminder, or the one being edited.
        let heading = if self.editing.is_some() {
            "Edit reminder"
        } else {
            "New reminder"
        };
        ui.heading(heading);

        ui.horizontal(|ui| {
            ui.label("Title");
            ui.add(
                egui::TextEdit::singleline(&mut self.title)
                    .hint_text("What to remember")
                    .desired_width(150.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("When");
            ui.add(
                egui::TextEdit::singleline(&mut self.when_text)
                    .hint_text("YYYY-MM-DD HH:MM")
                    .desired_width(150.0),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Note");
            ui.add(
                egui::TextEdit::singleline(&mut self.note)
                    .hint_text("optional")
                    .desired_width(150.0),
            );
        });
        egui::ComboBox::from_label("Repeat")
            .selected_text(self.repeat.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.repeat, Repeat::Once, Repeat::Once.label());
                ui.selectable_value(&mut self.repeat, Repeat::Daily, Repeat::Daily.label());
                ui.selectable_value(&mut self.repeat, Repeat::Weekly, Repeat::Weekly.label());
            });

        let submit_label = if self.editing.is_some() {
            "Save"
        } else {
            "Add"
        };
        let submit = ui.button(submit_label).clicked();
        if self.editing.is_some() && ui.button("Cancel").clicked() {
            self.clear_form();
        }
        if submit {
            changed |= self.submit(reminders);
        }
        if let Some(error) = &self.error {
            ui.colored_label(egui::Color32::from_rgb(239, 68, 68), error.clone());
        }

        ui.separator();

        // Clone the rows first: editing or deleting needs a mutable borrow of
        // `reminders` while we draw them.
        let rows: Vec<Reminder> = reminders.items().to_vec();
        egui::ScrollArea::vertical().show(ui, |ui| {
            if rows.is_empty() {
                ui.label("No reminders yet.");
            }

            for reminder in rows {
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(reminder.title.clone()).strong());
                    ui.label(schedule::format_when(&reminder.when));
                    if reminder.repeat != Repeat::Once {
                        ui.small(reminder.repeat.label());
                    }
                });
                if !reminder.note.is_empty() {
                    ui.small(reminder.note.clone());
                }

                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new("Edit").small()).clicked() {
                        self.fill_form(&reminder);
                    }
                    if ui.add(egui::Button::new("Delete").small()).clicked() {
                        reminders.remove(reminder.id);
                        changed = true;
                        if self.editing == Some(reminder.id) {
                            self.clear_form();
                        }
                    }
                });
                ui.separator();
            }
        });

        changed
    }
}

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
