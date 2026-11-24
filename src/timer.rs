use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

use crate::theme::{self, Kind};

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
        let editable = matches!(state, State::Idle | State::Finished);
        // Round up, so a fresh 5:00 timer shows 5:00 rather than 4:59.
        let left = Duration::from_secs(
            self.countdown
                .remaining(Instant::now())
                .as_millis()
                .div_ceil(1000) as u64,
        );

        // While a countdown is going, the wheels roll down to show what's left.
        let shown = if editable {
            [self.hours, self.minutes, self.seconds]
        } else {
            let secs = left.as_secs();
            [
                (secs / 3600) as u32,
                ((secs / 60) % 60) as u32,
                (secs % 60) as u32,
            ]
        };

        let avail = ui.available_size();
        let row = (avail.y * 0.075).clamp(30.0, 52.0);
        let gap = 10.0;
        let wheel_w = (row * 2.3).min((avail.x - gap * 2.0) / 3.0);
        let total_w = wheel_w * 3.0 + gap * 2.0;
        let content_h = row * 5.0 + 150.0;
        ui.add_space(((avail.y - content_h) * 0.4).max(8.0));

        let (rect, _) = ui.allocate_exact_size(Vec2::new(total_w, row * 5.0), Sense::hover());
        for y in [rect.center().y - row / 2.0, rect.center().y + row / 2.0] {
            ui.painter().line_segment(
                [
                    Pos2::new(rect.left() - 8.0, y),
                    Pos2::new(rect.right() + 8.0, y),
                ],
                Stroke::new(1.0, Color32::from_rgb(58, 58, 58)),
            );
        }
        let values = [&mut self.hours, &mut self.minutes, &mut self.seconds];
        let names = ["HOURS", "MIN", "SEC"];
        let maxes = [99, 59, 59];
        for (i, value) in values.into_iter().enumerate() {
            let wheel_rect = Rect::from_min_size(
                rect.min + Vec2::new(i as f32 * (wheel_w + gap), 0.0),
                Vec2::new(wheel_w, rect.height()),
            );
            wheel(
                ui,
                wheel_rect,
                egui::Id::unique(("timer-wheel", i)),
                value,
                shown[i],
                maxes[i],
                editable,
            );
        }

        // Labels under the wheels.
        let (labels, _) = ui.allocate_exact_size(Vec2::new(total_w, 16.0), Sense::hover());
        for (i, name) in names.iter().enumerate() {
            ui.painter().text(
                Pos2::new(
                    labels.left() + i as f32 * (wheel_w + gap) + wheel_w / 2.0,
                    labels.center().y,
                ),
                egui::Align2::CENTER_CENTER,
                *name,
                FontId::proportional(10.0),
                theme::TEXT_DIM,
            );
        }

        ui.add_space(6.0);
        let (caption, color) = match state {
            State::Idle => ("Scroll or drag to set".to_string(), theme::TEXT_DIM),
            State::Running => (format!("{} left", format_duration(left)), theme::TEXT_DIM),
            State::Paused => (
                format!("Paused \u{00b7} {} left", format_duration(left)),
                theme::TEXT_DIM,
            ),
            State::Finished => ("Time\u{2019}s up!".to_string(), theme::DANGER),
        };
        ui.label(egui::RichText::new(caption).size(13.0).color(color));
        ui.add_space(4.0);

        let label = match state {
            State::Idle | State::Finished => "Start",
            State::Running => "Pause",
            State::Paused => "Resume",
        };
        let resettable = state != State::Idle;
        let spacing = ui.spacing().item_spacing.x;
        let width = if resettable {
            160.0 + spacing + 90.0
        } else {
            160.0
        };
        centered_row(ui, width, |ui| {
            if theme::button_sized(ui, label, Kind::Primary, 160.0).clicked() {
                let now = Instant::now();
                match state {
                    State::Idle | State::Finished => {
                        let total = self.total();
                        if !total.is_zero() {
                            self.countdown.start(total, now);
                            self.alert_sent = false;
                        }
                    }
                    State::Running => self.countdown.pause(now),
                    State::Paused => self.countdown.resume(now),
                }
            }
            if resettable && theme::button_sized(ui, "Reset", Kind::Ghost, 90.0).clicked() {
                self.countdown.reset();
                self.alert_sent = false;
            }
        });

        if editable {
            ui.add_space(6.0);
            const QUICK: [(&str, u32); 3] = [("5 min", 5), ("10 min", 10), ("25 min", 25)];
            let widths: f32 = QUICK
                .iter()
                .map(|(text, _)| theme::link_width(ui, text) + spacing)
                .sum();
            let lead = theme::link_width(ui, "Quick:");
            centered_row(ui, lead + widths, |ui| {
                ui.label(
                    egui::RichText::new("Quick:")
                        .size(12.5)
                        .color(theme::TEXT_DIM),
                );
                for (text, minutes) in QUICK {
                    if theme::link(ui, text).clicked() {
                        (self.hours, self.minutes, self.seconds) = (0, minutes, 0);
                    }
                }
            });
        }
    }
}

/// A horizontal row of `width`, centred in the available space.
fn centered_row(ui: &mut egui::Ui, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - width) / 2.0).max(0.0));
        add(ui);
    });
}

/// One scroll wheel: five rows visible, the middle one is the value.
/// Drag, scroll or click a row to change it while `editable`; otherwise it
/// shows `shown` and rolls to each new value.
fn wheel(
    ui: &mut egui::Ui,
    rect: Rect,
    id: egui::Id,
    value: &mut u32,
    shown: u32,
    max: u32,
    editable: bool,
) {
    let row = rect.height() / 5.0;
    let sense = if editable {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let response = ui.interact(rect, id, sense);
    let drag_id = id.with("drag");
    let mut drag: f32 = ui.ctx().data(|data| data.get_temp(drag_id).unwrap_or(0.0));

    if editable {
        let step = |value: &mut u32, delta: i64| {
            *value = (i64::from(*value) + delta).clamp(0, i64::from(max)) as u32;
        };
        if response.dragged() {
            drag -= response.drag_delta().y / row;
            while drag >= 0.5 {
                step(value, 1);
                drag -= 1.0;
            }
            while drag <= -0.5 {
                step(value, -1);
                drag += 1.0;
            }
            // Stop the pull at either end.
            if (*value == 0 && drag < 0.0) || (*value == max && drag > 0.0) {
                drag = 0.0;
            }
        } else {
            drag = 0.0;
        }
        if response.hovered() {
            let notches: i64 = ui.input(|input| {
                input
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        egui::Event::MouseWheel { delta, .. } if delta.y != 0.0 => {
                            Some(if delta.y > 0.0 { -1 } else { 1 })
                        }
                        _ => None,
                    })
                    .sum()
            });
            step(value, notches);
        }
        if response.clicked()
            && let Some(pos) = response.interact_pointer_pos()
        {
            step(value, ((pos.y - rect.center().y) / row).round() as i64);
        }
    }
    ui.ctx().data_mut(|data| data.insert_temp(drag_id, drag));

    let target = if editable { *value } else { shown };
    let mut pos = theme::animate(ui.ctx(), id.with("pos"), target as f32, 0.25);
    if response.dragged() {
        pos = target as f32 + drag;
    }

    let painter = ui.painter().with_clip_rect(rect);
    let first = (pos.floor() as i64 - 3).max(0);
    let last = (pos.ceil() as i64 + 3).min(i64::from(max));
    for i in first..=last {
        let offset = i as f32 - pos;
        let distance = offset.abs();
        let opacity = (1.0 - distance * 0.38).max(0.12);
        let scale = (1.0 - distance * 0.08).max(0.78);
        painter.text(
            Pos2::new(rect.center().x, rect.center().y + offset * row),
            egui::Align2::CENTER_CENTER,
            format!("{i:02}"),
            theme::bold(row * 0.72 * scale),
            theme::TEXT.gamma_multiply(opacity),
        );
    }

    if editable {
        response.on_hover_cursor(egui::CursorIcon::ResizeVertical);
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
