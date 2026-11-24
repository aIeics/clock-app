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
use tray_icon::menu::{Menu, MenuEvent, MenuItem};

const RESTORE_ID: &str = "clock-app.restore";
const QUIT_ID: &str = "clock-app.quit";

#[derive(Clone)]
struct Actions {
    ctx: egui::Context,
    quitting: Arc<AtomicBool>,
}

impl Actions {
    fn restore(&self) {
        self.quitting.store(false, Ordering::SeqCst);
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Visible(true));
        self.ctx
            .send_viewport_cmd(egui::ViewportCommand::Minimized(false));
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        self.ctx.request_repaint();
    }

    fn quit(&self) {
        self.quitting.store(true, Ordering::SeqCst);
        self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        self.ctx.request_repaint();
    }
}

/// Kept in `ClockApp` so the native tray icon stays alive while the app runs.
pub struct Tray {
    _icon: TrayIcon,
    quitting: Arc<AtomicBool>,
}

impl Tray {
    pub fn new(ctx: egui::Context) -> Result<Self, String> {
        let actions = Actions {
            ctx,
            quitting: Arc::new(AtomicBool::new(false)),
        };

        MenuEvent::set_event_handler({
            let actions = actions.clone();
            Some(move |event: MenuEvent| {
                if event.id() == RESTORE_ID {
                    actions.restore();
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
        let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
        let menu = Menu::with_items(&[&restore, &quit])
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
            quitting: actions.quitting,
        })
    }

    /// True only when Quit was chosen from the tray menu, so the close button
    /// can still hide the window without ending the process.
    pub fn quit_requested(&self) -> bool {
        self.quitting.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tray_icon::menu::MenuId;

    #[test]
    fn menu_ids_are_distinct() {
        assert_ne!(MenuId::new(RESTORE_ID), MenuId::new(QUIT_ID));
    }
}
