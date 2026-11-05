//! The notification-area ("tray") icon and its menu.

use shared::ViewModel;
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu},
};

use crate::{icon, registry};

/// Menu items don't truncate, so keep long copies to one short line.
const TITLE_LENGTH: usize = 50;

pub enum Action {
    Show,
    ShowHistory,
    ShowFailed,
    ClearHistory,
    About,
    Settings,
    Quit,
    /// A history item: view or copy it, as chosen in Settings.
    Item(u64),
}

impl Action {
    pub fn from_id(id: &MenuId) -> Option<Self> {
        Some(match id.as_ref() {
            "show" => Self::Show,
            "history" => Self::ShowHistory,
            "failed" => Self::ShowFailed,
            "clear" => Self::ClearHistory,
            "about" => Self::About,
            "settings" => Self::Settings,
            "quit" => Self::Quit,
            other => Self::Item(other.strip_prefix("item:")?.parse().ok()?),
        })
    }
}

/// What the menu shows, to rebuild it only when that changes.
#[derive(PartialEq)]
struct Contents {
    recent: Vec<(u64, String)>,
    failed: usize,
}

pub struct Tray {
    icon: TrayIcon,
    light_taskbar: bool,
    shown: Option<Contents>,
}

impl Tray {
    pub fn new() -> Result<Self, tray_icon::Error> {
        let light_taskbar = light_taskbar();
        let mut builder = TrayIconBuilder::new()
            .with_tooltip("Klippy")
            // Left click opens the main window; the menu is on right click.
            .with_menu_on_left_click(false);
        if let Some(icon) = icon::tray(light_taskbar) {
            builder = builder.with_icon(icon);
        }
        Ok(Self {
            icon: builder.build()?,
            light_taskbar,
            shown: None,
        })
    }

    pub fn update(&mut self, view: &ViewModel) {
        let contents = Contents {
            recent: view
                .recent_items
                .iter()
                .map(|item| (item.id, item.preview.clone()))
                .collect(),
            failed: view.failed_items.len(),
        };
        if self.shown.as_ref() == Some(&contents) {
            return;
        }
        self.icon.set_menu(Some(Box::new(menu(&contents))));
        self.shown = Some(contents);
    }

    /// Recolours the icon if the taskbar switched between light and dark.
    pub fn follow_theme(&mut self) {
        let light = light_taskbar();
        if light != self.light_taskbar {
            self.light_taskbar = light;
            let _ = self.icon.set_icon(icon::tray(light));
        }
    }
}

fn menu(contents: &Contents) -> Menu {
    let item = |id: &str, text: &str| MenuItem::with_id(id, text, true, None);
    let separator = PredefinedMenuItem::separator;
    let menu = Menu::new();
    let _ = menu.append(&item("show", "Show Klippy"));
    let history = Submenu::new("History", true);
    if contents.recent.is_empty() {
        let _ = history.append(&MenuItem::new("Nothing copied yet", false, None));
    }
    // The newest items only: the full history lives in the History window.
    for (id, preview) in &contents.recent {
        let _ = history.append(&item(&format!("item:{id}"), &title(preview)));
    }
    let _ = history.append(&separator());
    let _ = history.append(&item("history", "Show All History…"));
    if contents.failed > 0 {
        let _ = history.append(&item("failed", &format!("Failed to Save ({})…", contents.failed)));
    }
    let _ = history.append(&separator());
    let _ = history.append(&item("clear", "Clear History…"));
    let _ = menu.append(&history);
    let _ = menu.append(&separator());
    let _ = menu.append(&item("about", "About Klippy"));
    let _ = menu.append(&item("settings", "Settings…"));
    let _ = menu.append(&separator());
    let _ = menu.append(&item("quit", "Quit Klippy"));
    menu
}

fn title(preview: &str) -> String {
    let title = if preview.chars().count() > TITLE_LENGTH {
        preview.chars().take(TITLE_LENGTH - 1).chain(['…']).collect()
    } else {
        preview.to_owned()
    };
    // A lone & would underline the next letter as a shortcut.
    title.replace('&', "&&")
}

fn light_taskbar() -> bool {
    registry::get_dword(
        r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
        "SystemUsesLightTheme",
    ) == Some(1)
}
