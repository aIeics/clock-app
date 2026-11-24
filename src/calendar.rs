use eframe::egui;

/// Month view calendar. Placeholder until Phase 4.
#[derive(Default)]
pub struct CalendarView;

impl CalendarView {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        crate::app::not_built_yet(ui, "Calendar", "Phase 4");
    }
}
