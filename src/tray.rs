//! System tray icon: closing the window hides it instead of quitting.
//!
//! The tray events arrive on the crate's own threads, so they use
//! `egui::Context` commands to wake the eframe loop. No polling is needed.

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use eframe::egui;
use tray_icon::TrayIcon;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};

const RESTORE_ID: &str = "clock-app.restore";
const AUTOSTART_ID: &str = "clock-app.autostart";
const QUIT_ID: &str = "clock-app.quit";

/// Shows, un-minimizes and focuses the main window. Safe to call from any
/// thread; also used when a second launch asks for the window.
pub fn show_window(ctx: &egui::Context) {
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    ctx.request_repaint();
}

#[derive(Clone)]
struct Actions {
    ctx: egui::Context,
    quitting: Arc<AtomicBool>,
    autostart_clicked: Arc<AtomicBool>,
}

impl Actions {
    fn restore(&self) {
        self.quitting.store(false, Ordering::SeqCst);
        show_window(&self.ctx);
    }

    fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        self.ctx.request_repaint();
    }

    /// The registry write happens in `ClockApp::logic`, on the UI thread.
    fn toggle_autostart(&self) {
        self.autostart_clicked.store(true, Ordering::SeqCst);
        self.ctx.request_repaint();
    }
}

/// Kept in `ClockApp` so the native tray icon stays alive while the app runs.
pub struct Tray {
    _icon: TrayIcon,
    autostart: CheckMenuItem,
    quitting: Arc<AtomicBool>,
    autostart_clicked: Arc<AtomicBool>,
}

impl Tray {
    /// `autostart` is the current "Start with Windows" state for the menu's
    /// check mark.
    pub fn new(ctx: egui::Context, autostart: bool) -> Result<Self, String> {
        let actions = Actions {
            ctx,
            quitting: Arc::new(AtomicBool::new(false)),
            autostart_clicked: Arc::new(AtomicBool::new(false)),
        };

        MenuEvent::set_event_handler({
            let actions = actions.clone();
            Some(move |event: MenuEvent| {
                if event.id() == RESTORE_ID {
                    actions.restore();
                } else if event.id() == AUTOSTART_ID {
                    actions.toggle_autostart();
                } else if event.id() == QUIT_ID {
                    actions.quit();
                }
            })
        });

        tray_icon::TrayIconEvent::set_event_handler({
            let actions = actions.clone();
            Some(move |event: tray_icon::TrayIconEvent| {
                use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent};
                match event {
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                    | TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } => actions.restore(),
                    _ => {}
                }
            })
        });

        let restore = MenuItem::with_id(RESTORE_ID, "Restore", true, None);
        let autostart_item =
            CheckMenuItem::with_id(AUTOSTART_ID, "Start with Windows", true, autostart, None);
        let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
        let menu = Menu::with_items(&[
            &restore,
            &PredefinedMenuItem::separator(),
            &autostart_item,
            &PredefinedMenuItem::separator(),
            &quit,
        ])
        .map_err(|err| format!("could not build tray menu: {err}"))?;

        let data = crate::icon::app_icon();
        let icon = tray_icon::Icon::from_rgba(data.rgba, data.width, data.height)
            .map_err(|err| format!("could not build tray icon: {err}"))?;

        let tray = tray_icon::TrayIconBuilder::new()
            .with_icon(icon)
            .with_tooltip("Clock")
            .with_menu(Box::new(menu))
            .build()
            .map_err(|err| format!("could not create tray icon: {err}"))?;

        Ok(Self {
            _icon: tray,
            autostart: autostart_item,
            quitting: actions.quitting,
            autostart_clicked: actions.autostart_clicked,
        })
    }

    /// True only when Quit was chosen from the tray menu, so the close button
    /// can still hide the window without ending the process.
    pub fn quit_requested(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }

    /// True once per click on "Start with Windows" in the tray menu.
    pub fn take_autostart_click(&self) -> bool {
        self.autostart_clicked.swap(false, Ordering::SeqCst)
    }

    /// Keeps the menu's check mark in line with the real setting.
    pub fn set_autostart_checked(&self, on: bool) {
        self.autostart.set_checked(on);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::MenuId;

    #[test]
    fn menu_ids_are_distinct() {
        let ids = [RESTORE_ID, AUTOSTART_ID, QUIT_ID].map(MenuId::new);
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[0], ids[2]);
        assert_ne!(ids[1], ids[2]);
    }
}
