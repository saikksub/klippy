use crux_core::{
    App, Command,
    capability::Operation,
    macros::effect,
    render::{RenderOperation, render},
};
use facet::Facet;
use serde::{Deserialize, Serialize};

use std::collections::HashMap;

use crate::{
    clean, export,
    store::{KEY_BYTES, Store, preview},
};

#[derive(Facet, Serialize, Deserialize, Clone, Debug)]
#[repr(C)]
pub enum Event {
    /// Sent by the shell before `Start`: where the encrypted history lives.
    /// Without it (e.g. in tests) history is kept in memory only.
    SetStorePath(String),
    /// Sent by the shell once the splash screen is up, and again from the
    /// Locked page to retry.
    Start,
    Unlocked(UnlockResult),
    ClipboardAccessChecked(ClipboardAccess),
    /// The user's answer to the notification permission prompt (or the stored
    /// answer on later launches).
    NotificationPermissionAnswered(NotificationPermission),
    /// Re-read the permission, e.g. when Settings opens, in case the user
    /// changed it in System Settings.
    RefreshNotificationPermission,
    Navigate(Page),
    /// The shell saw the system clipboard change `count` times. `at_ms` is a
    /// monotonic timestamp in milliseconds.
    ClipboardChanged {
        count: u32,
        at_ms: u64,
    },
    /// What the clipboard held after a triple copy.
    Copied(ClipboardSnapshot),
    /// The shell downloaded a copy's remote images; finish preprocessing.
    ImagesFetched {
        snapshot: ClipboardSnapshot,
        images: FetchedImages,
    },
    /// The user clicked a history item to copy it again.
    CopyItem(u64),
    /// The user picked an item to view in the History window.
    SelectItem(u64),
    /// The History window's own search, separate from the main window's.
    HistorySearch(String),
    Search(String),
    /// How many quick copies add the clipboard to the history (1 to 4).
    SetCopyTrigger(u32),
    /// Deletes every saved item. The shell asks the user to confirm first.
    ClearHistory,
    /// Which list the History window's sidebar shows.
    SetHistoryTab(HistoryTab),
    SelectFailed(u64),
    DismissFailed(u64),
    DismissAllFailed,
    /// Copies only the plain text of a failed copy, never its HTML.
    CopyFailedText(u64),
    /// Export a saved copy, asking the user where to save it.
    Export {
        id: u64,
        format: ExportFormat,
    },
    /// Export straight to Downloads in the quick-export format.
    QuickExport(u64),
    ExportFinished {
        result: ExportResult,
        quick: bool,
    },
    SetQuickExportFormat(ExportFormat),
    /// Delete items not copied again within this long (1 minute to 365 days).
    SetRetention {
        amount: u32,
        unit: RetentionUnit,
    },
    /// Sent by the shell periodically; `now_ms` is unix time in milliseconds.
    DeleteExpired {
        now_ms: u64,
    },
    Increment,
    Decrement,
    Reset,
}

/// The screen the shell should show. The app always starts on `Splash`.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[repr(C)]
pub enum Page {
    #[default]
    Splash,
    /// Touch ID was cancelled or failed; history stays encrypted.
    Locked,
    /// Shown instead of `Home` when the app can't read the clipboard.
    Setup,
    Home,
}

#[derive(Facet, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub enum ExportFormat {
    #[default]
    Pdf,
    Html,
    Markdown,
}

impl ExportFormat {
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Html => "html",
            Self::Markdown => "md",
        }
    }

    fn from_extension(name: &str) -> Option<Self> {
        [Self::Pdf, Self::Html, Self::Markdown]
            .into_iter()
            .find(|format| format.extension() == name)
    }
}

#[derive(Facet, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub enum HistoryTab {
    #[default]
    Saved,
    /// Copies rejected as malicious this session.
    Failed,
}

/// A copy that wasn't saved because it contained malicious content. Kept in
/// memory only, so the list is gone when the app quits.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FailedCopy {
    pub id: u64,
    pub preview: String,
    /// Only the plain text is kept; the dangerous HTML is discarded.
    pub text: String,
    pub reasons: Vec<String>,
    /// Unix time in milliseconds.
    pub failed_at_ms: u64,
}

/// At most this many failed copies are remembered per session.
const MAX_FAILED: usize = 100;

#[derive(Facet, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
pub enum RetentionUnit {
    Minutes,
    Hours,
    #[default]
    Days,
}

impl RetentionUnit {
    const fn minutes(self) -> u64 {
        match self {
            Self::Minutes => 1,
            Self::Hours => 60,
            Self::Days => 60 * 24,
        }
    }

    /// The largest amount in this unit that stays within 365 days.
    const fn max_amount(self) -> u32 {
        #[allow(clippy::cast_possible_truncation)]
        let max = (MAX_RETENTION_MINUTES / self.minutes()) as u32;
        max
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Minutes => "minutes",
            Self::Hours => "hours",
            Self::Days => "days",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        [Self::Minutes, Self::Hours, Self::Days]
            .into_iter()
            .find(|unit| unit.name() == name)
    }
}

const MAX_RETENTION_MINUTES: u64 = 365 * 24 * 60;
const DEFAULT_RETENTION_DAYS: u32 = 30;

/// Asks the shell to authenticate the user (Touch ID, or their password) and
/// return the history's encryption key from the Keychain.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Unlock;

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UnlockResult {
    /// The 32-byte key, or `None` if authentication failed or was cancelled.
    pub key: Option<Vec<u8>>,
    /// Why there's no key, for the Locked page.
    pub error: String,
    /// Unix time in milliseconds, to clear out expired items on unlock.
    pub now_ms: u64,
}

impl Operation for Unlock {
    type Output = UnlockResult;
}

/// Asks the shell whether the app may read the clipboard without prompting.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardAccessCheck;

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardAccess {
    pub granted: bool,
}

impl Operation for ClipboardAccessCheck {
    type Output = ClipboardAccess;
}

/// One representation of a clipboard item, e.g. HTML or RTF.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardFormat {
    /// The pasteboard type, e.g. `public.html`.
    pub kind: String,
    pub data: Vec<u8>,
}

/// Something copied, with every format the source app provided so styles
/// survive copying it again.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardContent {
    /// The plain-text version, used for display, search and de-duplication.
    pub text: String,
    pub formats: Vec<ClipboardFormat>,
}

/// Asks the shell to request permission to show notifications. macOS only
/// prompts the first time; afterwards it returns the stored answer.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotificationPermissionRequest;

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NotificationPermission {
    pub granted: bool,
}

impl Operation for NotificationPermissionRequest {
    type Output = NotificationPermission;
}

/// Asks the shell for the current clipboard content.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardRead;

/// The result of a `ClipboardRead`.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardSnapshot {
    /// `None` if the clipboard had no text or was marked private.
    pub content: Option<ClipboardContent>,
    /// Unix time in milliseconds when it was read.
    pub copied_at_ms: u64,
}

impl Operation for ClipboardRead {
    type Output = ClipboardSnapshot;
}

/// Asks the shell to download images (https only, each at most
/// `clean::MAX_IMAGE_BYTES`) so the core can clean and inline them.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FetchImages {
    pub urls: Vec<String>,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FetchedImages {
    /// One entry per requested URL, in order; `None` if it failed.
    pub images: Vec<Option<Vec<u8>>>,
}

impl Operation for FetchImages {
    type Output = FetchedImages;
}

/// Asks the shell to save an export. For PDF, `data` is the HTML page the
/// shell renders into pages; for HTML and Markdown it's the file itself.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SaveExport {
    pub file_name: String,
    pub format: ExportFormat,
    pub data: Vec<u8>,
    /// Show a save panel; otherwise save straight to Downloads.
    pub ask_location: bool,
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ExportResult {
    /// Where it was saved, or `None` if cancelled or failed.
    pub path: Option<String>,
    /// Why it failed; empty if it didn't (including when cancelled).
    pub error: String,
}

impl Operation for SaveExport {
    type Output = ExportResult;
}

/// Asks the shell to show a notification (if the user allowed them).
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Notify {
    pub title: String,
    pub body: String,
    /// A file to show selected in Finder when the notification is clicked.
    pub reveal_path: Option<String>,
}

impl Operation for Notify {
    type Output = ();
}

/// Asks the shell to put content on the system clipboard.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipboardWrite {
    pub content: ClipboardContent,
}

impl Operation for ClipboardWrite {
    type Output = ();
}

#[effect(facet_typegen)]
#[derive(Debug)]
pub enum Effect {
    Render(RenderOperation),
    Unlock(Unlock),
    ClipboardAccess(ClipboardAccessCheck),
    NotificationPermission(NotificationPermissionRequest),
    ClipboardRead(ClipboardRead),
    FetchImages(FetchImages),
    ClipboardWrite(ClipboardWrite),
    SaveExport(SaveExport),
    Notify(Notify),
}

/// Largest text or single format kept for one item. Bigger formats (often
/// image renderings apps add alongside text) are dropped; bigger text skips
/// the item, so one huge copy can't exhaust memory.
const MAX_ITEM_BYTES: usize = 10 * 1024 * 1024;

/// How many of the newest items the tray menu lists; menus aren't
/// virtualised, so the rest are reached through "Show All History…".
const TRAY_ITEMS: usize = 20;

const DEFAULT_COPY_TRIGGER: u32 = 3;
const MAX_COPY_TRIGGER: u32 = 4;

/// Copying `copy_trigger` times within this window adds the clipboard to the
/// history: 1s for 1 copy, then 0.5s more per extra copy, up to 2.5s for 4.
fn copy_window_ms(copy_trigger: u32) -> u64 {
    500 + 500 * u64::from(copy_trigger)
}

pub struct Model {
    page: Page,
    store_path: Option<String>,
    /// In memory until unlocked, then the encrypted file.
    store: Store,
    unlocked: bool,
    unlock_error: String,
    clipboard_access: Option<bool>,
    notifications_allowed: Option<bool>,
    /// The lists as last read from the store (ids and previews only).
    items: Vec<ClipItem>,
    history_items: Vec<ClipItem>,
    recent_items: Vec<ClipItem>,
    selected: Option<SelectedClip>,
    /// Newest first; this session only.
    failed: Vec<FailedCopy>,
    next_failed_id: u64,
    selected_failed: Option<u64>,
    history_tab: HistoryTab,
    /// Timestamps of recent clipboard changes, for spotting a triple copy.
    recent_copies: Vec<u64>,
    copy_trigger: u32,
    retention_amount: u32,
    retention_unit: RetentionUnit,
    quick_export_format: ExportFormat,
    search: String,
    history_search: String,
    count: isize,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            page: Page::default(),
            store_path: None,
            store: Store::in_memory(),
            unlocked: false,
            unlock_error: String::new(),
            clipboard_access: None,
            notifications_allowed: None,
            items: Vec::new(),
            history_items: Vec::new(),
            recent_items: Vec::new(),
            selected: None,
            failed: Vec::new(),
            next_failed_id: 0,
            selected_failed: None,
            history_tab: HistoryTab::Saved,
            recent_copies: Vec::new(),
            copy_trigger: DEFAULT_COPY_TRIGGER,
            retention_amount: DEFAULT_RETENTION_DAYS,
            retention_unit: RetentionUnit::Days,
            quick_export_format: ExportFormat::Pdf,
            search: String::new(),
            history_search: String::new(),
            count: 0,
        }
    }
}

impl Model {
    fn retention_ms(&self) -> u64 {
        u64::from(self.retention_amount) * self.retention_unit.minutes() * 60 * 1000
    }

    /// Re-reads the lists (and drops a selection that no longer exists).
    fn refresh(&mut self) {
        self.items = logged(self.store.list(&self.search)).unwrap_or_default();
        self.history_items = logged(self.store.list(&self.history_search)).unwrap_or_default();
        self.recent_items = logged(self.store.recent(TRAY_ITEMS)).unwrap_or_default();
        if let Some(selected) = &self.selected
            && !logged(self.store.contains(selected.id)).unwrap_or(false)
        {
            self.selected = None;
        }
    }

    fn load_settings(&mut self) {
        let setting = |key| logged(self.store.setting(key)).flatten();
        if let Some(trigger) = setting("copy_trigger").and_then(|v| v.parse().ok()) {
            self.copy_trigger = clamp_trigger(trigger);
        }
        if let Some(format) =
            setting("quick_export_format").and_then(|v| ExportFormat::from_extension(&v))
        {
            self.quick_export_format = format;
        }
        if let (Some(amount), Some(unit)) = (
            setting("retention_amount").and_then(|v| v.parse().ok()),
            setting("retention_unit").and_then(|v| RetentionUnit::from_name(&v)),
        ) {
            self.retention_unit = unit;
            self.retention_amount = clamp_retention(amount, unit);
        }
    }

    fn delete_expired(&mut self, now_ms: u64) -> bool {
        let cutoff = now_ms.saturating_sub(self.retention_ms());
        logged(self.store.delete_copied_before(cutoff)).unwrap_or(0) > 0
    }
}

#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ClipItem {
    pub id: u64,
    /// The text on one line, for display.
    pub preview: String,
}

/// The item shown in the History window's viewer.
#[derive(Facet, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SelectedClip {
    pub id: u64,
    pub text: String,
    /// Every format it was copied in, so the viewer can show its styling.
    pub formats: Vec<ClipboardFormat>,
    /// The sanitised HTML with cleaned images inlined, if it had HTML.
    pub display_html: Option<String>,
}

#[derive(Facet, Serialize, Deserialize, Clone, Default)]
pub struct ViewModel {
    pub page: Page,
    /// Whether Touch ID has unlocked the history this launch.
    pub unlocked: bool,
    /// Why the history is locked, on `Page::Locked`.
    pub unlock_error: String,
    /// "true" / "false" once checked, empty before.
    pub clipboard_access: String,
    /// `None` until the permission request has been answered.
    pub notifications_allowed: Option<bool>,
    pub search: String,
    pub copy_trigger: u32,
    /// Explains the current trigger, including its time window.
    pub copy_trigger_hint: String,
    pub retention_amount: u32,
    pub retention_unit: RetentionUnit,
    /// The largest `retention_amount` allowed in `retention_unit`.
    pub retention_max_amount: u32,
    pub retention_hint: String,
    /// What the Home list's export button saves as.
    pub quick_export_format: ExportFormat,
    /// History items matching `search`, newest first.
    pub items: Vec<ClipItem>,
    pub history_search: String,
    /// History items matching `history_search`, for the History window.
    pub history_items: Vec<ClipItem>,
    pub selected: Option<SelectedClip>,
    pub history_tab: HistoryTab,
    /// Copies rejected as malicious this session, newest first.
    pub failed_items: Vec<ClipItem>,
    pub selected_failed: Option<FailedCopy>,
    /// The newest items, unfiltered, for the tray menu.
    pub recent_items: Vec<ClipItem>,
    pub count: String,
}

#[derive(Default)]
pub struct Klippy;

impl App for Klippy {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    #[allow(clippy::too_many_lines)]
    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::SetStorePath(path) => {
                model.store_path = Some(path);
                return Command::done();
            }
            Event::Start => {
                return Command::request_from_shell(Unlock).then_send(Event::Unlocked);
            }
            Event::Unlocked(result) => {
                let key = result
                    .key
                    .and_then(|key| <[u8; KEY_BYTES]>::try_from(key).ok());
                let Some(key) = key else {
                    model.page = Page::Locked;
                    model.unlock_error = result.error;
                    return render();
                };
                if let Some(path) = &model.store_path {
                    match Store::open(path, &key) {
                        Ok(store) => model.store = store,
                        Err(error) => {
                            model.page = Page::Locked;
                            model.unlock_error = format!("Couldn't open the history: {error}");
                            return render();
                        }
                    }
                }
                model.unlocked = true;
                model.unlock_error.clear();
                model.load_settings();
                model.delete_expired(result.now_ms);
                model.refresh();
                return Command::request_from_shell(ClipboardAccessCheck)
                    .then_send(Event::ClipboardAccessChecked);
            }
            Event::ClipboardAccessChecked(access) => {
                model.clipboard_access = Some(access.granted);
                model.page = if access.granted {
                    Page::Home
                } else {
                    Page::Setup
                };
                // Ask once the splash is gone, so the prompt appears over the
                // real app.
                return render().and(
                    Command::request_from_shell(NotificationPermissionRequest)
                        .then_send(Event::NotificationPermissionAnswered),
                );
            }
            Event::RefreshNotificationPermission => {
                return Command::request_from_shell(NotificationPermissionRequest)
                    .then_send(Event::NotificationPermissionAnswered);
            }
            Event::NotificationPermissionAnswered(permission) => {
                model.notifications_allowed = Some(permission.granted);
            }
            Event::Navigate(page) => model.page = page,
            Event::ClipboardChanged { count, at_ms } => {
                model
                    .recent_copies
                    .retain(|&t| at_ms.saturating_sub(t) <= copy_window_ms(model.copy_trigger));
                model
                    .recent_copies
                    .extend(std::iter::repeat_n(at_ms, count as usize));
                if model.recent_copies.len() < model.copy_trigger as usize {
                    return Command::done();
                }
                model.recent_copies.clear();
                return Command::request_from_shell(ClipboardRead).then_send(Event::Copied);
            }
            Event::Copied(snapshot) => {
                let Some(content) = &snapshot.content else {
                    return Command::done();
                };
                if content.text.trim().is_empty() || content.text.len() > MAX_ITEM_BYTES {
                    return Command::done();
                }
                // Reject crafted HTML straight away, before downloading anything.
                let threats = html_of(content)
                    .map(|html| clean::threats(&html))
                    .unwrap_or_default();
                if !threats.is_empty() {
                    let text = content.text.clone();
                    return reject_copy(model, text, threats, snapshot.copied_at_ms);
                }
                // Remote images are downloaded by the shell, then cleaned here.
                let remote = html_of(content)
                    .map(|html| clean::remote_sources(&clean::image_sources(&html)))
                    .unwrap_or_default();
                if !remote.is_empty() {
                    return Command::request_from_shell(FetchImages { urls: remote })
                        .then_send(move |images| Event::ImagesFetched { snapshot, images });
                }
                return save_copy(model, snapshot, &HashMap::new());
            }
            Event::ImagesFetched { snapshot, images } => {
                let remote = snapshot
                    .content
                    .as_ref()
                    .and_then(html_of)
                    .map(|html| clean::remote_sources(&clean::image_sources(&html)))
                    .unwrap_or_default();
                let downloaded = remote
                    .into_iter()
                    .zip(images.images)
                    .filter_map(|(url, image)| Some((url, image?)))
                    .collect();
                return save_copy(model, snapshot, &downloaded);
            }
            Event::CopyItem(id) => {
                let Some(mut content) = logged(model.store.content(id)).flatten() else {
                    return Command::done();
                };
                // Klippy's display copy stays out of the clipboard.
                content
                    .formats
                    .retain(|format| format.kind != clean::DISPLAY_HTML_KIND);
                // The item keeps its place in the list.
                return Command::notify_shell(ClipboardWrite { content }).into();
            }
            Event::SelectItem(id) => {
                let Some(content) = logged(model.store.content(id)).flatten() else {
                    return Command::done();
                };
                let (display, formats): (Vec<_>, Vec<_>) = content
                    .formats
                    .into_iter()
                    .partition(|format| format.kind == clean::DISPLAY_HTML_KIND);
                model.selected = Some(SelectedClip {
                    id,
                    text: content.text,
                    formats,
                    display_html: display
                        .into_iter()
                        .next()
                        .map(|format| String::from_utf8_lossy(&format.data).into_owned()),
                });
            }
            Event::HistorySearch(search) => {
                model.history_items = logged(model.store.list(&search)).unwrap_or_default();
                model.history_search = search;
            }
            Event::Search(search) => {
                model.items = logged(model.store.list(&search)).unwrap_or_default();
                model.search = search;
            }
            Event::SetCopyTrigger(count) => {
                model.copy_trigger = clamp_trigger(count);
                model.recent_copies.clear();
                logged(
                    model
                        .store
                        .set_setting("copy_trigger", &model.copy_trigger.to_string()),
                );
            }
            Event::ClearHistory => {
                logged(model.store.clear());
                model.selected = None;
                model.refresh();
            }
            Event::SetHistoryTab(tab) => model.history_tab = tab,
            Event::SelectFailed(id) => {
                if !model.failed.iter().any(|failed| failed.id == id) {
                    return Command::done();
                }
                model.selected_failed = Some(id);
            }
            Event::DismissFailed(id) => {
                model.failed.retain(|failed| failed.id != id);
                if model.selected_failed == Some(id) {
                    model.selected_failed = None;
                }
            }
            Event::DismissAllFailed => {
                model.failed.clear();
                model.selected_failed = None;
            }
            Event::CopyFailedText(id) => {
                let Some(failed) = model.failed.iter().find(|failed| failed.id == id) else {
                    return Command::done();
                };
                return Command::notify_shell(ClipboardWrite {
                    content: ClipboardContent {
                        text: failed.text.clone(),
                        formats: vec![],
                    },
                })
                .into();
            }
            Event::Export { id, format } => return export(model, id, format, false),
            Event::QuickExport(id) => {
                let format = model.quick_export_format;
                return export(model, id, format, true);
            }
            Event::ExportFinished { result, quick } => {
                let note = match (&result.path, result.error.is_empty()) {
                    (Some(path), _) if quick => Notify {
                        title: "Exported".into(),
                        body: format!(
                            "Saved \u{201c}{}\u{201d} to Downloads.",
                            path.rsplit('/').next().unwrap_or(path)
                        ),
                        reveal_path: Some(path.clone()),
                    },
                    (None, false) => Notify {
                        title: "Export failed".into(),
                        body: result.error,
                        reveal_path: None,
                    },
                    // Saved via the panel (the user saw where), or cancelled.
                    _ => return Command::done(),
                };
                return Command::notify_shell(note).into();
            }
            Event::SetQuickExportFormat(format) => {
                model.quick_export_format = format;
                logged(
                    model
                        .store
                        .set_setting("quick_export_format", format.extension()),
                );
            }
            Event::SetRetention { amount, unit } => {
                model.retention_unit = unit;
                model.retention_amount = clamp_retention(amount, unit);
                logged(
                    model
                        .store
                        .set_setting("retention_amount", &model.retention_amount.to_string()),
                );
                logged(model.store.set_setting("retention_unit", unit.name()));
            }
            Event::DeleteExpired { now_ms } => {
                if !model.delete_expired(now_ms) {
                    return Command::done();
                }
                model.refresh();
            }
            Event::Increment => model.count += 1,
            Event::Decrement => model.count -= 1,
            Event::Reset => model.count = 0,
        }

        render()
    }

    fn view(&self, model: &Model) -> ViewModel {
        ViewModel {
            page: model.page.clone(),
            unlocked: model.unlocked,
            unlock_error: model.unlock_error.clone(),
            notifications_allowed: model.notifications_allowed,
            clipboard_access: model
                .clipboard_access
                .map(|granted| granted.to_string())
                .unwrap_or_default(),
            search: model.search.clone(),
            copy_trigger: model.copy_trigger,
            copy_trigger_hint: copy_trigger_hint(model.copy_trigger),
            retention_amount: model.retention_amount,
            retention_unit: model.retention_unit,
            retention_max_amount: model.retention_unit.max_amount(),
            retention_hint: retention_hint(model.retention_amount, model.retention_unit),
            quick_export_format: model.quick_export_format,
            items: model.items.clone(),
            history_search: model.history_search.clone(),
            history_items: model.history_items.clone(),
            selected: model.selected.clone(),
            history_tab: model.history_tab,
            failed_items: model
                .failed
                .iter()
                .map(|failed| ClipItem {
                    id: failed.id,
                    preview: failed.preview.clone(),
                })
                .collect(),
            selected_failed: model
                .selected_failed
                .and_then(|id| model.failed.iter().find(|failed| failed.id == id))
                .cloned(),
            recent_items: model.recent_items.clone(),
            count: format!("Count is: {}", model.count),
        }
    }
}

/// The copy's HTML, if it has any.
fn html_of(content: &ClipboardContent) -> Option<String> {
    content
        .formats
        .iter()
        .find(|format| format.kind == "public.html")
        .map(|format| String::from_utf8_lossy(&format.data).into_owned())
}

/// The preprocessing stage: drops oversized formats, adds the cleaned
/// display copy of any HTML, then saves; or rejects the copy if its images
/// turn out to be attacks.
fn save_copy(
    model: &mut Model,
    snapshot: ClipboardSnapshot,
    downloaded: &HashMap<String, Vec<u8>>,
) -> Command<Effect, Event> {
    let Some(mut content) = snapshot.content else {
        return Command::done();
    };
    content.formats.retain(|format| {
        format.data.len() <= MAX_ITEM_BYTES && format.kind != clean::DISPLAY_HTML_KIND
    });
    if let Some(html) = html_of(&content) {
        let display = clean::display_html(&html, downloaded);
        if !display.threats.is_empty() {
            return reject_copy(model, content.text, display.threats, snapshot.copied_at_ms);
        }
        if display.html.len() <= MAX_ITEM_BYTES {
            content.formats.push(ClipboardFormat {
                kind: clean::DISPLAY_HTML_KIND.into(),
                data: display.html.into_bytes(),
            });
        }
    }
    logged(model.store.add(&content, snapshot.copied_at_ms));
    model.refresh();
    render()
}

/// Builds the export from the saved copy's sanitised HTML (or text) and asks
/// the shell to save it.
fn export(model: &Model, id: u64, format: ExportFormat, quick: bool) -> Command<Effect, Event> {
    let Some(content) = logged(model.store.content(id)).flatten() else {
        return Command::done();
    };
    let display = content
        .formats
        .iter()
        .find(|format| format.kind == clean::DISPLAY_HTML_KIND)
        .map(|format| String::from_utf8_lossy(&format.data).into_owned());
    let title = preview(&content.text);
    let data = match format {
        ExportFormat::Pdf | ExportFormat::Html => {
            export::html_document(&title, display.as_deref(), &content.text)
        }
        ExportFormat::Markdown => export::markdown(display.as_deref(), &content.text),
    };
    Command::request_from_shell(SaveExport {
        file_name: export::file_name(&title, format),
        format,
        data: data.into_bytes(),
        ask_location: !quick,
    })
    .then_send(move |result| Event::ExportFinished { result, quick })
}

/// Keeps a malicious copy's plain text in the session's Failed list instead
/// of saving it, and tells the user.
fn reject_copy(
    model: &mut Model,
    text: String,
    reasons: Vec<String>,
    failed_at_ms: u64,
) -> Command<Effect, Event> {
    model.next_failed_id += 1;
    model.failed.insert(
        0,
        FailedCopy {
            id: model.next_failed_id,
            preview: preview(&text),
            text,
            reasons,
            failed_at_ms,
        },
    );
    model.failed.truncate(MAX_FAILED);
    let notify: Command<Effect, Event> = Command::notify_shell(Notify {
        title: "Can't save copy".into(),
        body: "Klippy found malicious content and didn't save it. See History → Failed.".into(),
        reveal_path: None,
    })
    .into();
    render().and(notify)
}

/// Database errors are logged and the app carries on with what it has.
fn logged<T>(result: rusqlite::Result<T>) -> Option<T> {
    result
        .inspect_err(|error| eprintln!("klippy: history store: {error}"))
        .ok()
}

fn clamp_trigger(count: u32) -> u32 {
    count.clamp(1, MAX_COPY_TRIGGER)
}

fn clamp_retention(amount: u32, unit: RetentionUnit) -> u32 {
    amount.clamp(1, unit.max_amount())
}

fn copy_trigger_hint(copy_trigger: u32) -> String {
    if copy_trigger == 1 {
        return "Every copy is added to the history.".into();
    }
    #[allow(clippy::cast_precision_loss)]
    let seconds = copy_window_ms(copy_trigger) as f64 / 1000.0;
    format!("Copy {copy_trigger} times within {seconds} seconds to add it to the history.")
}

fn retention_hint(amount: u32, unit: RetentionUnit) -> String {
    let unit = if amount == 1 {
        unit.name().trim_end_matches('s')
    } else {
        unit.name()
    };
    format!("Items not copied again within {amount} {unit} are deleted.")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn starts_on_splash() {
        let model = Model::default();
        assert_eq!(Klippy.view(&model).page, Page::Splash);
    }

    const KEY: [u8; KEY_BYTES] = [9; KEY_BYTES];

    /// Runs `Start` through a successful Touch ID unlock, returning the
    /// command that then asks for clipboard access.
    fn start_and_unlock(model: &mut Model, now_ms: u64) -> Command<Effect, Event> {
        let mut cmd = Klippy.update(Event::Start, model);
        let mut request = cmd.expect_one_effect().expect_unlock();
        request
            .resolve(UnlockResult {
                key: Some(KEY.to_vec()),
                error: String::new(),
                now_ms,
            })
            .unwrap();
        let event = cmd.events().next().unwrap();
        Klippy.update(event, model)
    }

    #[test]
    fn start_asks_for_touch_id_and_stays_locked_without_it() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(Event::Start, &mut model);
        let mut request = cmd.expect_one_effect().expect_unlock();
        request
            .resolve(UnlockResult {
                key: None,
                error: "Cancelled".into(),
                now_ms: 0,
            })
            .unwrap();
        let event = cmd.events().next().unwrap();
        Klippy.update(event, &mut model).expect_only_render();

        let view = Klippy.view(&model);
        assert_eq!(view.page, Page::Locked);
        assert_eq!(view.unlock_error, "Cancelled");
    }

    #[test]
    fn unlocking_opens_the_encrypted_file_with_saved_history_and_settings() {
        let path = std::env::temp_dir().join(format!("klippy-app-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let launch = || {
            let mut model = Model::default();
            let _ = Klippy.update(
                Event::SetStorePath(path.to_string_lossy().into()),
                &mut model,
            );
            let _ = start_and_unlock(&mut model, 1_000);
            model
        };

        let mut model = launch();
        let _ = Klippy.update(copied("remember me"), &mut model);
        let _ = Klippy.update(Event::SetCopyTrigger(2), &mut model);
        let _ = Klippy.update(
            Event::SetRetention {
                amount: 6,
                unit: RetentionUnit::Hours,
            },
            &mut model,
        );
        drop(model);

        let model = launch();
        let view = Klippy.view(&model);
        assert_eq!(items(&model), ["remember me"]);
        assert_eq!(view.copy_trigger, 2);
        assert_eq!(
            (view.retention_amount, view.retention_unit),
            (6, RetentionUnit::Hours)
        );
        drop(model);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_wrong_key_keeps_the_history_locked() {
        let path =
            std::env::temp_dir().join(format!("klippy-wrongkey-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path, &[1; KEY_BYTES]).unwrap());

        let mut model = Model::default();
        let _ = Klippy.update(
            Event::SetStorePath(path.to_string_lossy().into()),
            &mut model,
        );
        let _ = start_and_unlock(&mut model, 0);
        let view = Klippy.view(&model);
        assert_eq!(view.page, Page::Locked);
        assert!(view.unlock_error.starts_with("Couldn't open the history"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn the_tray_gets_the_newest_items_only_once_unlocked() {
        let mut model = Model::default();
        assert!(!Klippy.view(&model).unlocked);
        let _ = start_and_unlock(&mut model, 0);
        assert!(Klippy.view(&model).unlocked);

        for i in 0..TRAY_ITEMS + 5 {
            let _ = Klippy.update(copied(&format!("item {i}")), &mut model);
        }
        let _ = Klippy.update(Event::Search("item 1".into()), &mut model);
        let recent = Klippy.view(&model).recent_items;
        assert_eq!(recent.len(), TRAY_ITEMS);
        assert_eq!(recent[0].preview, format!("item {}", TRAY_ITEMS + 4));
    }

    #[test]
    fn retention_defaults_to_30_days_and_is_limited_to_365_days() {
        let mut model = Model::default();
        let view = Klippy.view(&model);
        assert_eq!(
            (view.retention_amount, view.retention_unit),
            (30, RetentionUnit::Days)
        );
        assert_eq!(
            view.retention_hint,
            "Items not copied again within 30 days are deleted."
        );

        for (unit, max) in [
            (RetentionUnit::Minutes, 525_600),
            (RetentionUnit::Hours, 8_760),
            (RetentionUnit::Days, 365),
        ] {
            let _ = Klippy.update(
                Event::SetRetention {
                    amount: u32::MAX,
                    unit,
                },
                &mut model,
            );
            assert_eq!(Klippy.view(&model).retention_amount, max);
            assert_eq!(Klippy.view(&model).retention_max_amount, max);
        }
        let _ = Klippy.update(
            Event::SetRetention {
                amount: 0,
                unit: RetentionUnit::Minutes,
            },
            &mut model,
        );
        let view = Klippy.view(&model);
        assert_eq!(view.retention_amount, 1);
        assert_eq!(
            view.retention_hint,
            "Items not copied again within 1 minute are deleted."
        );
    }

    #[test]
    fn expired_items_are_deleted() {
        let mut model = Model::default();
        let _ = Klippy.update(
            Event::SetRetention {
                amount: 1,
                unit: RetentionUnit::Minutes,
            },
            &mut model,
        );
        for (text, at) in [("old", 0), ("new", 50_000)] {
            let _ = Klippy.update(
                Event::Copied(ClipboardSnapshot {
                    content: Some(ClipboardContent {
                        text: text.into(),
                        formats: vec![],
                    }),
                    copied_at_ms: at,
                }),
                &mut model,
            );
        }
        Klippy
            .update(Event::DeleteExpired { now_ms: 59_000 }, &mut model)
            .expect_done();
        Klippy
            .update(Event::DeleteExpired { now_ms: 61_000 }, &mut model)
            .expect_only_render();
        assert_eq!(items(&model), ["new"]);
    }

    #[test]
    fn start_checks_clipboard_access_then_shows_home() {
        let mut model = Model::default();
        let mut cmd = start_and_unlock(&mut model, 0);
        let mut request = cmd.expect_one_effect().expect_clipboard_access();
        request.resolve(ClipboardAccess { granted: true }).unwrap();

        let event = cmd.events().next().unwrap();
        let _ = Klippy.update(event, &mut model);
        let view = Klippy.view(&model);
        assert_eq!(view.page, Page::Home);
        assert_eq!(view.clipboard_access, "true");
    }

    fn copied(text: &str) -> Event {
        Event::Copied(ClipboardSnapshot {
            content: Some(ClipboardContent {
                text: text.into(),
                formats: vec![],
            }),
            copied_at_ms: 0,
        })
    }

    fn items(model: &Model) -> Vec<String> {
        Klippy
            .view(model)
            .items
            .into_iter()
            .map(|item| item.preview)
            .collect()
    }

    #[test]
    fn copied_text_goes_to_the_top_without_duplicates() {
        let mut model = Model::default();
        for text in ["one", "two", "one"] {
            Klippy.update(copied(text), &mut model).expect_only_render();
        }
        assert_eq!(items(&model), ["one", "two"]);
    }

    #[test]
    fn drops_oversized_formats_but_keeps_the_text() {
        let mut model = Model::default();
        let content = ClipboardContent {
            text: "hi".into(),
            formats: vec![
                ClipboardFormat {
                    kind: "public.html".into(),
                    data: b"<b>hi</b>".to_vec(),
                },
                ClipboardFormat {
                    kind: "public.tiff".into(),
                    data: vec![0; MAX_ITEM_BYTES + 1],
                },
            ],
        };
        let _ = Klippy.update(
            Event::Copied(ClipboardSnapshot {
                content: Some(content),
                copied_at_ms: 0,
            }),
            &mut model,
        );
        let id = Klippy.view(&model).items[0].id;
        let _ = Klippy.update(Event::SelectItem(id), &mut model);
        let kinds: Vec<String> = Klippy
            .view(&model)
            .selected
            .unwrap()
            .formats
            .into_iter()
            .map(|format| format.kind)
            .collect();
        assert_eq!(kinds, ["public.html"]);
    }

    #[test]
    fn skips_oversized_text() {
        let mut model = Model::default();
        let _ = Klippy.update(copied(&"x".repeat(MAX_ITEM_BYTES + 1)), &mut model);
        assert!(items(&model).is_empty());
    }

    #[test]
    fn ignores_blank_copies() {
        let mut model = Model::default();
        let _ = Klippy.update(copied(" \n"), &mut model);
        assert!(items(&model).is_empty());
    }

    #[test]
    fn previews_are_one_line() {
        let mut model = Model::default();
        let _ = Klippy.update(copied("fn main() {\n    hi\n}"), &mut model);
        assert_eq!(items(&model), ["fn main() { hi }"]);
    }

    #[test]
    fn search_filters_case_insensitively() {
        let mut model = Model::default();
        for text in ["Hello world", "goodbye"] {
            let _ = Klippy.update(copied(text), &mut model);
        }
        let _ = Klippy.update(Event::Search("HELLO".into()), &mut model);
        assert_eq!(items(&model), ["Hello world"]);
    }

    #[test]
    fn clicking_an_item_writes_all_its_formats_to_the_clipboard() {
        let mut model = Model::default();
        let content = ClipboardContent {
            text: "a\nb".into(),
            formats: vec![ClipboardFormat {
                kind: "public.html".into(),
                data: b"<b>a</b><br>b".to_vec(),
            }],
        };
        let _ = Klippy.update(
            Event::Copied(ClipboardSnapshot {
                content: Some(content.clone()),
                copied_at_ms: 0,
            }),
            &mut model,
        );
        let id = Klippy.view(&model).items[0].id;

        let mut cmd = Klippy.update(Event::CopyItem(id), &mut model);
        let request = cmd.expect_one_effect().expect_clipboard_write();
        assert_eq!(request.operation.content, content);
    }

    #[test]
    fn clicking_an_item_keeps_its_place() {
        let mut model = Model::default();
        for text in ["old", "new"] {
            let _ = Klippy.update(copied(text), &mut model);
        }
        let old_id = Klippy.view(&model).items[1].id;
        let _ = Klippy.update(Event::CopyItem(old_id), &mut model);
        assert_eq!(items(&model), ["new", "old"]);
    }

    fn changed(count: u32, at_ms: u64) -> Event {
        Event::ClipboardChanged { count, at_ms }
    }

    #[test]
    fn three_copies_within_the_window_read_the_clipboard() {
        let mut model = Model::default();
        Klippy.update(changed(1, 0), &mut model).expect_done();
        Klippy.update(changed(1, 700), &mut model).expect_done();
        let mut cmd = Klippy.update(changed(1, 1400), &mut model);
        cmd.expect_one_effect().expect_clipboard_read();
    }

    #[test]
    fn copies_counted_in_one_poll_add_up() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(changed(3, 0), &mut model);
        cmd.expect_one_effect().expect_clipboard_read();
    }

    #[test]
    fn the_copy_trigger_is_configurable() {
        let mut model = Model::default();
        let _ = Klippy.update(Event::SetCopyTrigger(1), &mut model);
        assert_eq!(Klippy.view(&model).copy_trigger, 1);
        let mut cmd = Klippy.update(changed(1, 0), &mut model);
        cmd.expect_one_effect().expect_clipboard_read();

        let _ = Klippy.update(Event::SetCopyTrigger(4), &mut model);
        Klippy.update(changed(3, 0), &mut model).expect_done();
        let mut cmd = Klippy.update(changed(1, 100), &mut model);
        cmd.expect_one_effect().expect_clipboard_read();
    }

    #[test]
    fn the_copy_trigger_is_limited_to_1_to_4() {
        let mut model = Model::default();
        let _ = Klippy.update(Event::SetCopyTrigger(0), &mut model);
        assert_eq!(Klippy.view(&model).copy_trigger, 1);
        let _ = Klippy.update(Event::SetCopyTrigger(50), &mut model);
        assert_eq!(Klippy.view(&model).copy_trigger, 4);
    }

    #[test]
    fn each_trigger_count_has_its_own_time_window() {
        for (count, window_ms) in [(2, 1500), (3, 2000), (4, 2500)] {
            let mut model = Model::default();
            let _ = Klippy.update(Event::SetCopyTrigger(count), &mut model);
            let step = window_ms / u64::from(count - 1);

            // Spread exactly across the window: added.
            for i in 0..count - 1 {
                Klippy
                    .update(changed(1, u64::from(i) * step), &mut model)
                    .expect_done();
            }
            let mut cmd = Klippy.update(changed(1, window_ms), &mut model);
            cmd.expect_one_effect().expect_clipboard_read();

            // Just too slow: not added.
            let start = 10_000;
            for i in 0..count - 1 {
                Klippy
                    .update(changed(1, start + u64::from(i) * step), &mut model)
                    .expect_done();
            }
            Klippy
                .update(changed(1, start + window_ms + 1), &mut model)
                .expect_done();
        }
    }

    #[test]
    fn the_hint_explains_the_time_window() {
        let mut model = Model::default();
        assert_eq!(
            Klippy.view(&model).copy_trigger_hint,
            "Copy 3 times within 2 seconds to add it to the history."
        );
        let _ = Klippy.update(Event::SetCopyTrigger(4), &mut model);
        assert_eq!(
            Klippy.view(&model).copy_trigger_hint,
            "Copy 4 times within 2.5 seconds to add it to the history."
        );
        let _ = Klippy.update(Event::SetCopyTrigger(1), &mut model);
        assert_eq!(
            Klippy.view(&model).copy_trigger_hint,
            "Every copy is added to the history."
        );
    }

    #[test]
    fn slow_copies_are_not_added() {
        let mut model = Model::default();
        Klippy.update(changed(1, 0), &mut model).expect_done();
        Klippy.update(changed(1, 1100), &mut model).expect_done();
        Klippy.update(changed(1, 2200), &mut model).expect_done();
    }

    #[test]
    fn a_triple_copy_starts_counting_again() {
        let mut model = Model::default();
        let _ = Klippy.update(changed(3, 0), &mut model);
        Klippy.update(changed(1, 100), &mut model).expect_done();
    }

    #[test]
    fn a_triple_copy_without_text_adds_nothing() {
        let mut model = Model::default();
        let _ = Klippy.update(
            Event::Copied(ClipboardSnapshot {
                content: None,
                copied_at_ms: 0,
            }),
            &mut model,
        );
        assert!(items(&model).is_empty());
    }

    #[test]
    fn start_shows_setup_when_clipboard_access_is_denied() {
        let mut model = Model::default();
        let mut cmd = start_and_unlock(&mut model, 0);
        let mut request = cmd.expect_one_effect().expect_clipboard_access();
        request.resolve(ClipboardAccess { granted: false }).unwrap();

        let event = cmd.events().next().unwrap();
        let _ = Klippy.update(event, &mut model);
        assert_eq!(Klippy.view(&model).page, Page::Setup);
    }

    #[test]
    fn asks_for_notification_permission_after_the_splash() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(
            Event::ClipboardAccessChecked(ClipboardAccess { granted: true }),
            &mut model,
        );
        let mut effects: Vec<Effect> = cmd.effects().collect();
        assert_eq!(effects.len(), 2);
        assert!(effects[0].is_render());
        let mut request = effects.remove(1).expect_notification_permission();
        assert_eq!(Klippy.view(&model).notifications_allowed, None);

        request
            .resolve(NotificationPermission { granted: true })
            .unwrap();
        let event = cmd.events().next().unwrap();
        Klippy.update(event, &mut model).expect_only_render();
        assert_eq!(Klippy.view(&model).notifications_allowed, Some(true));
    }

    #[test]
    fn selecting_an_item_shows_it_with_its_formats() {
        let mut model = Model::default();
        let content = ClipboardContent {
            text: "bold".into(),
            formats: vec![ClipboardFormat {
                kind: "public.html".into(),
                data: b"<b>bold</b>".to_vec(),
            }],
        };
        let _ = Klippy.update(
            Event::Copied(ClipboardSnapshot {
                content: Some(content.clone()),
                copied_at_ms: 0,
            }),
            &mut model,
        );
        let id = Klippy.view(&model).items[0].id;

        Klippy
            .update(Event::SelectItem(id), &mut model)
            .expect_only_render();
        assert_eq!(
            Klippy.view(&model).selected,
            Some(SelectedClip {
                id,
                text: content.text,
                formats: content.formats,
                display_html: Some("<b>bold</b>".into()),
            })
        );
    }

    fn png() -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            2,
            2,
            image::Rgba([0, 0, 255, 200]),
        ))
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
        out.into_inner()
    }

    #[test]
    fn copies_with_remote_images_are_downloaded_cleaned_and_saved() {
        let mut model = Model::default();
        let html = r#"<p style="color: red; background-image: url(https://t.example/p)">Hi</p><img src="https://img.example/a.png">"#;
        let original = ClipboardContent {
            text: "Hi".into(),
            formats: vec![ClipboardFormat {
                kind: "public.html".into(),
                data: html.as_bytes().to_vec(),
            }],
        };
        let mut cmd = Klippy.update(
            Event::Copied(ClipboardSnapshot {
                content: Some(original.clone()),
                copied_at_ms: 0,
            }),
            &mut model,
        );
        assert!(items(&model).is_empty(), "nothing saved before cleaning");

        let mut request = cmd.expect_one_effect().expect_fetch_images();
        assert_eq!(request.operation.urls, ["https://img.example/a.png"]);
        request
            .resolve(FetchedImages {
                images: vec![Some(png())],
            })
            .unwrap();
        let event = cmd.events().next().unwrap();
        Klippy.update(event, &mut model).expect_only_render();

        let id = Klippy.view(&model).items[0].id;
        let _ = Klippy.update(Event::SelectItem(id), &mut model);
        let selected = Klippy.view(&model).selected.unwrap();
        let display = selected.display_html.unwrap();
        assert!(display.contains("data:image/png;base64,"), "{display}");
        for banned in ["img.example", "t.example"] {
            assert!(!display.contains(banned), "{banned} survived: {display}");
        }
        assert_eq!(
            selected.formats, original.formats,
            "originals kept for pasting"
        );

        let mut cmd = Klippy.update(Event::CopyItem(id), &mut model);
        let write = cmd.expect_one_effect().expect_clipboard_write();
        assert_eq!(
            write.operation.content, original,
            "display copy never pasted"
        );
    }

    fn html_copy(html: &str) -> Event {
        Event::Copied(ClipboardSnapshot {
            content: Some(ClipboardContent {
                text: "plain words".into(),
                formats: vec![ClipboardFormat {
                    kind: "public.html".into(),
                    data: html.as_bytes().to_vec(),
                }],
            }),
            copied_at_ms: 7,
        })
    }

    #[test]
    fn malicious_copies_are_not_saved_but_listed_and_notified() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(
            html_copy(r#"<p onclick="steal()">hi</p><script>bad()</script>"#),
            &mut model,
        );
        let effects: Vec<Effect> = cmd.effects().collect();
        assert!(effects.iter().any(Effect::is_render));
        let notify = effects
            .into_iter()
            .find(Effect::is_notify)
            .unwrap()
            .expect_notify();
        assert_eq!(notify.operation.title, "Can't save copy");

        assert!(items(&model).is_empty(), "not saved");
        let view = Klippy.view(&model);
        assert_eq!(view.failed_items.len(), 1);
        let id = view.failed_items[0].id;
        let _ = Klippy.update(Event::SelectFailed(id), &mut model);
        let failed = Klippy.view(&model).selected_failed.unwrap();
        assert_eq!(failed.text, "plain words");
        assert_eq!(failed.failed_at_ms, 7);
        assert_eq!(
            failed.reasons,
            ["Runs code on an event (onclick)", "Contains a script"]
        );
    }

    #[test]
    fn malicious_images_reject_the_copy_after_download() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(
            html_copy(r#"<p>hi</p><img src="https://img.example/a.svg">"#),
            &mut model,
        );
        let mut request = cmd.expect_one_effect().expect_fetch_images();
        request
            .resolve(FetchedImages {
                images: vec![Some(b"<svg><script>alert(1)</script></svg>".to_vec())],
            })
            .unwrap();
        let event = cmd.events().next().unwrap();
        let mut cmd = Klippy.update(event, &mut model);
        assert!(cmd.effects().any(|effect| effect.is_notify()));
        assert!(items(&model).is_empty());
        assert_eq!(Klippy.view(&model).failed_items.len(), 1);
    }

    #[test]
    fn failed_copies_can_be_copied_as_text_or_dismissed() {
        let mut model = Model::default();
        for _ in 0..2 {
            let _ = Klippy.update(html_copy("<script>x</script>"), &mut model);
        }
        let ids: Vec<u64> = Klippy
            .view(&model)
            .failed_items
            .iter()
            .map(|item| item.id)
            .collect();

        let mut cmd = Klippy.update(Event::CopyFailedText(ids[0]), &mut model);
        let write = cmd.expect_one_effect().expect_clipboard_write();
        assert_eq!(
            write.operation.content,
            ClipboardContent {
                text: "plain words".into(),
                formats: vec![]
            },
            "text only, never the HTML"
        );

        let _ = Klippy.update(Event::SelectFailed(ids[0]), &mut model);
        let _ = Klippy.update(Event::DismissFailed(ids[0]), &mut model);
        let view = Klippy.view(&model);
        assert_eq!(view.failed_items.len(), 1);
        assert_eq!(view.selected_failed, None);

        let _ = Klippy.update(Event::DismissAllFailed, &mut model);
        assert!(Klippy.view(&model).failed_items.is_empty());
    }

    #[test]
    fn export_asks_where_to_save_a_sanitised_document() {
        let mut model = Model::default();
        let _ = Klippy.update(
            html_copy(r#"<h1>Report</h1><p style="color: red">body</p>"#),
            &mut model,
        );
        let id = Klippy.view(&model).items[0].id;

        let mut cmd = Klippy.update(
            Event::Export {
                id,
                format: ExportFormat::Markdown,
            },
            &mut model,
        );
        let request = cmd.expect_one_effect().expect_save_export();
        let save = &request.operation;
        assert!(save.ask_location);
        assert_eq!(save.format, ExportFormat::Markdown);
        assert_eq!(save.file_name, "Klippy – plain words.md");
        let markdown = String::from_utf8(save.data.clone()).unwrap();
        assert!(markdown.contains("# Report"), "{markdown}");
    }

    #[test]
    fn quick_export_saves_to_downloads_in_the_chosen_format_and_notifies() {
        let mut model = Model::default();
        let _ = Klippy.update(copied("note"), &mut model);
        let id = Klippy.view(&model).items[0].id;
        assert_eq!(Klippy.view(&model).quick_export_format, ExportFormat::Pdf);

        let _ = Klippy.update(Event::SetQuickExportFormat(ExportFormat::Html), &mut model);
        let mut cmd = Klippy.update(Event::QuickExport(id), &mut model);
        let mut request = cmd.expect_one_effect().expect_save_export();
        assert!(!request.operation.ask_location);
        assert_eq!(request.operation.format, ExportFormat::Html);
        assert!(String::from_utf8_lossy(&request.operation.data).contains("<pre>note</pre>"));

        request
            .resolve(ExportResult {
                path: Some("/Users/me/Downloads/Klippy – note.html".into()),
                error: String::new(),
            })
            .unwrap();
        let event = cmd.events().next().unwrap();
        let mut cmd = Klippy.update(event, &mut model);
        let notify = cmd.expect_one_effect().expect_notify();
        assert_eq!(
            notify.operation.body,
            "Saved \u{201c}Klippy – note.html\u{201d} to Downloads."
        );
        assert_eq!(
            notify.operation.reveal_path.as_deref(),
            Some("/Users/me/Downloads/Klippy – note.html")
        );
    }

    #[test]
    fn a_cancelled_export_is_silent_and_a_failed_one_notifies() {
        let mut model = Model::default();
        let cancelled = ExportResult {
            path: None,
            error: String::new(),
        };
        Klippy
            .update(
                Event::ExportFinished {
                    result: cancelled,
                    quick: false,
                },
                &mut model,
            )
            .expect_done();

        let failed = ExportResult {
            path: None,
            error: "Disk full".into(),
        };
        let mut cmd = Klippy.update(
            Event::ExportFinished {
                result: failed,
                quick: true,
            },
            &mut model,
        );
        assert_eq!(
            cmd.expect_one_effect().expect_notify().operation.title,
            "Export failed"
        );
    }

    #[test]
    fn clear_history_deletes_everything_saved() {
        let mut model = Model::default();
        for text in ["a", "b"] {
            let _ = Klippy.update(copied(text), &mut model);
        }
        let id = Klippy.view(&model).items[0].id;
        let _ = Klippy.update(Event::SelectItem(id), &mut model);

        Klippy
            .update(Event::ClearHistory, &mut model)
            .expect_only_render();
        let view = Klippy.view(&model);
        assert!(
            view.items.is_empty() && view.history_items.is_empty() && view.recent_items.is_empty()
        );
        assert_eq!(view.selected, None);
    }

    #[test]
    fn selecting_a_missing_item_does_nothing() {
        let mut model = Model::default();
        Klippy
            .update(Event::SelectItem(42), &mut model)
            .expect_done();
        assert_eq!(Klippy.view(&model).selected, None);
    }

    #[test]
    fn the_history_window_has_its_own_search() {
        let mut model = Model::default();
        for text in ["apple", "banana"] {
            let _ = Klippy.update(copied(text), &mut model);
        }
        let _ = Klippy.update(Event::HistorySearch("app".into()), &mut model);
        let view = Klippy.view(&model);
        let previews = |items: Vec<ClipItem>| -> Vec<String> {
            items.into_iter().map(|item| item.preview).collect()
        };
        assert_eq!(previews(view.history_items), ["apple"]);
        assert_eq!(previews(view.items), ["banana", "apple"]);
    }

    #[test]
    fn navigates() {
        let mut model = Model::default();
        Klippy
            .update(Event::Navigate(Page::Home), &mut model)
            .expect_only_render();
        assert_eq!(Klippy.view(&model).page, Page::Home);
    }

    #[test]
    fn shows_initial_count() {
        let model = Model::default();
        assert_eq!(Klippy.view(&model).count, "Count is: 0");
    }

    #[test]
    fn increments_and_renders() {
        let mut model = Model::default();
        Klippy
            .update(Event::Increment, &mut model)
            .expect_only_render();
        assert_eq!(Klippy.view(&model).count, "Count is: 1");
    }

    #[test]
    fn resets() {
        let mut model = Model::default();
        let _ = Klippy.update(Event::Decrement, &mut model);
        let _ = Klippy.update(Event::Reset, &mut model);
        assert_eq!(Klippy.view(&model).count, "Count is: 0");
    }
}
