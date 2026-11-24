use std::time::{Duration, Instant};

use eframe::egui;

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
        let left = self.countdown.remaining(now);
        let total = self.countdown.total();
        let progress = if total.is_zero() {
            1.0
        } else {
            1.0 - (left.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
        };
        let editable = matches!(state, State::Idle | State::Finished);

        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() * 0.08);
            ui.label(
                egui::RichText::new(format_duration(left))
                    .size(56.0)
                    .monospace(),
            );
            ui.add_space(8.0);
            ui.add(
                egui::ProgressBar::new(progress)
                    .desired_width(260.0)
                    .fill(egui::Color32::from_rgb(59, 130, 246)),
            );
            ui.add_space(16.0);

            ui.add_enabled_ui(editable, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Hours");
                    ui.add(egui::DragValue::new(&mut self.hours).range(0..=99));
                    ui.label("Minutes");
                    ui.add(egui::DragValue::new(&mut self.minutes).range(0..=59));
                    ui.label("Seconds");
                    ui.add(egui::DragValue::new(&mut self.seconds).range(0..=59));
                });
            });

            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let total = self.total();
                match state {
                    State::Idle | State::Finished => {
                        if ui.button("Start").clicked() {
                            self.countdown.start(total, Instant::now());
                            self.alert_sent = false;
                        }
                    }
                    State::Running => {
                        if ui.button("Pause").clicked() {
                            self.countdown.pause(Instant::now());
                        }
                    }
                    State::Paused => {
                        if ui.button("Resume").clicked() {
                            self.countdown.resume(Instant::now());
                        }
                    }
                }

                if state != State::Idle && ui.button("Reset").clicked() {
                    self.countdown.reset();
                    self.alert_sent = false;
                }
            });

            if state == State::Finished {
                ui.add_space(8.0);
                ui.colored_label(egui::Color32::from_rgb(239, 68, 68), "Time's up!");
            }
        });
    }
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
