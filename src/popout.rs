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
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            locked: false,
            position: None,
            size: DEFAULT_SIZE,
            settings: Settings::default(),
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
}

/// Keeps the shared state and gives the main tab a handle to open the window.
pub struct Popout {
    state: Arc<RwLock<State>>,
    store: Arc<Store>,
}

impl Popout {
    pub fn new(store: Store) -> Self {
        let store = Arc::new(store);
        let state = Arc::new(RwLock::new(State::load(&store)));
        Self { state, store }
    }

    /// Called by the "Pop out clock" button.
    pub fn open(&self) {
        let mut state = self.state.write().expect("popout state lock");
        if !state.open {
            state.open = true;
            if state.position.is_none() {
                let pos = state.position();
                state.set_position(pos);
            }
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
        let (open, position, size) = {
            let state = self.state.read().expect("popout state lock");
            (state.open, state.position(), state.size())
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
            .with_position(position)
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

fn render(ui: &mut egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    if ui.input(|input| input.viewport().close_requested()) {
        let mut current = state.write().expect("popout state lock");
        current.open = false;
        current.save(store);
        drop(current);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        return;
    }

    let (locked, settings) = {
        let current = state.read().expect("popout state lock");
        (current.locked, current.settings.clone())
    };

    egui::CentralPanel::default().show(ui, |ui| {
        // Right-click anywhere on the widget for Lock/Unlock and Close.
        let background = ui.interact(
            ui.max_rect(),
            ui.scope_id().with("popout_background"),
            egui::Sense::hover(),
        );
        background.context_menu(|ui| {
            let mut current = state.write().expect("popout state lock");
            if current.locked {
                if ui.button("Unlock").clicked() {
                    current.locked = false;
                    current.save(store);
                }
            } else if ui.button("Lock").clicked() {
                current.locked = true;
                current.save(store);
            }
            if ui.button("Close pop-out").clicked() {
                current.open = false;
                current.save(store);
                drop(current);
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        });

        if !locked {
            ui.horizontal(|ui| {
                let drag = ui.add(egui::Label::new("⠿ Drag").sense(egui::Sense::drag()));
                if drag.dragged() {
                    let mut current = state.write().expect("popout state lock");
                    let pos = current.position() + drag.drag_delta();
                    current.set_position(pos);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
                }
                if drag.drag_stopped() {
                    state.read().expect("popout state lock").save(store);
                }

                if ui.button("Lock").clicked() {
                    let mut current = state.write().expect("popout state lock");
                    current.locked = true;
                    current.save(store);
                }
                if ui.button("Close").clicked() {
                    let mut current = state.write().expect("popout state lock");
                    current.open = false;
                    current.save(store);
                    drop(current);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
            });
        }

        ui.vertical_centered(|ui| {
            let now = Local::now();
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format_time(&now, &settings))
                    .size(40.0)
                    .monospace(),
            );
            ui.label(egui::RichText::new(format_date(&now)).size(14.0));
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
                if grip.dragged() {
                    let mut current = state.write().expect("popout state lock");
                    let size = current.size() + grip.drag_delta();
                    current.set_size(size);
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::InnerSize(current.size()));
                }
                if grip.drag_stopped() {
                    state.read().expect("popout state lock").save(store);
                }
            });
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
