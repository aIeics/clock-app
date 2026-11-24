use std::time::{Duration, Instant};

use std::f32::consts::TAU;

use eframe::egui::{self, FontId, Pos2, Stroke, Vec2};

use crate::theme::{self, Icon};

/// Where a countdown is in its lifecycle.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Idle,
    Running,
    Paused,
    Finished,
}

/// Countdown state machine. Kept free of UI code so the rules can be tested.
///
/// Time is read from `Instant`, which is monotonic: changing the system clock
/// cannot speed up, slow down or reverse a running timer.
#[derive(Debug, Default)]
pub struct Countdown {
    total: Duration,
    /// Time left at `since` when running, or the frozen value otherwise.
    left: Duration,
    /// When the current run started. `None` unless running.
    since: Option<Instant>,
    state: State,
}

impl Countdown {
    /// Used by the tests to set up a specific total before `start`.
    #[cfg(test)]
    pub fn new(total: Duration) -> Self {
        Self {
            total,
            left: total,
            since: None,
            state: State::Idle,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn total(&self) -> Duration {
        self.total
    }

    /// Time left at `now`, never negative.
    pub fn remaining(&self, now: Instant) -> Duration {
        match self.since {
            Some(since) => self
                .left
                .saturating_sub(now.saturating_duration_since(since)),
            None => self.left,
        }
    }

    /// Starts a fresh countdown from `total`.
    pub fn start(&mut self, total: Duration, now: Instant) {
        self.total = total;
        self.left = total;
        self.since = Some(now);
        self.state = if total.is_zero() {
            State::Finished
        } else {
            State::Running
        };
    }

    /// Freezes the countdown where it is.
    pub fn pause(&mut self, now: Instant) {
        if self.state == State::Running {
            self.left = self.remaining(now);
            self.since = None;
            self.state = State::Paused;
        }
    }

    /// Continues a paused countdown from where it stopped.
    pub fn resume(&mut self, now: Instant) {
        if self.state == State::Paused && !self.left.is_zero() {
            self.since = Some(now);
            self.state = State::Running;
        }
    }

    /// Back to idle with the full configured time showing.
    pub fn reset(&mut self) {
        self.left = self.total;
        self.since = None;
        self.state = State::Idle;
    }

    /// Moves a finished countdown to `Finished` and reports the transition once.
    pub fn tick(&mut self, now: Instant) -> bool {
        if self.state == State::Running && self.remaining(now).is_zero() {
            self.left = Duration::ZERO;
            self.since = None;
            self.state = State::Finished;
            return true;
        }
        false
    }
}

/// `02:07`, or `1:02:07` for timers an hour or longer.
pub fn format_duration(left: Duration) -> String {
    let seconds = left.as_secs();
    let (hours, minutes, seconds) = (seconds / 3600, (seconds / 60) % 60, seconds % 60);

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

/// Countdown UI. The buttons follow the state machine; editing is only allowed
/// while idle, so a running timer can't be changed underneath itself.
#[derive(Debug)]
pub struct TimerView {
    hours: u32,
    minutes: u32,
    seconds: u32,
    countdown: Countdown,
    alert_sent: bool,
}

impl Default for TimerView {
    fn default() -> Self {
        Self {
            hours: 0,
            minutes: 5,
            seconds: 0,
            countdown: Countdown::default(),
            alert_sent: false,
        }
    }
}

impl TimerView {
    fn total(&self) -> Duration {
        Duration::from_secs(
            u64::from(self.hours) * 3600 + u64::from(self.minutes) * 60 + u64::from(self.seconds),
        )
    }

    /// Called from `App::logic`, so the timer also finishes while the window is
    /// hidden. Repaints only happen while the countdown is running.
    pub fn logic(&mut self, ctx: &egui::Context) {
        let now = Instant::now();

        if self.countdown.tick(now) && !self.alert_sent {
            self.alert_sent = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Critical,
            ));
        }

        if self.countdown.state() == State::Running {
            let left = self.countdown.remaining(now);
            ctx.request_repaint_after(Duration::from_millis(100).min(left));
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let state = self.countdown.state();
        let now = Instant::now();
        let editable = matches!(state, State::Idle | State::Finished);

        // While idle, show the time that Start would count down from.
        let (shown, fraction) = match state {
            State::Idle => (self.total(), 1.0),
            _ => {
                let left = self.countdown.remaining(now);
                let total = self.countdown.total();
                let fraction = if total.is_zero() {
                    0.0
                } else {
                    (left.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
                };
                (left, fraction)
            }
        };
        // Glide the ring on start, reset and finish. While running it follows
        // the clock directly, at the repaint rate `logic` already schedules.
        let glide = if state == State::Running { 0.0 } else { 0.45 };
        let fraction =
            ui.ctx()
                .animate_value_with_time(egui::Id::unique("timer-ring"), fraction, glide);

        let avail = ui.available_size();
        let diameter = (avail.x * 0.75).min(avail.y - 250.0).clamp(120.0, 400.0);

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(((avail.y - diameter - 240.0) * 0.4).max(4.0));
                    self.ring(ui, diameter, fraction, shown, state);
                    ui.add_space(16.0);

                    ui.add_enabled_ui(editable, |ui| {
                        self.presets(ui);
                        ui.add_space(4.0);
                        self.steppers(ui);
                    });
                    ui.add_space(16.0);
                    self.controls(ui, state);
                    ui.add_space(8.0);
                });
            });
    }

    /// The countdown ring with the time and state in the middle.
    fn ring(&self, ui: &mut egui::Ui, diameter: f32, fraction: f32, shown: Duration, state: State) {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(diameter), egui::Sense::hover());
        let painter = ui.painter();
        let center = rect.center();
        let width = (diameter * 0.035).clamp(5.0, 12.0);
        let radius = diameter / 2.0 - width;
        let finished = state == State::Finished;

        let track = if finished {
            theme::DANGER.gamma_multiply(0.6)
        } else {
            theme::SURFACE_HI
        };
        painter.circle_stroke(center, radius, Stroke::new(width, track));

        if fraction > 0.001 {
            const STEPS: usize = 180;
            let steps = ((STEPS as f32 * fraction).ceil() as usize).max(2);
            let at = |turn: f32| center + Vec2::angled(turn * TAU - TAU / 4.0) * radius;
            let points: Vec<Pos2> = (0..=steps)
                .map(|i| at(fraction * i as f32 / steps as f32))
                .collect();
            painter.circle_filled(points[0], width / 2.0, theme::TEXT);
            painter.circle_filled(points[steps], width / 2.0, theme::TEXT);
            painter.line(points, Stroke::new(width, theme::TEXT));
        }

        // A one-shot pop when the timer finishes; it settles and stays still.
        let pop = ui.ctx().animate_bool_with_time_and_easing(
            egui::Id::unique("timer-finished"),
            finished,
            0.4,
            egui::emath::easing::back_out,
        );
        let size = (diameter * 0.2).clamp(30.0, 80.0) * (1.0 + 0.06 * pop);
        let color = if finished { theme::DANGER } else { theme::TEXT };
        theme::tabular_text(
            painter,
            center - Vec2::new(0.0, size * 0.12),
            &format_duration(shown),
            FontId::proportional(size),
            color,
        );
        let label = match state {
            State::Idle => "READY",
            State::Running => "RUNNING",
            State::Paused => "PAUSED",
            State::Finished => "TIME\u{2019}S UP",
        };
        painter.text(
            center + Vec2::new(0.0, size * 0.62),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional((size * 0.2).clamp(11.0, 16.0)),
            if finished {
                theme::DANGER
            } else {
                theme::TEXT_DIM
            },
        );
    }

    /// Quick picks for common lengths.
    fn presets(&mut self, ui: &mut egui::Ui) {
        const PRESETS: [(&str, u32); 4] =
            [("1 min", 1), ("5 min", 5), ("10 min", 10), ("25 min", 25)];
        let spacing = ui.spacing().item_spacing.x;
        let width = 4.0 * 72.0 + 3.0 * spacing;
        centered_row(ui, width, |ui| {
            for (label, minutes) in PRESETS {
                if theme::button(ui, label, theme::Kind::Ghost).clicked() {
                    (self.hours, self.minutes, self.seconds) = (0, minutes, 0);
                }
            }
        });
    }

    /// Hours, minutes and seconds, each with + and - buttons.
    fn steppers(&mut self, ui: &mut egui::Ui) {
        theme::card()
            .inner_margin(egui::Margin::symmetric(12, 10))
            .show(ui, |ui| {
                ui.columns(3, |columns| {
                    stepper(&mut columns[0], "Hours", &mut self.hours, 99);
                    stepper(&mut columns[1], "Minutes", &mut self.minutes, 59);
                    stepper(&mut columns[2], "Seconds", &mut self.seconds, 59);
                });
            });
    }

    /// Start/pause/resume as one big round button, with reset beside it.
    fn controls(&mut self, ui: &mut egui::Ui, state: State) {
        let resettable = state != State::Idle;
        let spacing = ui.spacing().item_spacing.x;
        let width = if resettable {
            64.0 + spacing + 48.0
        } else {
            64.0
        };
        centered_row(ui, width, |ui| {
            let icon = if state == State::Running {
                Icon::Pause
            } else {
                Icon::Play
            };
            if theme::icon_button(ui, icon, 64.0, true).clicked() {
                let now = Instant::now();
                match state {
                    State::Idle | State::Finished => {
                        self.countdown.start(self.total(), now);
                        self.alert_sent = false;
                    }
                    State::Running => self.countdown.pause(now),
                    State::Paused => self.countdown.resume(now),
                }
            }
            if resettable && theme::icon_button(ui, Icon::Reset, 48.0, false).clicked() {
                self.countdown.reset();
                self.alert_sent = false;
            }
        });
    }
}

/// A horizontal row of `width`, centred in the available space.
fn centered_row(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        add(ui);
    });
}

/// One column of the duration picker: the label above a `- 05 +` row.
fn stepper(ui: &mut egui::Ui, label: &str, value: &mut u32, max: u32) {
    ui.vertical_centered(|ui| {
        theme::section_label(ui, label);
        let spacing = ui.spacing().item_spacing.x;
        centered_row(ui, 26.0 * 2.0 + 30.0 + spacing * 2.0, |ui| {
            if theme::icon_button(ui, Icon::Minus, 26.0, false).clicked() {
                *value = if *value == 0 { max } else { *value - 1 };
            }
            let (rect, _) = ui.allocate_exact_size(Vec2::new(30.0, 26.0), egui::Sense::hover());
            let color = if ui.is_enabled() {
                theme::TEXT
            } else {
                theme::TEXT_DIM
            };
            theme::tabular_text(
                ui.painter(),
                rect.center(),
                &format!("{value:02}"),
                FontId::proportional(20.0),
                color,
            );
            if theme::icon_button(ui, Icon::Plus, 26.0, false).clicked() {
                *value = if *value >= max { 0 } else { *value + 1 };
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_short_and_long_durations() {
        assert_eq!(format_duration(Duration::from_secs(0)), "00:00");
        assert_eq!(format_duration(Duration::from_secs(7)), "00:07");
        assert_eq!(format_duration(Duration::from_secs(125)), "02:05");
        assert_eq!(format_duration(Duration::from_secs(3727)), "1:02:07");
    }

    #[test]
    fn counts_down_while_running() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(10));
        timer.start(Duration::from_secs(10), start);

        assert_eq!(timer.remaining(start), Duration::from_secs(10));
        assert_eq!(
            timer.remaining(start + Duration::from_secs(3)),
            Duration::from_secs(7)
        );
        assert!(!timer.tick(start + Duration::from_secs(3)));
    }

    #[test]
    fn pause_freezes_and_resume_continues() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(10));
        timer.start(Duration::from_secs(10), start);

        timer.pause(start + Duration::from_secs(3));
        assert_eq!(timer.state(), State::Paused);
        assert_eq!(
            timer.remaining(start + Duration::from_secs(100)),
            Duration::from_secs(7)
        );

        timer.resume(start + Duration::from_secs(100));
        assert_eq!(timer.state(), State::Running);
        assert_eq!(
            timer.remaining(start + Duration::from_secs(102)),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn reset_goes_back_to_idle_with_the_full_time() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(10));
        timer.start(Duration::from_secs(10), start);
        timer.pause(start + Duration::from_secs(4));
        timer.reset();

        assert_eq!(timer.state(), State::Idle);
        assert_eq!(
            timer.remaining(start + Duration::from_secs(400)),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn finishes_once_at_zero() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(2));
        timer.start(Duration::from_secs(2), start);

        assert!(!timer.tick(start + Duration::from_secs(1)));
        assert!(timer.tick(start + Duration::from_secs(2)));
        assert_eq!(timer.state(), State::Finished);
        assert!(!timer.tick(start + Duration::from_secs(3)));
        assert_eq!(
            timer.remaining(start + Duration::from_millis(999_999)),
            Duration::ZERO
        );
    }

    #[test]
    fn never_goes_below_zero() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(1));
        timer.start(Duration::from_secs(1), start);

        assert_eq!(
            timer.remaining(start + Duration::from_secs(10_000)),
            Duration::ZERO
        );
    }

    #[test]
    fn a_zero_length_timer_finishes_immediately() {
        let mut timer = Countdown::new(Duration::ZERO);
        timer.start(Duration::ZERO, Instant::now());
        assert_eq!(timer.state(), State::Finished);
    }

    #[test]
    fn stray_pause_and_resume_do_not_change_the_state() {
        let start = Instant::now();
        let mut timer = Countdown::new(Duration::from_secs(10));
        timer.pause(start);
        assert_eq!(timer.state(), State::Idle);
        timer.resume(start);
        assert_eq!(timer.state(), State::Idle);

        timer.start(Duration::from_secs(10), start);
        timer.resume(start + Duration::from_secs(1));
        assert_eq!(timer.state(), State::Running);
        assert_eq!(
            timer.remaining(start + Duration::from_secs(1)),
            Duration::from_secs(9)
        );
    }
}
