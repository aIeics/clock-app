use eframe::egui;

/// Reminders that fire at a set date and time. Placeholder until Phase 3.
#[derive(Default)]
pub struct RemindersView;

impl RemindersView {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        crate::app::not_built_yet(ui, "Reminders", "Phase 3");
    }
}
