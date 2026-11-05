//! The event loop: runs the core, carries out the effects it asks for and
//! keeps the tray menu and windows in step with its view model.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crux_core::{Core, Request, capability::Operation};
use shared::{
    ClipboardAccess, ClipboardSnapshot, Effect, Event, ExportFormat, ExportResult, FetchedImages,
    HistoryTab, Klippy, NotificationPermission, Page, SaveExport, SelectedClip, TrayClickAction,
    ViewModel,
};
use tao::{
    dpi::LogicalSize,
    event::{Event as LoopEvent, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget},
    platform::windows::WindowExtWindows,
    window::{Window, WindowBuilder, WindowId},
};
use tray_icon::{MouseButton, MouseButtonState, TrayIconEvent, menu::MenuEvent};
use wry::{NewWindowResponse, PageLoadEvent, WebView, WebViewBuilder};

use crate::{
    clipboard, export, icon, images, instance, login, notify,
    tray::{Action, Tray},
    ui::{self, Message, ShellState, Tab, UiWindow, WindowKind},
};

/// Fast enough to time a triple copy within its 1.5 s window.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How often the core deletes items older than the retention setting.
const EXPIRY_INTERVAL: Duration = Duration::from_secs(15);

type Target = EventLoopWindowTarget<UserEvent>;

/// Work finished on a background thread, continued on the UI thread.
pub type Task = Box<dyn FnOnce(&mut App, &Target) + Send>;

pub enum UserEvent {
    Ui(WindowId, Message),
    Menu(MenuEvent),
    Tray(TrayIconEvent),
    Task(Task),
    /// Klippy was launched again while running.
    ShowMain,
}

pub fn run() {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let show = proxy.clone();
    if !instance::claim(move || {
        let _ = show.send_event(UserEvent::ShowMain);
    }) {
        return;
    }
    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));
    let tray_proxy = proxy.clone();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = tray_proxy.send_event(UserEvent::Tray(event));
    }));

    let data_dir = data_dir();
    let icon_path = data_dir.join("icon.png");
    notify::register(icon::save_png(&icon_path).then_some(icon_path.as_path()));
    login::enable_on_first_launch();

    let mut app = match App::new(&event_loop, proxy, &data_dir) {
        Ok(app) => app,
        Err(error) => {
            rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Error)
                .set_title("Klippy can't start")
                .set_description(error)
                .show();
            return;
        }
    };
    app.open(WindowKind::Main, None, &event_loop);
    event_loop.run(move |event, target, control_flow| app.handle(event, target, control_flow));
}

/// %LOCALAPPDATA%\Klippy: per user (only they can read it) and kept out of
/// roaming profiles, like the macOS key that never leaves the device.
fn data_dir() -> PathBuf {
    let dir = dirs::data_local_dir().unwrap_or_default().join("Klippy");
    if let Err(error) = std::fs::create_dir_all(&dir) {
        eprintln!("klippy: {}: {error}", dir.display());
    }
    dir
}

/// Unix time in milliseconds.
fn now_ms() -> u64 {
    let since_epoch = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX)
}

fn resolve_later<Op>(request: Request<Op>, output: Op::Output) -> Task
where
    Op: Operation + Send,
    Op::Output: Send,
{
    Box::new(move |app, target| app.resolve(request, output, target))
}

/// A PDF export: its page loads in a hidden webview, then prints to `path`.
struct PdfJob {
    // Dropped before its window.
    webview: WebView,
    _window: Window,
    path: PathBuf,
    request: Request<SaveExport>,
    printing: bool,
}

pub struct App {
    core: Core<Klippy>,
    proxy: EventLoopProxy<UserEvent>,
    /// The latest view model, without `selected`, which is kept apart.
    view: ViewModel,
    selected: Option<SelectedClip>,
    shell: ShellState,
    windows: HashMap<WindowKind, UiWindow>,
    tray: Tray,
    /// For the monotonic timestamps of clipboard changes.
    started: Instant,
    last_sequence: u32,
    next_poll: Instant,
    next_expiry: Instant,
    sent_start: bool,
    pdf_jobs: HashMap<u64, PdfJob>,
    next_pdf_job: u64,
}

impl App {
    fn new(target: &Target, proxy: EventLoopProxy<UserEvent>, data_dir: &Path) -> Result<Self, String> {
        let tray = Tray::new().map_err(|error| format!("Couldn't add the tray icon ({error})."))?;
        let core = Core::<Klippy>::new();
        let view = core.view();
        let mut app = Self {
            core,
            proxy,
            view,
            selected: None,
            shell: ShellState {
                launch_at_login: login::is_enabled(),
                launch_needs_approval: login::needs_approval(),
                launch_error: None,
            },
            windows: HashMap::new(),
            tray,
            started: Instant::now(),
            last_sequence: clipboard::sequence_number(),
            next_poll: Instant::now(),
            next_expiry: Instant::now() + EXPIRY_INTERVAL,
            sent_start: false,
            pdf_jobs: HashMap::new(),
            next_pdf_job: 0,
        };
        let store = data_dir.join("history.sqlite");
        app.update(Event::SetStorePath(store.display().to_string()), target);
        Ok(app)
    }

    fn handle(&mut self, event: LoopEvent<UserEvent>, target: &Target, control_flow: &mut ControlFlow) {
        match event {
            LoopEvent::UserEvent(UserEvent::Ui(id, message)) => {
                self.on_message(id, message, target, control_flow);
            }
            LoopEvent::UserEvent(UserEvent::Menu(event)) => {
                if let Some(action) = Action::from_id(event.id()) {
                    self.on_tray_action(action, target, control_flow);
                }
            }
            LoopEvent::UserEvent(
                UserEvent::Tray(TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                })
                | UserEvent::ShowMain,
            ) => self.open(WindowKind::Main, None, target),
            LoopEvent::UserEvent(UserEvent::Task(task)) => task(self, target),
            LoopEvent::WindowEvent {
                window_id,
                event: WindowEvent::CloseRequested,
                ..
            } => self.windows.retain(|_, window| window.window.id() != window_id),
            // Settings may have changed elsewhere while the window was in the background.
            LoopEvent::WindowEvent {
                window_id,
                event: WindowEvent::Focused(true),
                ..
            } if self.kind_of(window_id) == Some(WindowKind::Settings) => self.refresh_settings(target),
            _ => {}
        }

        // Timers are checked on every pass, so a stream of other events can't
        // hold them back.
        let now = Instant::now();
        if now >= self.next_poll {
            self.poll_clipboard(target);
            self.next_poll = now + POLL_INTERVAL;
        }
        if now >= self.next_expiry {
            self.update(Event::DeleteExpired { now_ms: now_ms() }, target);
            self.tray.follow_theme();
            self.next_expiry = now + EXPIRY_INTERVAL;
        }
        if *control_flow != ControlFlow::Exit {
            *control_flow = ControlFlow::WaitUntil(self.next_poll.min(self.next_expiry));
        }
    }

    fn kind_of(&self, id: WindowId) -> Option<WindowKind> {
        self.windows
            .values()
            .find(|window| window.window.id() == id)
            .map(|window| window.kind)
    }

    fn on_message(&mut self, id: WindowId, message: Message, target: &Target, control_flow: &mut ControlFlow) {
        let Some(kind) = self.kind_of(id) else {
            return;
        };
        match message {
            Message::Ready => {
                if let Some(window) = self.windows.get_mut(&kind) {
                    window.ready = true;
                }
                if kind == WindowKind::Settings {
                    self.refresh_settings(target);
                }
                self.render();
                // Start once the splash screen is up.
                if kind == WindowKind::Main && self.view.page == Page::Splash && !self.sent_start {
                    self.sent_start = true;
                    self.update(Event::Start { now_ms: now_ms() }, target);
                }
            }
            Message::Event { event } if ui::allowed(&event) => self.update(event, target),
            Message::Event { event } => eprintln!("klippy: ignored event from a page: {event:?}"),
            Message::Open { window, tab } => self.open(window, tab, target),
            Message::Close => {
                self.windows.remove(&kind);
            }
            Message::Quit => self.quit(control_flow),
            Message::SetLaunchAtLogin { enabled } => {
                self.shell.launch_error = login::set_enabled(enabled).err();
                self.refresh_launch_at_login();
                self.render();
            }
            Message::OpenSettingsPage { page } => notify::open_uri(page.uri()),
            Message::OpenLink { link } => notify::open_uri(link.url()),
        }
    }

    fn on_tray_action(&mut self, action: Action, target: &Target, control_flow: &mut ControlFlow) {
        match action {
            Action::Show => self.open(WindowKind::Main, None, target),
            Action::ShowHistory => self.open(WindowKind::History, None, target),
            Action::ShowFailed => {
                self.update(Event::SetHistoryTab(HistoryTab::Failed), target);
                self.open(WindowKind::History, None, target);
            }
            Action::ClearHistory => self.open(WindowKind::ClearHistory, None, target),
            Action::About => self.open(WindowKind::Settings, Some(Tab::About), target),
            Action::Settings => self.open(WindowKind::Settings, Some(Tab::General), target),
            Action::Quit => self.quit(control_flow),
            Action::Item(id) => match self.view.tray_click_action {
                TrayClickAction::View => {
                    self.update(Event::SetHistoryTab(HistoryTab::Saved), target);
                    self.update(Event::SelectItem(id), target);
                    self.open(WindowKind::History, None, target);
                }
                TrayClickAction::Copy => self.update(Event::CopyItem(id), target),
            },
        }
    }

    /// One instance of each window: opening it again brings it to the front.
    fn open(&mut self, kind: WindowKind, tab: Option<Tab>, target: &Target) {
        if let Some(window) = self.windows.get(&kind) {
            window.bring_to_front();
            if let Some(tab) = tab {
                window.show_tab(tab);
            }
            return;
        }
        match UiWindow::create(kind, tab, target, self.proxy.clone()) {
            Ok(window) => {
                self.windows.insert(kind, window);
            }
            Err(error) => eprintln!("klippy: couldn't open {kind:?}: {error}"),
        }
    }

    /// Failed copies live only in memory, so warn before they're discarded.
    fn quit(&self, control_flow: &mut ControlFlow) {
        let failed = self.view.failed_items.len();
        if failed > 0 {
            let (title, them) = if failed == 1 {
                ("1 copy failed to save".to_owned(), "it")
            } else {
                (format!("{failed} copies failed to save"), "them")
            };
            let answer = rfd::MessageDialog::new()
                .set_level(rfd::MessageLevel::Warning)
                .set_title(title)
                .set_description(format!(
                    "Klippy found malicious content in {them} and didn't save {them}. Failed copies \
                     are kept only while Klippy is running and will be discarded when it quits.\n\n\
                     Quit anyway?"
                ))
                .set_buttons(rfd::MessageButtons::OkCancel)
                .show();
            if answer != rfd::MessageDialogResult::Ok {
                return;
            }
        }
        *control_flow = ControlFlow::Exit;
    }

    fn refresh_settings(&mut self, target: &Target) {
        self.refresh_launch_at_login();
        self.update(Event::RefreshNotificationPermission, target);
        self.render();
    }

    fn refresh_launch_at_login(&mut self) {
        self.shell.launch_at_login = login::is_enabled();
        self.shell.launch_needs_approval = login::needs_approval();
    }

    /// Reports each change of the clipboard's sequence number to the core.
    /// Only the number is read here, so ordinary copies never touch the
    /// clipboard's content.
    fn poll_clipboard(&mut self, target: &Target) {
        let sequence = clipboard::sequence_number();
        if sequence == self.last_sequence {
            return;
        }
        self.last_sequence = sequence;
        // One copy can bump the number several times (once per format an
        // app adds), so a change seen in one poll counts as one copy.
        let at_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.update(Event::ClipboardChanged { count: 1, at_ms }, target);
    }

    fn update(&mut self, event: Event, target: &Target) {
        let effects = self.core.process_event(event);
        self.process(effects, target);
    }

    fn resolve<Op: Operation>(&mut self, mut request: Request<Op>, output: Op::Output, target: &Target) {
        match self.core.resolve(&mut request, output) {
            Ok(effects) => self.process(effects, target),
            Err(error) => eprintln!("klippy: couldn't resolve an effect: {error}"),
        }
    }

    fn process(&mut self, effects: Vec<Effect>, target: &Target) {
        for effect in effects {
            match effect {
                Effect::Render(_) => self.render(),
                // Windows has no permission for reading the clipboard.
                Effect::ClipboardAccess(request) => {
                    self.resolve(request, ClipboardAccess { granted: true }, target);
                }
                Effect::NotificationPermission(request) => {
                    let permission = NotificationPermission {
                        granted: notify::allowed(),
                    };
                    self.resolve(request, permission, target);
                }
                Effect::ClipboardRead(request) => {
                    let snapshot = ClipboardSnapshot {
                        content: clipboard::read(),
                        copied_at_ms: now_ms(),
                    };
                    self.resolve(request, snapshot, target);
                }
                Effect::FetchImages(request) => self.in_background(move || {
                    let images = images::fetch(&request.operation.urls);
                    resolve_later(request, FetchedImages { images })
                }),
                Effect::ClipboardWrite(request) => {
                    if let Err(error) = clipboard::write(&request.operation.content) {
                        eprintln!("klippy: clipboard: {error}");
                    }
                    // Our own write isn't a user copy, so don't count it.
                    self.last_sequence = clipboard::sequence_number();
                }
                Effect::SaveExport(request) => self.save_export(request),
                Effect::Notify(request) => {
                    let note = request.operation;
                    notify::show(&note.title, &note.body, note.reveal_path);
                }
            }
        }
    }

    fn in_background(&self, work: impl FnOnce() -> Task + Send + 'static) {
        let proxy = self.proxy.clone();
        thread::spawn(move || {
            let _ = proxy.send_event(UserEvent::Task(work()));
        });
    }

    fn render(&mut self) {
        let mut view = self.core.view();
        self.selected = view.selected.take();
        self.view = view;
        self.tray.update(&self.view);
        for window in self.windows.values_mut() {
            window.render(&self.view, self.selected.as_ref(), &self.shell);
        }
    }

    /// The window dialogs belong to.
    fn owner(&self) -> isize {
        [WindowKind::Main, WindowKind::History, WindowKind::Settings, WindowKind::ClearHistory]
            .iter()
            .find_map(|kind| self.windows.get(kind))
            .map_or(0, |window| window.window.hwnd())
    }

    fn save_export(&self, request: Request<SaveExport>) {
        let owner = self.owner();
        self.in_background(move || {
            let Some(path) = export::destination(&request.operation, owner) else {
                let cancelled = ExportResult {
                    path: None,
                    error: String::new(),
                };
                return resolve_later(request, cancelled);
            };
            if request.operation.format == ExportFormat::Pdf {
                return Box::new(move |app, target| app.start_pdf(request, path, target));
            }
            let result = match export::write(&path, &request.operation.data) {
                Ok(()) => ExportResult {
                    path: Some(path.display().to_string()),
                    error: String::new(),
                },
                Err(error) => ExportResult { path: None, error },
            };
            resolve_later(request, result)
        });
    }

    /// Loads the export's page offscreen, under the same lockdown as the
    /// viewer (no JavaScript, no network, nothing stored); `print_pdf`
    /// follows once it has loaded.
    fn start_pdf(&mut self, request: Request<SaveExport>, path: PathBuf, target: &Target) {
        let job = self.next_pdf_job;
        self.next_pdf_job += 1;
        let proxy = self.proxy.clone();
        let html = String::from_utf8_lossy(&request.operation.data).into_owned();
        let built = WindowBuilder::new()
            .with_visible(false)
            .with_inner_size(LogicalSize::new(800.0, 1000.0))
            .build(target)
            .map_err(|error| error.to_string())
            .and_then(|window| {
                let webview = WebViewBuilder::new()
                    .with_html(html)
                    .with_javascript_disabled()
                    .with_incognito(true)
                    .with_navigation_handler(ui::is_initial_load)
                    .with_new_window_req_handler(|_, _| NewWindowResponse::Deny)
                    .with_on_page_load_handler(move |event, _| {
                        if matches!(event, PageLoadEvent::Finished) {
                            let print: Task = Box::new(move |app, target| app.print_pdf(job, target));
                            let _ = proxy.send_event(UserEvent::Task(print));
                        }
                    })
                    .build(&window)
                    .map_err(|error| error.to_string())?;
                Ok((window, webview))
            });
        match built {
            Ok((window, webview)) => {
                self.pdf_jobs.insert(
                    job,
                    PdfJob {
                        webview,
                        _window: window,
                        path,
                        request,
                        printing: false,
                    },
                );
            }
            Err(error) => self.resolve(request, ExportResult { path: None, error }, target),
        }
    }

    fn print_pdf(&mut self, job: u64, target: &Target) {
        let Some(pdf) = self.pdf_jobs.get_mut(&job) else {
            return;
        };
        if pdf.printing {
            return;
        }
        pdf.printing = true;
        let proxy = self.proxy.clone();
        let printed = export::print_pdf(&pdf.webview, &pdf.path, move |succeeded| {
            let finish: Task = Box::new(move |app, target| app.finish_pdf(job, succeeded, target));
            let _ = proxy.send_event(UserEvent::Task(finish));
        });
        if let Err(error) = printed {
            eprintln!("klippy: PDF: {error}");
            self.finish_pdf(job, false, target);
        }
    }

    fn finish_pdf(&mut self, job: u64, succeeded: bool, target: &Target) {
        let Some(pdf) = self.pdf_jobs.remove(&job) else {
            return;
        };
        let result = if succeeded {
            ExportResult {
                path: Some(pdf.path.display().to_string()),
                error: String::new(),
            }
        } else {
            ExportResult {
                path: None,
                error: "The PDF couldn't be created.".into(),
            }
        };
        self.resolve(pdf.request, result, target);
    }
}
