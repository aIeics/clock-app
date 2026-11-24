use eframe::egui;

use crate::{calendar::CalendarView, clock::ClockView, reminders::RemindersView, timer::TimerView};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Clock,
    Timer,
    Reminders,
    Calendar,
}

impl Tab {
    const ALL: [Self; 4] = [Self::Clock, Self::Timer, Self::Reminders, Self::Calendar];

    fn label(self) -> &'static str {
        match self {
            Self::Clock => "Clock",
            Self::Timer => "Timer",
            Self::Reminders => "Reminders",
            Self::Calendar => "Calendar",
        }
    }
}

/// Root of the app: a tab bar on top, the selected feature below.
/// Each feature lives in its own module and owns its own state.
#[derive(Default)]
pub struct ClockApp {
    tab: Tab,
    clock: ClockView,
    timer: TimerView,
    reminders: RemindersView,
    calendar: CalendarView,
}

impl ClockApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self::default()
    }
}

impl eframe::App for ClockApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, tab.label());
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Clock => self.clock.ui(ui),
            Tab::Timer => self.timer.ui(ui),
            Tab::Reminders => self.reminders.ui(ui),
            Tab::Calendar => self.calendar.ui(ui),
        });
    }
}

/// Shared placeholder for features that are planned but not built yet.
pub fn not_built_yet(ui: &mut egui::Ui, feature: &str, phase: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.heading(feature);
        ui.label(format!("Not built yet. Planned for {phase} in PLAN.md."));
    });
}
