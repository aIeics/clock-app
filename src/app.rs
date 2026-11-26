use chrono::Local;
use eframe::egui;

use crate::autostart;
use crate::calendar::CalendarView;
use crate::clock::ClockView;
use crate::instance::Running;
use crate::popout::Popout;
use crate::reminders::{Reminders, RemindersView};
use crate::settings::Settings;
use crate::storage::Store;
use crate::theme;
use crate::timer::TimerView;
use crate::tray::{self, Tray};

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
    reminders: Reminders,
    reminders_view: RemindersView,
    calendar: CalendarView,
    tray: Option<Tray>,
    popout: Popout,
    /// "Start with Windows", read from the registry at start-up.
    autostart: bool,
}

impl ClockApp {
    /// `start_in_tray` is true when the window was created hidden (a start at
    /// login).
    pub fn new(cc: &eframe::CreationContext<'_>, running: Running, start_in_tray: bool) -> Self {
        theme::apply(&cc.egui_ctx);
        let store = Store::new();
        let settings = Settings::load(&store);
        let reminders = Reminders::load(&store);
        let autostart = autostart::is_enabled();
        let tray = match Tray::new(cc.egui_ctx.clone(), autostart) {
            Ok(tray) => Some(tray),
            Err(err) => {
                eprintln!("clock-app: {err}");
                None
            }
        };

        // Without a tray icon a hidden window could never come back.
        if start_in_tray && tray.is_none() {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(true));
        }

        running.on_show_request({
            let ctx = cc.egui_ctx.clone();
            move || tray::show_window(&ctx)
        });

        Self {
            tab: Tab::default(),
            store,
            settings,
            clock: ClockView,
            timer: TimerView::default(),
            reminders,
            reminders_view: RemindersView::default(),
            calendar: CalendarView::default(),
            tray,
            popout: Popout::new(Store::new()),
            autostart,
        }
    }

    /// Turns "Start with Windows" on or off, then shows the real result in
    /// both the Clock tab and the tray menu.
    fn set_autostart(&mut self, on: bool) {
        if let Err(err) = autostart::set(on) {
            eprintln!("clock-app: {err}");
        }
        self.autostart = autostart::is_enabled();
        if let Some(tray) = &self.tray {
            tray.set_autostart_checked(self.autostart);
        }
    }
}

impl eframe::App for ClockApp {
    /// Transparent so the pop-out can show only its text on the desktop. The
    /// main window's panels still paint their own backgrounds.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // The window close button hides the app in the tray instead of ending
        // it. "Quit" from the tray menu sets a flag so the close can proceed.
        let quitting = self.tray.as_ref().is_some_and(Tray::quit_requested);
        if self.tray.is_some() && !quitting && ctx.input(|input| input.viewport().close_requested())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        // The tray menu's "Start with Windows" item flips the setting. The
        // registry is re-read, in case the installer changed it meanwhile.
        if self.tray.as_ref().is_some_and(Tray::take_autostart_click) {
            self.set_autostart(!autostart::is_enabled());
        }

        // Called even while the window is hidden, so timers and reminders can
        // become due without the UI having to run.
        self.timer.logic(ctx);

        let now = Local::now();
        let fired = self.reminders.fire_due(now);
        if !fired.is_empty() {
            self.reminders.save(&self.store);
            for item in fired {
                self.reminders_view.show_fired(item);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
        }

        // Sleep until the next reminder instead of polling. This also wakes
        // `logic` while the window is minimized (D3).
        if let Some(next) = self.reminders.next_due() {
            ctx.request_repaint_after((next - Local::now()).to_std().unwrap_or_default());
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tabs")
            .show_separator_line(false)
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin {
                        left: 16,
                        right: 16,
                        top: 14,
                        bottom: 6,
                    }),
            )
            .show(ui, |ui| {
                theme::column(ui, theme::MAX_WIDTH, |ui| {
                    let options = Tab::ALL.map(|tab| (tab, tab.label()));
                    let size = egui::Vec2::new(ui.available_width(), 38.0);
                    theme::segmented(
                        ui,
                        egui::Id::unique("tab-bar"),
                        &mut self.tab,
                        &options,
                        size,
                        0.0,
                    );
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::NONE
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| {
                // Fade and slide the newly selected tab in. Every tab is
                // animated each frame so the incoming one starts from zero.
                let mut shown = 1.0;
                for tab in Tab::ALL {
                    let t = ui.ctx().animate_bool_with_time_and_easing(
                        egui::Id::unique("tab-fade").with(tab.label()),
                        tab == self.tab,
                        0.25,
                        egui::emath::easing::cubic_out,
                    );
                    if tab == self.tab {
                        shown = t;
                    }
                }
                ui.multiply_opacity(shown);
                ui.add_space((1.0 - shown) * 14.0);

                theme::column(ui, theme::MAX_WIDTH, |ui| match self.tab {
                    Tab::Clock => {
                        let mut autostart = self.autostart;
                        let changes = self.clock.ui(ui, &mut self.settings, &mut autostart);
                        if changes.settings_changed {
                            self.settings.save(&self.store);
                        }
                        if changes.autostart_changed {
                            self.set_autostart(autostart);
                        }
                        if changes.pop_out {
                            self.popout.open();
                        }
                    }
                    Tab::Timer => self.timer.ui(ui),
                    Tab::Reminders => {
                        if self.reminders_view.ui(ui, &mut self.reminders) {
                            self.reminders.save(&self.store);
                        }
                    }
                    Tab::Calendar => {
                        if self.calendar.ui(ui, &mut self.reminders) {
                            self.reminders.save(&self.store);
                        }
                    }
                });
            });

        // The floating clock is a child viewport, so it stays available on
        // every tab (and while the main window is hidden in the tray).
        self.popout.sync_settings(&self.settings);
        self.popout.ui(ui.ctx());
    }
}
