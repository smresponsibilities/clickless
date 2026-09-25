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

    /// Check mark means the pointer is paused. The tooltip uses the same
    /// wording, so the two can never disagree about enabled vs paused.
    pub fn pause_checked(paused: bool) -> bool {
        paused
    }

    /// One-line tooltip built from the same state as the check mark, plus
    /// whether the grid is currently open.
    pub fn tray_tooltip(paused: bool, grid_open: bool) -> String {
        let state = if paused { "paused" } else { "enabled" };
        let grid = if grid_open { ", grid open" } else { "" };
        format!("Clickless: {state}{grid}")
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
        paused: bool,
        grid_open: bool,
    }

    impl TrayMenu {
        pub fn new() -> Result<Self, String> {
            let show = MenuItem::new("Show grid", true, None);
            let hide = MenuItem::new("Hide grid", true, None);
            let pause = CheckMenuItem::new("Pause pointer control", true, false, None);
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
                .with_tooltip(tray_tooltip(false, false))
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
                paused: false,
                grid_open: false,
            })
        }

        /// Reflects the live hook state in the checkbox and the tooltip. Both
        /// read from the same values, so checked and tooltip agree. Cheap to
        /// call every loop: unchanged state does no widget work.
        pub fn sync(&mut self, paused: bool, grid_open: bool) {
            if self.paused == paused && self.grid_open == grid_open {
                return;
            }
            self.paused = paused;
            self.grid_open = grid_open;
            self.pause_item.set_checked(pause_checked(paused));
            let _ = self.icon.set_tooltip(Some(tray_tooltip(paused, grid_open)));
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
pub use win::{
    MenuCommand, TrayIds, TrayMenu, command_for, menu_events, pause_checked, tray_tooltip,
};

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

    #[test]
    fn t03_check_mark_and_tooltip_agree() {
        assert!(!super::pause_checked(false));
        assert!(super::pause_checked(true));
        assert_eq!(super::tray_tooltip(false, false), "Clickless: enabled");
        assert_eq!(super::tray_tooltip(true, false), "Clickless: paused");
        assert_eq!(
            super::tray_tooltip(false, true),
            "Clickless: enabled, grid open"
        );
    }
}
