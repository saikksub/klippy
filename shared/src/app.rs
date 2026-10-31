use crux_core::{
    App, Command,
    capability::Operation,
    macros::effect,
    render::{RenderOperation, render},
};
use facet::Facet;
use serde::{Deserialize, Serialize};

#[derive(Facet, Serialize, Deserialize, Clone, Debug)]
#[repr(C)]
pub enum Event {
    /// Sent by the shell once the splash screen is up.
    Start,
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
    /// The user clicked a history item to copy it again.
    CopyItem(u64),
    /// The user picked an item to view in the History window.
    SelectItem(u64),
    /// The History window's own search, separate from the main window's.
    HistorySearch(String),
    Search(String),
    /// How many quick copies add the clipboard to the history (1 to 4).
    SetCopyTrigger(u32),
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
    /// Shown instead of `Home` when the app can't read the clipboard.
    Setup,
    Home,
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
}

impl Operation for ClipboardRead {
    type Output = ClipboardSnapshot;
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
    ClipboardAccess(ClipboardAccessCheck),
    NotificationPermission(NotificationPermissionRequest),
    ClipboardRead(ClipboardRead),
    ClipboardWrite(ClipboardWrite),
}

/// Largest text or single format kept for one item. Bigger formats (often
/// image renderings apps add alongside text) are dropped; bigger text skips
/// the item, so one huge copy can't exhaust memory.
const MAX_ITEM_BYTES: usize = 10 * 1024 * 1024;

const DEFAULT_COPY_TRIGGER: u32 = 3;
const MAX_COPY_TRIGGER: u32 = 4;

/// Copying `copy_trigger` times within this window adds the clipboard to the
/// history: 1s for 1 copy, then 0.5s more per extra copy, up to 2.5s for 4.
fn copy_window_ms(copy_trigger: u32) -> u64 {
    500 + 500 * u64::from(copy_trigger)
}

struct Clip {
    id: u64,
    content: ClipboardContent,
}

pub struct Model {
    page: Page,
    clipboard_access: Option<bool>,
    notifications_allowed: Option<bool>,
    /// Newest first, no duplicates.
    history: Vec<Clip>,
    next_id: u64,
    /// Timestamps of recent clipboard changes, for spotting a triple copy.
    recent_copies: Vec<u64>,
    copy_trigger: u32,
    search: String,
    history_search: String,
    selected: Option<u64>,
    count: isize,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            page: Page::default(),
            clipboard_access: None,
            notifications_allowed: None,
            history: Vec::new(),
            next_id: 0,
            recent_copies: Vec::new(),
            copy_trigger: DEFAULT_COPY_TRIGGER,
            search: String::new(),
            history_search: String::new(),
            selected: None,
            count: 0,
        }
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
}

#[derive(Facet, Serialize, Deserialize, Clone, Default)]
pub struct ViewModel {
    pub page: Page,
    /// "true" / "false" once checked, empty before.
    pub clipboard_access: String,
    /// `None` until the permission request has been answered.
    pub notifications_allowed: Option<bool>,
    pub search: String,
    pub copy_trigger: u32,
    /// Explains the current trigger, including its time window.
    pub copy_trigger_hint: String,
    /// History items matching `search`, newest first.
    pub items: Vec<ClipItem>,
    pub history_search: String,
    /// History items matching `history_search`, for the History window.
    pub history_items: Vec<ClipItem>,
    pub selected: Option<SelectedClip>,
    pub count: String,
}

#[derive(Default)]
pub struct Klippy;

impl App for Klippy {
    type Event = Event;
    type Model = Model;
    type ViewModel = ViewModel;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Start => {
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
            Event::Copied(snapshot) => match snapshot.content {
                Some(mut content)
                    if !content.text.trim().is_empty() && content.text.len() <= MAX_ITEM_BYTES =>
                {
                    content
                        .formats
                        .retain(|format| format.data.len() <= MAX_ITEM_BYTES);
                    add_to_history(model, content);
                }
                _ => return Command::done(),
            },
            Event::CopyItem(id) => {
                let Some(clip) = model.history.iter().find(|clip| clip.id == id) else {
                    return Command::done();
                };
                // The item keeps its place in the list.
                return Command::notify_shell(ClipboardWrite {
                    content: clip.content.clone(),
                })
                .into();
            }
            Event::SelectItem(id) => {
                if !model.history.iter().any(|clip| clip.id == id) {
                    return Command::done();
                }
                model.selected = Some(id);
            }
            Event::HistorySearch(search) => model.history_search = search,
            Event::Search(search) => model.search = search,
            Event::SetCopyTrigger(count) => {
                model.copy_trigger = count.clamp(1, MAX_COPY_TRIGGER);
                model.recent_copies.clear();
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
            notifications_allowed: model.notifications_allowed,
            clipboard_access: model
                .clipboard_access
                .map(|granted| granted.to_string())
                .unwrap_or_default(),
            search: model.search.clone(),
            copy_trigger: model.copy_trigger,
            copy_trigger_hint: copy_trigger_hint(model.copy_trigger),
            items: items_matching(model, &model.search),
            history_search: model.history_search.clone(),
            history_items: items_matching(model, &model.history_search),
            selected: model
                .selected
                .and_then(|id| model.history.iter().find(|clip| clip.id == id))
                .map(|clip| SelectedClip {
                    id: clip.id,
                    text: clip.content.text.clone(),
                    formats: clip.content.formats.clone(),
                }),
            count: format!("Count is: {}", model.count),
        }
    }
}

/// Adds to the top of the history. Content already there moves to the top,
/// keeping the newest formats.
fn add_to_history(model: &mut Model, content: ClipboardContent) {
    let id = match model
        .history
        .iter()
        .position(|clip| clip.content.text == content.text)
    {
        Some(index) => model.history.remove(index).id,
        None => {
            model.next_id += 1;
            model.next_id
        }
    };
    model.history.insert(0, Clip { id, content });
}

fn copy_trigger_hint(copy_trigger: u32) -> String {
    if copy_trigger == 1 {
        return "Every copy is added to the history.".into();
    }
    #[allow(clippy::cast_precision_loss)]
    let seconds = copy_window_ms(copy_trigger) as f64 / 1000.0;
    format!("Copy {copy_trigger} times within {seconds} seconds to add it to the history.")
}

fn items_matching(model: &Model, search: &str) -> Vec<ClipItem> {
    model
        .history
        .iter()
        .filter(|clip| matches_search(&clip.content.text, search))
        .map(|clip| ClipItem {
            id: clip.id,
            preview: preview(&clip.content.text),
        })
        .collect()
}

fn matches_search(text: &str, search: &str) -> bool {
    let search = search.trim();
    search.is_empty() || text.to_lowercase().contains(&search.to_lowercase())
}

/// Collapses all whitespace (including newlines) so the text fits one line.
fn preview(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn starts_on_splash() {
        let model = Model::default();
        assert_eq!(Klippy.view(&model).page, Page::Splash);
    }

    #[test]
    fn start_checks_clipboard_access_then_shows_home() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(Event::Start, &mut model);
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
            Event::Copied(ClipboardSnapshot { content: None }),
            &mut model,
        );
        assert!(items(&model).is_empty());
    }

    #[test]
    fn start_shows_setup_when_clipboard_access_is_denied() {
        let mut model = Model::default();
        let mut cmd = Klippy.update(Event::Start, &mut model);
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
            })
        );
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
