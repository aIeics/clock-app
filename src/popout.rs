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
fn menu_viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("clock-popout-menu")
}

const DEFAULT_POSITION: [f32; 2] = [120.0, 120.0];
const DEFAULT_SIZE: [f32; 2] = [260.0, 96.0];
const MIN_SIZE: egui::Vec2 = egui::Vec2::new(180.0, 74.0);
const MAX_SIZE: egui::Vec2 = egui::Vec2::new(800.0, 400.0);
/// Width of the right-click menu window. Its height follows its content.
const MENU_WIDTH: f32 = 190.0;
/// First guess at the menu size, used to keep it on screen before it is measured.
const MENU_ESTIMATE: egui::Vec2 = egui::Vec2::new(MENU_WIDTH, 230.0);
const TEXT_COLORS: [(&str, [u8; 4]); 7] = [
    ("White", [255, 255, 255, 255]),
    ("Black", [0, 0, 0, 255]),
    ("Amber", [255, 191, 0, 255]),
    ("Green", [0, 255, 128, 255]),
    ("Red", [255, 80, 80, 255]),
    ("Cyan", [0, 220, 255, 255]),
    ("Magenta", [255, 100, 255, 255]),
];

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
    /// Screen position of the open right-click menu, in points.
    #[serde(skip)]
    menu: Option<[f32; 2]>,
    /// The menu window has had focus, so losing it now means "clicked away".
    #[serde(skip)]
    menu_focused: bool,
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
            menu: None,
            menu_focused: false,
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

/// Commands here always target the pop-out window, also when they are sent
/// from the menu window's pass.
fn keep_behind_apps(ctx: &egui::Context) {
    ctx.send_viewport_cmd_to(
        viewport_id(),
        egui::ViewportCommand::WindowLevel(egui::viewport::WindowLevel::AlwaysOnBottom),
    );
}

/// Apply a menu choice to the saved state, then close the menu.
fn choose(
    ctx: &egui::Context,
    state: &Arc<RwLock<State>>,
    store: &Arc<Store>,
    apply: impl FnOnce(&mut State),
) {
    let mut current = state.write().expect("popout state lock");
    apply(&mut current);
    current.save(store);
    drop(current);
    close_menu(ctx, state);
    keep_behind_apps(ctx);
}

fn close(ctx: &egui::Context, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    let mut current = state.write().expect("popout state lock");
    current.open = false;
    current.dragging = false;
    current.resizing = false;
    current.initialized = false;
    current.menu = None;
    current.save(store);
    drop(current);
    ctx.send_viewport_cmd_to(viewport_id(), egui::ViewportCommand::Visible(false));
    // Hide the menu now; the main window's next pass then drops both viewports.
    ctx.send_viewport_cmd_to(menu_viewport_id(), egui::ViewportCommand::Visible(false));
    ctx.request_repaint_of(egui::ViewportId::ROOT);
}

/// Where the menu window opens for a right-click at `click` (screen points).
/// It flips left or up when it would run off the monitor. egui only reports
/// the monitor's size, not where it is, so this assumes the monitor starts at
/// (0, 0) and leaves clicks outside that area alone.
fn menu_position(click: egui::Pos2, menu: egui::Vec2, monitor: Option<egui::Vec2>) -> egui::Pos2 {
    let Some(monitor) = monitor else {
        return click;
    };
    if !egui::Rect::from_min_size(egui::Pos2::ZERO, monitor).contains(click) {
        return click;
    }
    let flip = |at: f32, len: f32, limit: f32| {
        if at + len > limit {
            (at - len).max(0.0)
        } else {
            at
        }
    };
    egui::pos2(
        flip(click.x, menu.x, monitor.x),
        flip(click.y, menu.y, monitor.y),
    )
}

fn open_menu(ctx: &egui::Context, state: &Arc<RwLock<State>>, at: egui::Pos2) {
    let mut current = state.write().expect("popout state lock");
    let already_open = current.menu.is_some();
    current.menu = Some([at.x, at.y]);
    current.menu_focused = false;
    drop(current);
    if already_open {
        // A new builder position moves it; it also needs the focus back.
        ctx.send_viewport_cmd_to(menu_viewport_id(), egui::ViewportCommand::Focus);
    }
}

fn close_menu(ctx: &egui::Context, state: &Arc<RwLock<State>>) {
    let mut current = state.write().expect("popout state lock");
    current.menu = None;
    current.menu_focused = false;
    drop(current);
    // The pop-out's next pass stops showing the menu viewport, which closes it.
    ctx.request_repaint_of(viewport_id());
}

/// The right-click menu, in its own small native window. The pop-out is too
/// small to hold an in-window popup: it got clipped to the clock's size.
fn render_menu(ui: &mut egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    let ctx = ui.ctx().clone();
    let (focused, close_requested, escape, inner_size) = ui.input(|input| {
        let info = input.viewport();
        (
            info.focused,
            info.close_requested(),
            input.key_pressed(egui::Key::Escape),
            info.inner_rect.map(|rect| rect.size()),
        )
    });

    let (locked, font, color) = {
        let mut current = state.write().expect("popout state lock");
        if current.menu.is_none() {
            return;
        }
        // Close on click-away, but only once it has had focus: a new window
        // is not focused yet on its first pass.
        let clicked_away = focused == Some(false) && current.menu_focused;
        if focused == Some(true) {
            current.menu_focused = true;
        }
        if clicked_away || close_requested || escape {
            drop(current);
            close_menu(&ctx, state);
            return;
        }
        (current.locked, current.font, current.text_color)
    };

    let frame = egui::Frame::menu(ui.style()).corner_radius(0);
    let shown = frame.show(ui, |ui| {
        ui.set_width(MENU_WIDTH - frame.total_margin().sum().x);
        // Measured inside the frame, so buttons never push the menu wider.
        let full_width = egui::vec2(ui.available_width(), 0.0);
        ui.label(egui::RichText::new("Font").weak());
        for option in PopoutFont::ALL {
            let button = egui::Button::selectable(option == font, option.label());
            if ui.add(button.min_size(full_width)).clicked() {
                choose(&ctx, state, store, |current| current.font = option);
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("Color").weak());
        ui.horizontal(|ui| {
            for (name, value) in TEXT_COLORS {
                if color_swatch(ui, value, value == color)
                    .on_hover_text(name)
                    .clicked()
                {
                    choose(&ctx, state, store, |current| current.text_color = value);
                }
            }
        });
        ui.separator();
        let lock_label = if locked { "Unlock" } else { "Lock" };
        if ui
            .add(egui::Button::new(lock_label).min_size(full_width))
            .clicked()
        {
            choose(&ctx, state, store, |current| {
                current.locked = !locked;
                current.dragging = false;
                current.resizing = false;
            });
        }
        if ui
            .add(egui::Button::new("Close pop-out").min_size(full_width))
            .clicked()
        {
            close(&ctx, state, store);
        }
    });

    // Fit the window to the menu once its size is known. The native size
    // snaps to whole pixels, so allow a point of slack to avoid resizing (and
    // repainting) forever.
    let wanted = shown.response.rect.size();
    let off = |size: egui::Vec2| (size.x - wanted.x).abs() > 1.0 || (size.y - wanted.y).abs() > 1.0;
    if inner_size.is_some_and(off) {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(wanted));
    }
}

fn color_swatch(ui: &mut egui::Ui, value: [u8; 4], selected: bool) -> egui::Response {
    let [r, g, b, a] = value;
    let (rect, response) = ui.allocate_exact_size(egui::Vec2::splat(20.0), egui::Sense::click());
    let painter = ui.painter();
    painter.rect_filled(
        rect.shrink(3.0),
        3.0,
        egui::Color32::from_rgba_unmultiplied(r, g, b, a),
    );
    if selected || response.hovered() {
        let stroke = ui.visuals().widgets.active.fg_stroke;
        painter.rect_stroke(rect, 4.0, stroke, egui::StrokeKind::Inside);
    }
    response
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
        keep_behind_apps(ui.ctx());
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
        keep_behind_apps(ui.ctx());
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
        keep_behind_apps(ui.ctx());
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
        keep_behind_apps(ui.ctx());
    }
}

fn render(ui: &mut egui::Ui, state: &Arc<RwLock<State>>, store: &Arc<Store>) {
    if ui.input(|input| input.viewport().close_requested()) {
        close(ui.ctx(), state, store);
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
        keep_behind_apps(ui.ctx());
    }

    egui::CentralPanel::default()
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

    // Right-click anywhere on the pop-out, text included, opens the menu.
    let click = ui.input(|input| {
        if !input.pointer.secondary_clicked() {
            return None;
        }
        let info = input.viewport();
        let origin = info.inner_rect?.min;
        let local = input.pointer.interact_pos()?;
        Some((origin + local.to_vec2(), info.monitor_size))
    });
    if let Some((at, monitor)) = click {
        open_menu(ui.ctx(), state, menu_position(at, MENU_ESTIMATE, monitor));
    }

    let menu = state.read().expect("popout state lock").menu;
    if let Some([x, y]) = menu {
        // A new undecorated window would show Windows' white client area until
        // its first egui frame paints. Make it transparent from creation, and
        // drop the shadow (which can ghost on transparent windows), so the menu
        // appears already dark instead of flashing white.
        let builder = egui::ViewportBuilder::default()
            .with_title("Clock menu")
            .with_decorations(false)
            .with_resizable(false)
            .with_taskbar(false)
            .with_transparent(true)
            .with_has_shadow(false)
            .with_always_on_top()
            .with_active(true)
            .with_position(egui::pos2(x, y))
            .with_inner_size(MENU_ESTIMATE);
        let state = Arc::clone(state);
        let store = Arc::clone(store);
        ui.ctx()
            .show_viewport_deferred(menu_viewport_id(), builder, move |ui, _class| {
                render_menu(ui, &state, &store);
            });
    }

    // Windows only applies AlwaysOnBottom when the level changes, so reassert it
    // after focus changes and whenever another window is opened.
    if !dragging && !resizing && !ui.input(|input| input.pointer.any_down()) {
        keep_behind_apps(ui.ctx());
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

    #[test]
    fn the_menu_opens_at_the_click_and_stays_on_screen() {
        let menu = egui::vec2(190.0, 230.0);
        let monitor = Some(egui::vec2(1920.0, 1080.0));
        // Room to spare: open right at the click.
        assert_eq!(
            menu_position(egui::pos2(300.0, 200.0), menu, monitor),
            egui::pos2(300.0, 200.0)
        );
        // Near the bottom-right corner: flip left and up.
        assert_eq!(
            menu_position(egui::pos2(1850.0, 1000.0), menu, monitor),
            egui::pos2(1660.0, 770.0)
        );
        // Another monitor, or no monitor info: leave it alone.
        assert_eq!(
            menu_position(egui::pos2(2500.0, 1000.0), menu, monitor),
            egui::pos2(2500.0, 1000.0)
        );
        assert_eq!(
            menu_position(egui::pos2(1850.0, 1000.0), menu, None),
            egui::pos2(1850.0, 1000.0)
        );
    }
}
