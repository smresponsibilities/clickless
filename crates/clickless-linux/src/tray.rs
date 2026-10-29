//! Native tray belongs to the runtime, independent of desktop window lifetime.
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

pub enum Action {
    Settings,
    TogglePause,
    Quit,
}
pub struct Tray {
    icon: TrayIcon,
    settings: MenuItem,
    pause: MenuItem,
    quit: MenuItem,
}
impl Tray {
    pub fn new(enabled: bool) -> Result<Self, String> {
        let menu = Menu::new();
        let settings = MenuItem::new("Open Clickless", true, None);
        let pause = MenuItem::new(
            if enabled {
                "Pause pointer control"
            } else {
                "Resume pointer control"
            },
            true,
            None,
        );
        let quit = MenuItem::new("Quit", true, None);
        menu.append_items(&[&settings, &pause, &quit])
            .map_err(|e| e.to_string())?;
        let mut pixels = vec![0; 16 * 16 * 4];
        for y in 2..14 {
            for x in 3..=(3 + (y - 2) / 2) {
                let i = (y * 16 + x) * 4;
                pixels[i..i + 4].copy_from_slice(&[102, 82, 155, 255]);
            }
        }
        let icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Clickless")
            .with_icon(Icon::from_rgba(pixels, 16, 16).map_err(|e| e.to_string())?)
            .build()
            .map_err(|e| e.to_string())?;
        let tray = Self {
            icon,
            settings,
            pause,
            quit,
        };
        tray.refresh(enabled)?;
        Ok(tray)
    }
    pub fn refresh(&self, enabled: bool) -> Result<(), String> {
        self.pause.set_text(if enabled {
            "Pause pointer control"
        } else {
            "Resume pointer control"
        });
        self.icon
            .set_tooltip(Some(if enabled {
                "Clickless: pointer control on"
            } else {
                "Clickless: paused"
            }))
            .map_err(|e| e.to_string())
    }
    pub fn poll(&self) -> Option<Action> {
        let event = MenuEvent::receiver().try_recv().ok()?;
        if event.id == *self.settings.id() {
            Some(Action::Settings)
        } else if event.id == *self.pause.id() {
            Some(Action::TogglePause)
        } else if event.id == *self.quit.id() {
            Some(Action::Quit)
        } else {
            None
        }
    }
}
