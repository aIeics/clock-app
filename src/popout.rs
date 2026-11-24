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
use crate::theme::PopoutFont;

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
    /// Popout text color as RGBA bytes.
    text_color: [u8; 4],
    /// Font used by the popout clock text.
    font: PopoutFont,
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
    /// Active native move/resize gesture, so we start each one only once.
    #[serde(skip)]
    dragging: bool,
    #[serde(skip)]
    resizing: bool,
    /// The window has left its one-frame fullscreen creation state. See the
    /// builder comment in `Popout::ui` for why that state is needed.
    #[serde(skip)]
    initialized: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            open: false,
            locked: true,
            position: None,
            size: DEFAULT_SIZE,
            text_color: [255, 255, 255, 255],
            font: PopoutFont::default(),
            settings: Settings::default(),
            placement: None,
            placement_size: None,
            dragging: false,
            resizing: false,
            initialized: false,
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

    fn color(&self) -> egui::Color32 {
        let [r, g, b, a] = self.text_color;
        egui::Color32::from_rgba_unmultiplied(r, g, b, a)
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
            state.initialized = false;
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

        // Windows adds a drop shadow (and a 1px border) to undecorated windows.
        // Creating the viewport fullscreen disables that shadow; the first child
        // pass leaves fullscreen and restores the saved geometry before painting.
        let builder = egui::ViewportBuilder::default()
            .with_title("Clock")
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_window_level(egui::viewport::WindowLevel::AlwaysOnBottom)
            .with_fullscreen(true)
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

fn keep_behind_apps(ui: &egui::Ui) {
    ui.ctx()
        .send_viewport_cmd(egui::ViewportCommand::WindowLevel(
            egui::viewport::WindowLevel::AlwaysOnBottom,
        ));
}

fn set_locked(ui: &egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>, locked: bool) {
    let mut current = state.write().expect("popout state lock");
    current.locked = locked;
    current.dragging = false;
    current.resizing = false;
    current.save(store);
    drop(current);
    ui.ctx()
        .memory_mut(|mem| mem.move_focus(egui::FocusDirection::None));
    keep_behind_apps(ui);
}

fn set_font(ui: &egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>, font: PopoutFont) {
    let mut current = state.write().expect("popout state lock");
    current.font = font;
    current.save(store);
    drop(current);
    ui.close();
    keep_behind_apps(ui);
}

fn set_text_color(ui: &egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>, color: [u8; 4]) {
    let mut current = state.write().expect("popout state lock");
    current.text_color = color;
    current.save(store);
    drop(current);
    ui.close();
    keep_behind_apps(ui);
}

fn close(ui: &egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    let mut current = state.write().expect("popout state lock");
    current.open = false;
    current.dragging = false;
    current.resizing = false;
    current.initialized = false;
    current.save(store);
    drop(current);
    ui.ctx()
        .send_viewport_cmd(egui::ViewportCommand::Visible(false));
}

/// Let Windows handle the resize gesture. `is_pointer_button_down_on` is true
/// from the press frame, unlike `drag_started`, so unlocking doesn't swallow
/// the first drag.
fn resize_window(
    ui: &egui::Ui,
    state: &Arc<RwLock<State>>,
    store: &Arc<Store>,
    response: egui::Response,
) {
    let primary_down = ui.input(|input| input.pointer.primary_down());
    let pressed_here = primary_down && response.is_pointer_button_down_on();

    let mut current = state.write().expect("popout state lock");
    if pressed_here && !current.resizing {
        current.resizing = true;
        drop(current);
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        keep_behind_apps(ui);
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::BeginResize(
                egui::viewport::ResizeDirection::SouthEast,
            ));
        return;
    }
    if !primary_down && current.resizing {
        current.resizing = false;
        let native_size = ui.input(|input| input.viewport().inner_rect.map(|rect| rect.size()));
        if let Some(size) = native_size {
            current.set_size(size);
        }
        current.save(store);
        drop(current);
        keep_behind_apps(ui);
    }
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
    let primary_down = ui.input(|input| input.pointer.primary_down());
    let pressed_here = primary_down && response.is_pointer_button_down_on();

    let mut current = state.write().expect("popout state lock");
    if pressed_here && !current.dragging {
        current.dragging = true;
        drop(current);
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
        keep_behind_apps(ui);
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        return;
    }
    if !primary_down && current.dragging {
        current.dragging = false;
        let native_position =
            ui.input(|input| input.viewport().outer_rect.map(|rect| rect.left_top()));
        if let Some(pos) = native_position {
            current.set_position(pos);
        }
        current.save(store);
        drop(current);
        keep_behind_apps(ui);
    }
}

fn render(ui: &mut egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    if ui.input(|input| input.viewport().close_requested()) {
        close(ui, state, store);
        return;
    }

    let (locked, settings, position, size, initialized, dragging, resizing, color, font) = {
        let current = state.read().expect("popout state lock");
        (
            current.locked,
            current.settings.clone(),
            current.position(),
            current.size(),
            current.initialized,
            current.dragging,
            current.resizing,
            current.color(),
            current.font,
        )
    };

    if !initialized {
        state.write().expect("popout state lock").initialized = true;
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::OuterPosition(position));
        ui.ctx()
            .send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        keep_behind_apps(ui);
    }

    let panel = egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let now = Local::now();
                ui.add_space(6.0);
                // `Sense::drag()` also makes the label focusable, which draws
                // a focus outline. Use the raw drag sense and keep the labels
                // non-selectable so only the text is visible.
                let sense = if locked {
                    egui::Sense::hover()
                } else {
                    egui::Sense::DRAG
                };
                let time = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format_time(&now, &settings))
                            .size(40.0)
                            .family(font.family())
                            .color(color),
                    )
                    .selectable(false)
                    .sense(sense),
                );
                let date = ui.add(
                    egui::Label::new(
                        egui::RichText::new(format_date(&now))
                            .size(14.0)
                            .family(font.secondary_family())
                            .color(color),
                    )
                    .selectable(false)
                    .sense(sense),
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
                    // Invisible resize corner: only the text is drawn.
                    let grip = ui.allocate_response(egui::Vec2::splat(16.0), egui::Sense::DRAG);
                    resize_window(
                        ui,
                        state,
                        store,
                        grip.on_hover_cursor(egui::CursorIcon::ResizeNwSe),
                    );
                });
            }
        });

    // A container context menu also opens when the click lands on the time or
    // date text, which a plain response-based context menu can miss.
    panel.response.container_context_menu(|ui| {
        ui.menu_button("Fonts", |ui| {
            for option in PopoutFont::ALL {
                let selected = option == font;
                if ui.selectable_label(selected, option.label()).clicked() {
                    set_font(ui, state, store, option);
                }
            }
        });
        ui.menu_button("Color", |ui| {
            let colors: [(&str, [u8; 4]); 6] = [
                ("White", [255, 255, 255, 255]),
                ("Amber", [255, 191, 0, 255]),
                ("Green", [0, 255, 128, 255]),
                ("Red", [255, 80, 80, 255]),
                ("Cyan", [0, 220, 255, 255]),
                ("Magenta", [255, 100, 255, 255]),
            ];
            for (name, value) in colors {
                let selected = value == [color.r(), color.g(), color.b(), color.a()];
                if ui.selectable_label(selected, name).clicked() {
                    set_text_color(ui, state, store, value);
                }
            }
        });
        ui.separator();
        if locked {
            if ui.button("Unlock").clicked() {
                set_locked(ui, state, store, false);
            }
        } else if ui.button("Lock").clicked() {
            set_locked(ui, state, store, true);
        }
        if ui.button("Close pop-out").clicked() {
            close(ui, state, store);
        }
    });

    // Windows only applies AlwaysOnBottom when the level changes, so reassert it
    // after focus changes and whenever another window is opened.
    if !dragging && !resizing && !ui.input(|input| input.pointer.any_down()) {
        keep_behind_apps(ui);
    }
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
        assert!(state.locked);
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
            text_color: [0, 255, 128, 255],
            font: PopoutFont::DigitTech,
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
        assert_eq!(loaded.text_color, [0, 255, 128, 255]);
        assert_eq!(loaded.font, PopoutFont::DigitTech);
        assert_eq!(loaded.settings, Settings::default());
    }
}
