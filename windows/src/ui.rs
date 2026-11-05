//! Klippy's windows: each is a WebView2 page (`ui/`) that renders the view
//! model and sends back messages. Copied HTML is shown only inside a fully
//! sandboxed `<iframe>` (no scripts, no network, no navigation), and the
//! page itself allows nothing but its own inline script and styles.

use serde::{Deserialize, Serialize};
use shared::{Appearance, Event, SelectedClip, ViewModel};
use tao::{
    dpi::LogicalSize,
    event_loop::{EventLoopProxy, EventLoopWindowTarget},
    window::{Window, WindowBuilder},
};
use wry::{NewWindowResponse, WebView, WebViewBuilder, WebViewBuilderExtWindows, http::Request};

use crate::{icon, shell::UserEvent};

const TEMPLATE: &str = include_str!("../ui/index.html");
const CSS: &str = include_str!("../ui/app.css");
const JS: &str = include_str!("../ui/app.js");

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowKind {
    Main,
    History,
    Settings,
    ClearHistory,
}

impl WindowKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::History => "history",
            Self::Settings => "settings",
            Self::ClearHistory => "clear_history",
        }
    }

    const fn title(self) -> &'static str {
        match self {
            Self::Main => "Klippy",
            Self::History => "History",
            Self::Settings => "Klippy Settings",
            Self::ClearHistory => "Clear History",
        }
    }

    /// Only History can be resized, like on macOS.
    const fn size(self) -> (f64, f64) {
        match self {
            Self::Main => (300.0, 480.0),
            Self::History => (900.0, 600.0),
            Self::Settings => (560.0, 640.0),
            Self::ClearHistory => (400.0, 230.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tab {
    General,
    About,
}

impl Tab {
    const fn name(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::About => "about",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsPage {
    Notifications,
    StartupApps,
}

impl SettingsPage {
    pub const fn uri(&self) -> &'static str {
        match self {
            Self::Notifications => "ms-settings:notifications",
            Self::StartupApps => "ms-settings:startupapps",
        }
    }
}

/// The only websites a page can open.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Link {
    Website,
    Icons,
    Licence,
}

impl Link {
    pub const fn url(&self) -> &'static str {
        match self {
            Self::Website => "https://xplorebits.com",
            Self::Icons => "https://www.figma.com/community/file/1166831539721848736/solar-icons-set",
            Self::Licence => "https://creativecommons.org/licenses/by/4.0/",
        }
    }
}

/// What a page asks the shell to do.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// The page has loaded and can be rendered.
    Ready,
    Event {
        event: Event,
    },
    Open {
        window: WindowKind,
        #[serde(default)]
        tab: Option<Tab>,
    },
    /// Close the window that sent it.
    Close,
    Quit,
    SetLaunchAtLogin {
        enabled: bool,
    },
    OpenSettingsPage {
        page: SettingsPage,
    },
    OpenLink {
        link: Link,
    },
}

/// Only what the windows' controls send; the core's other events (start,
/// clipboard content…) come from the shell alone.
pub const fn allowed(event: &Event) -> bool {
    matches!(
        event,
        Event::RefreshNotificationPermission
            | Event::Search(_)
            | Event::HistorySearch(_)
            | Event::CopyItem(_)
            | Event::SelectItem(_)
            | Event::SetCopyTrigger(_)
            | Event::ClearHistory
            | Event::SetHistoryTab(_)
            | Event::SelectFailed(_)
            | Event::DismissFailed(_)
            | Event::DismissAllFailed
            | Event::CopyFailedText(_)
            | Event::Export { .. }
            | Event::QuickExport(_)
            | Event::SetQuickExportFormat(_)
            | Event::SetTrayClickAction(_)
            | Event::SetRetention { .. }
    )
}

/// What only the shell knows, sent along with the view model.
#[derive(Serialize)]
pub struct ShellState {
    pub launch_at_login: bool,
    /// Turned off in Startup apps, where only the user can turn it back on.
    pub launch_needs_approval: bool,
    pub launch_error: Option<String>,
}

/// The selected copy as the viewer needs it: the formats' bytes stay behind.
#[derive(Serialize)]
struct Selected<'a> {
    id: u64,
    text: &'a str,
    /// The sanitised display copy, or the original HTML if there isn't one.
    html: Option<String>,
    appearance: Appearance,
}

#[derive(Serialize)]
struct State<'a> {
    view: &'a ViewModel,
    shell: &'a ShellState,
    selected: Option<Selected<'a>>,
    /// The page already shows `selected`; it's left out to keep updates small.
    selected_unchanged: bool,
}

pub struct UiWindow {
    pub kind: WindowKind,
    // Dropped before its window.
    pub webview: WebView,
    pub window: Window,
    pub ready: bool,
    /// The selected copy this page last received.
    shown_selected: Option<Option<u64>>,
}

impl UiWindow {
    pub fn create(
        kind: WindowKind,
        tab: Option<Tab>,
        target: &EventLoopWindowTarget<UserEvent>,
        proxy: EventLoopProxy<UserEvent>,
    ) -> Result<Self, String> {
        let (width, height) = kind.size();
        let resizable = kind == WindowKind::History;
        let mut builder = WindowBuilder::new()
            .with_title(kind.title())
            .with_inner_size(LogicalSize::new(width, height))
            .with_resizable(resizable)
            .with_maximizable(resizable)
            .with_window_icon(icon::window());
        if resizable {
            builder = builder.with_min_inner_size(LogicalSize::new(640.0, 400.0));
        }
        let window = builder.build(target).map_err(|error| error.to_string())?;
        let id = window.id();
        let webview = WebViewBuilder::new()
            .with_html(page(kind, tab))
            .with_ipc_handler(move |request: Request<String>| {
                match serde_json::from_str::<Message>(request.body()) {
                    Ok(message) => {
                        let _ = proxy.send_event(UserEvent::Ui(id, message));
                    }
                    Err(error) => eprintln!("klippy: bad message from page: {error}"),
                }
            })
            .with_navigation_handler(is_initial_load)
            .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
            .with_incognito(true)
            .with_devtools(cfg!(debug_assertions))
            .with_hotkeys_zoom(false)
            .with_default_context_menus(false)
            .with_browser_accelerator_keys(false)
            .build(&window)
            .map_err(|error| error.to_string())?;
        Ok(Self {
            kind,
            webview,
            window,
            ready: false,
            shown_selected: None,
        })
    }

    pub fn render(&mut self, view: &ViewModel, selected: Option<&SelectedClip>, shell: &ShellState) {
        if !self.ready {
            return;
        }
        let selected = selected.filter(|_| self.kind == WindowKind::History);
        let id = selected.map(|clip| clip.id);
        let unchanged = self.shown_selected == Some(id);
        self.shown_selected = Some(id);
        let state = State {
            view,
            shell,
            selected: selected.filter(|_| !unchanged).map(|clip| Selected {
                id: clip.id,
                text: &clip.text,
                html: clip.display_html.clone().or_else(|| {
                    clip.formats
                        .iter()
                        .find(|format| format.kind == "public.html")
                        .map(|format| String::from_utf8_lossy(&format.data).into_owned())
                }),
                appearance: clip.appearance,
            }),
            selected_unchanged: unchanged,
        };
        match serde_json::to_string(&state) {
            Ok(json) => self.eval(&format!("klippy.render({json})")),
            Err(error) => eprintln!("klippy: view model: {error}"),
        }
    }

    pub fn show_tab(&self, tab: Tab) {
        self.eval(&format!("klippy.showTab('{}')", tab.name()));
    }

    pub fn bring_to_front(&self) {
        self.window.set_visible(true);
        self.window.set_minimized(false);
        self.window.set_focus();
    }

    fn eval(&self, script: &str) {
        if let Err(error) = self.webview.evaluate_script(script) {
            eprintln!("klippy: page script: {error}");
        }
    }
}

/// Only the page itself (loaded from a string) may load; links, redirects
/// and anything else go nowhere.
pub fn is_initial_load(url: String) -> bool {
    url == "about:blank" || url.starts_with("data:")
}

fn page(kind: WindowKind, tab: Option<Tab>) -> String {
    let mut nonce = [0u8; 16];
    let _ = getrandom::fill(&mut nonce);
    let nonce: String = nonce.iter().map(|byte| format!("{byte:02x}")).collect();
    TEMPLATE
        .replace("{{NONCE}}", &nonce)
        .replace("{{WINDOW}}", kind.name())
        .replace("{{TAB}}", tab.unwrap_or(Tab::General).name())
        .replace("{{VERSION}}", env!("CARGO_PKG_VERSION"))
        .replace("{{ICON}}", icon::APP_SVG)
        .replace("{{CSS}}", CSS)
        .replace("{{JS}}", JS)
}
