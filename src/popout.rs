//! A frameless clock/date window that can float on the desktop.
//!
//! The main window owns a child viewport. `State` lives behind an `Arc<RwLock>`
//! because eframe calls the child's UI callback from its own viewport pass.

use std::sync::{Arc, RwLock};

use chrono::Local;
use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::clock::{format_date, format_time, until_next_tick};
use crate::settings::Settings;
use crate::storage::Store;

pub const FILE: &str = "popout.json";
fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("clock-popout")
}

const DEFAULT_POSITION: [f32; 2] = [120.0, 120.0];
const DEFAULT_SIZE: [f32; 2] = [260.0, 96.0];
const MIN_SIZE: egui::Vec2 = egui::Vec2::new(180.0, 74.0);
const MAX_SIZE: egui::Vec2 = egui::Vec2::new(800.0, 400.0);

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
struct State {
    /// Whether the child window exists.
    open: bool,
    /// Locked widgets cannot be moved, resized or closed by their buttons.
    locked: bool,
    /// Outer window position, in egui points. `None` means "not placed yet".
    position: Option<[f32; 2]>,
    /// Inner window size, in egui points.
    size: [f32; 2],
    /// A copy of the clock format options; saved in `settings.json`, not here.
    #[serde(skip)]
    settings: Settings,
    /// The initial window placement. Kept fixed while the widget moves so the
    /// parent doesn't resend a position every frame and fight the drag.
    #[serde(skip)]
    placement: Option<[f32; 2]>,
    /// The initial window size, for the same reason as `placement`.
    #[serde(skip)]
    placement_size: Option<[f32; 2]>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            locked: false,
            position: None,
            size: DEFAULT_SIZE,
            settings: Settings::default(),
            placement: None,
            placement_size: None,
        }
    }
}

impl State {
    fn load(store: &Store) -> Self {
        store.load_or_default(FILE)
    }

    fn save(&self, store: &Store) {
        if let Err(err) = store.save(FILE, self) {
            eprintln!("clock-app: could not save {FILE}: {err}");
        }
    }

    fn position(&self) -> egui::Pos2 {
        self.position
            .map(|[x, y]| egui::pos2(x, y))
            .unwrap_or_else(|| egui::pos2(DEFAULT_POSITION[0], DEFAULT_POSITION[1]))
    }

    fn set_position(&mut self, pos: egui::Pos2) {
        self.position = Some([pos.x, pos.y]);
    }

    fn size(&self) -> egui::Vec2 {
        egui::vec2(self.size[0], self.size[1])
    }

    fn set_size(&mut self, size: egui::Vec2) {
        let size = size.clamp(MIN_SIZE, MAX_SIZE);
        self.size = [size.x, size.y];
    }

    /// Where the native window is first created. This never changes while it is
    /// open: dragging sends `OuterPosition` commands directly to the child.
    fn placement(&self) -> egui::Pos2 {
        self.placement
            .map(|[x, y]| egui::pos2(x, y))
            .unwrap_or_else(|| self.position())
    }

    fn placement_size(&self) -> egui::Vec2 {
        self.placement_size
            .map(|[x, y]| egui::vec2(x, y))
            .unwrap_or_else(|| self.size())
    }

    /// Freeze the current layout as the native window's starting geometry.
    fn freeze_layout(&mut self) {
        if self.position.is_none() {
            let pos = self.position();
            self.set_position(pos);
        }
        self.placement = self.position;
        self.placement_size = Some([self.size[0], self.size[1]]);
    }
}

/// Keeps the shared state and gives the main tab a handle to open the window.
pub struct Popout {
    state: Arc<RwLock<State>>,
    store: Arc<Store>,
}

impl Popout {
    pub fn new(store: Store) -> Self {
        let store = Arc::new(store);
        let mut loaded = State::load(&store);
        loaded.freeze_layout();
        let state = Arc::new(RwLock::new(loaded));
        Self { state, store }
    }

    /// Called by the "Pop out clock" button.
    pub fn open(&self) {
        let mut state = self.state.write().expect("popout state lock");
        if !state.open {
            state.open = true;
            state.freeze_layout();
            state.save(&self.store);
        }
    }

    /// Keep the floating clock in sync with the Clock tab's format options.
    pub fn sync_settings(&self, settings: &Settings) {
        let mut state = self.state.write().expect("popout state lock");
        if state.settings != *settings {
            state.settings = settings.clone();
        }
    }

    /// Called once per main-window pass. Deferred viewports let the floating
    /// clock repaint every second without waking the main window too.
    pub fn ui(&self, ctx: &egui::Context) {
        let (open, placement, size) = {
            let state = self.state.read().expect("popout state lock");
            (state.open, state.placement(), state.placement_size())
        };
        if !open {
            return;
        }

        let builder = egui::ViewportBuilder::default()
            .with_title("Clock")
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_always_on_top()
            .with_position(placement)
            .with_inner_size(size)
            .with_min_inner_size(MIN_SIZE)
            .with_max_inner_size(MAX_SIZE);

        let state = Arc::clone(&self.state);
        let store = Arc::clone(&self.store);
        ctx.show_viewport_deferred(viewport_id(), builder, move |ui, _class| {
            render(ui, &state, &store);
        });
    }
}

fn set_locked(state: &Arc<RwLock<State>>, store: &Arc<Store>, locked: bool) {
    let mut current = state.write().expect("popout state lock");
    current.locked = locked;
    current.save(store);
}

/// Let Windows handle the resize gesture too, for the same reason as dragging.
fn resize_window(
    ui: &egui::Ui,
    state: &Arc<RwLock<State>>,
    store: &Arc<Store>,
    response: egui::Response,
) {
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                egui::viewport::ResizeDirection::SouthEast,
            ));
    }
    if response.drag_stopped() {
        let native_size = ui.input(|input| input.viewport().inner_rect.map(|rect| rect.size()));
        if let Some(size) = native_size {
            let mut current = state.write().expect("popout state lock");
            current.set_size(size);
            current.save(store);
        }
    }
}

fn close(ui: &egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    let mut current = state.write().expect("popout state lock");
    current.open = false;
    current.save(store);
    drop(current);
    ui.ctx()
        .send_viewport_cmd(egui::ViewportCommand::Visible(false));
}

/// Let Windows move the whole viewport while the pointer is held down.
/// Driving it with `OuterPosition` commands from egui made dragging lag, because
/// moving the window under the pointer changes the next pointer delta.
fn drag_window(
    ui: &egui::Ui,
    state: &Arc<RwLock<State>>,
    store: &Arc<Store>,
    response: egui::Response,
) {
    if response.drag_started() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if response.drag_stopped() {
        let native_position =
            ui.input(|input| input.viewport().outer_rect.map(|rect| rect.left_top()));
        if let Some(pos) = native_position {
            let mut current = state.write().expect("popout state lock");
            current.set_position(pos);
            current.save(store);
        }
    }
}

fn render(ui: &mut egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    if ui.input(|input| input.viewport().close_requested()) {
        close(ui, state, store);
        return;
    }

    let (locked, settings) = {
        let current = state.read().expect("popout state lock");
        (current.locked, current.settings.clone())
    };

    let panel = egui::CentralPanel::default().show(ui, |ui| {
        if !locked {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                if ui.button("Close").clicked() {
                    close(ui, state, store);
                }
                if ui.button("Lock").clicked() {
                    set_locked(state, store, true);
                }
            });
        }

        ui.vertical_centered(|ui| {
            let now = Local::now();
            ui.add_space(6.0);
            let sense = if locked {
                egui::Sense::hover()
            } else {
                egui::Sense::drag()
            };
            let time = ui.add(
                egui::Label::new(
                    egui::RichText::new(format_time(&now, &settings))
                        .size(40.0)
                        .monospace(),
                )
                .sense(sense),
            );
            let date = ui.add(
                egui::Label::new(egui::RichText::new(format_date(&now)).size(14.0)).sense(sense),
            );
            if !locked {
                drag_window(
                    ui,
                    state,
                    store,
                    time.on_hover_cursor(egui::CursorIcon::Grab),
                );
                drag_window(
                    ui,
                    state,
                    store,
                    date.on_hover_cursor(egui::CursorIcon::Grab),
                );
            }
            ui.ctx()
                .request_repaint_after(until_next_tick(&now, settings.show_seconds));
        });

        if !locked {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::BOTTOM), |ui| {
                let grip = ui.allocate_response(egui::Vec2::splat(16.0), egui::Sense::drag());
                let rect = grip.rect;
                ui.painter().line_segment(
                    [rect.left_bottom(), rect.right_top()],
                    ui.visuals().widgets.noninteractive.fg_stroke,
                );
                resize_window(ui, state, store, grip);
            });
        }
    });

    // A container context menu also opens when the click lands on the time or
    // date text, which a plain response-based context menu can miss.
    panel.response.container_context_menu(|ui| {
        if locked {
            if ui.button("Unlock").clicked() {
                set_locked(state, store, false);
            }
        } else if ui.button("Lock").clicked() {
            set_locked(state, store, true);
        }
        if ui.button("Close pop-out").clicked() {
            close(ui, state, store);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store(label: &str) -> Store {
        let dir =
            std::env::temp_dir().join(format!("clock-app-popout-{}-{label}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Store::at(dir)
    }

    #[test]
    fn a_new_popout_is_closed_at_the_default_place() {
        let state = State::default();
        assert!(!state.open);
        assert!(!state.locked);
        assert_eq!(state.position(), egui::pos2(120.0, 120.0));
        assert_eq!(state.size(), egui::vec2(260.0, 96.0));
    }

    #[test]
    fn sizes_are_clamped_to_sensible_limits() {
        let mut state = State::default();
        state.set_size(egui::vec2(10.0, 10.0));
        assert_eq!(state.size(), MIN_SIZE);

        state.set_size(egui::vec2(2000.0, 2000.0));
        assert_eq!(state.size(), MAX_SIZE);
    }

    #[test]
    fn moving_updates_the_saved_layout_but_not_the_window_placement() {
        let mut state = State {
            position: Some([100.0, 120.0]),
            ..Default::default()
        };
        state.freeze_layout();

        state.set_position(egui::pos2(400.0, 260.0));
        state.set_size(egui::vec2(320.0, 110.0));

        assert_eq!(state.position(), egui::pos2(400.0, 260.0));
        assert_eq!(state.size(), egui::vec2(320.0, 110.0));
        assert_eq!(state.placement(), egui::pos2(100.0, 120.0));
        assert_eq!(
            state.placement_size(),
            egui::vec2(DEFAULT_SIZE[0], DEFAULT_SIZE[1])
        );
    }

    #[test]
    fn saves_and_loads_layout_without_duplicating_settings() {
        let store = temp_store("roundtrip");
        let mut state = State {
            open: true,
            locked: true,
            settings: Settings {
                clock_24h: false,
                show_seconds: false,
            },
            ..Default::default()
        };
        state.set_position(egui::pos2(640.0, 200.0));
        state.set_size(egui::vec2(320.0, 110.0));

        state.save(&store);
        let loaded = State::load(&store);
        assert!(loaded.open);
        assert!(loaded.locked);
        assert_eq!(loaded.position(), egui::pos2(640.0, 200.0));
        assert_eq!(loaded.size(), egui::vec2(320.0, 110.0));
        assert_eq!(loaded.settings, Settings::default());
    }
}
