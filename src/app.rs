use eframe::egui;

use crate::calendar::CalendarView;
use crate::clock::ClockView;
use crate::reminders::RemindersView;
use crate::settings::Settings;
use crate::storage::Store;
use crate::timer::TimerView;

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
pub struct ClockApp {
    tab: Tab,
    store: Store,
    settings: Settings,
    clock: ClockView,
    timer: TimerView,
    reminders: RemindersView,
    calendar: CalendarView,
}

impl ClockApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let store = Store::new();
        let settings = Settings::load(&store);

        Self {
            tab: Tab::default(),
            store,
            settings,
            clock: ClockView,
            timer: TimerView::default(),
            reminders: RemindersView::default(),
            calendar: CalendarView::default(),
        }
    }
}

impl eframe::App for ClockApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Called even while the window is hidden, so a timer can finish and
        // ask for the user's attention without the UI having to run.
        self.timer.logic(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for tab in Tab::ALL {
                    ui.selectable_value(&mut self.tab, tab, tab.label());
                }
            });
        });

        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Clock => {
                let before = self.settings.clone();
                self.clock.ui(ui, &mut self.settings);
                if self.settings != before {
                    self.settings.save(&self.store);
                }
            }
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
