//! System tray menu for the Windows desktop runtime.
//!
//! `MenuCommand` and `resolve_menu` are pure and unit tested. The native
//! construction through `tray-icon` needs a live session; it is covered by the
//! bounded native check, not by unit tests.

#[cfg(windows)]
pub mod win {
    use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuEventReceiver, MenuId, MenuItem};
    use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

    /// Commands the tray can send to the runtime.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MenuCommand {
        ShowGrid,
        HideGrid,
        TogglePause,
        OpenSettings,
        OpenPractice,
        Quit,
    }

    /// Which tray item carries which command. Pure, so tests cover it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TrayIds {
        pub show_grid: usize,
        pub hide_grid: usize,
        pub toggle_pause: usize,
        pub settings: usize,
        pub practice: usize,
        pub quit: usize,
    }

    /// Maps synthetic ids to commands. `resolve_menu` and the tray use the
    /// same table, so the menu cannot drift from the handler.
    pub fn command_for(id: usize, ids: TrayIds) -> Option<MenuCommand> {
        if id == ids.show_grid {
            Some(MenuCommand::ShowGrid)
        } else if id == ids.hide_grid {
            Some(MenuCommand::HideGrid)
        } else if id == ids.toggle_pause {
            Some(MenuCommand::TogglePause)
        } else if id == ids.settings {
            Some(MenuCommand::OpenSettings)
        } else if id == ids.practice {
            Some(MenuCommand::OpenPractice)
        } else if id == ids.quit {
            Some(MenuCommand::Quit)
        } else {
            None
        }
    }

    fn tray_icon() -> Result<Icon, String> {
        const SIZE: u32 = 32;
        const RGBA: &[u8] = include_bytes!("../assets/tray-icon-32.rgba");
        Icon::from_rgba(RGBA.to_vec(), SIZE, SIZE).map_err(|e| format!("tray icon rejected: {e}"))
    }

    /// Live tray with its menu. Created on the thread that runs the event loop.
    pub struct TrayMenu {
        pub ids: TrayIds,
        pause_item: CheckMenuItem,
        icon: TrayIcon,
        /// Real muda ids in the same order as `TrayIds`.
        raw_ids: [MenuId; 6],
    }

    impl TrayMenu {
        pub fn new() -> Result<Self, String> {
            let show = MenuItem::new("Show grid", true, None);
            let hide = MenuItem::new("Hide grid", true, None);
            let pause = CheckMenuItem::new("Enabled", true, true, None);
            let settings = MenuItem::new("Settings", true, None);
            let practice = MenuItem::new("Practice", true, None);
            let quit = MenuItem::new("Quit", true, None);

            let ids = TrayIds {
                show_grid: 0,
                hide_grid: 1,
                toggle_pause: 2,
                settings: 3,
                practice: 4,
                quit: 5,
            };

            let menu = Menu::new();
            menu.append_items(&[&show, &hide, &pause, &settings, &practice, &quit])
                .map_err(|e| format!("tray menu build failed: {e}"))?;

            let icon = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Clickless: enabled")
                .with_icon(tray_icon()?)
                .with_menu_on_left_click(true)
                .with_menu_on_right_click(true)
                .build()
                .map_err(|e| format!("tray icon build failed: {e}"))?;

            let raw_ids = [
                show.id().clone(),
                hide.id().clone(),
                pause.id().clone(),
                settings.id().clone(),
                practice.id().clone(),
                quit.id().clone(),
            ];
            Ok(Self {
                ids,
                pause_item: pause,
                icon,
                raw_ids,
            })
        }

        /// Reflects the pause state in the checkbox and the tooltip, so the
        /// state is readable without relying on color alone.
        pub fn set_paused(&mut self, paused: bool) {
            self.pause_item.set_checked(paused);
            let state = if paused { "paused" } else { "enabled" };
            let _ = self.icon.set_tooltip(Some(format!("Clickless: {state}")));
        }

        /// Resolves a menu event id into a command using this tray's ids.
        pub fn resolve(&self, id: &MenuId) -> Option<MenuCommand> {
            let index = self.raw_ids.iter().position(|raw| raw == id)?;
            command_for(index, self.ids)
        }
    }

    /// Global menu-event channel receiver, polled from the main loop.
    pub fn menu_events() -> &'static MenuEventReceiver {
        MenuEvent::receiver()
    }
}

#[cfg(windows)]
pub use win::{MenuCommand, TrayIds, TrayMenu, command_for, menu_events};

#[cfg(test)]
mod tests {
    use super::win::{MenuCommand, TrayIds, command_for};

    fn ids() -> TrayIds {
        TrayIds {
            show_grid: 0,
            hide_grid: 1,
            toggle_pause: 2,
            settings: 3,
            practice: 4,
            quit: 5,
        }
    }

    #[test]
    fn t01_every_tray_slot_maps_to_its_command() {
        let ids = ids();
        assert_eq!(command_for(0, ids), Some(MenuCommand::ShowGrid));
        assert_eq!(command_for(1, ids), Some(MenuCommand::HideGrid));
        assert_eq!(command_for(2, ids), Some(MenuCommand::TogglePause));
        assert_eq!(command_for(3, ids), Some(MenuCommand::OpenSettings));
        assert_eq!(command_for(4, ids), Some(MenuCommand::OpenPractice));
        assert_eq!(command_for(5, ids), Some(MenuCommand::Quit));
    }

    #[test]
    fn t02_unknown_id_resolves_to_nothing() {
        assert_eq!(command_for(6, ids()), None);
        assert_eq!(command_for(999, ids()), None);
    }
}
