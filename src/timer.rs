use eframe::egui;

/// Countdown timer. Placeholder until Phase 2.
#[derive(Default)]
pub struct TimerView;

impl TimerView {
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        crate::app::not_built_yet(ui, "Timer", "Phase 2");
    }
}
