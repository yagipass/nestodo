use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, Focusable, Font, FontWeight, Hsla,
    Image, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent, MouseUpEvent, NoAction, Pixels,
    PromptLevel, Radians, ScrollHandle, SharedString, StrikethroughStyle, StyledText, Subscription,
    Task, TextLayout, TextRun, Transformation, TransformationMatrix, Window, actions, canvas, div,
    fill, point, prelude::*, px, size,
};
use unicode_segmentation::UnicodeSegmentation;

use nestodo_desktop::error::Error;
use nestodo_desktop::icon::{ImageIcon, image_id, image_value};
use nestodo_desktop::outline::{
    END, Focus, Guide, Key, Outline, ROOT, Row, append, caret_in, clear_icon, destinations, guides,
    icon_uses, indent, key_for, merge_into_previous, move_by, move_to, next_hit, open_under,
    outdent, parent_of, remove, rename, search_hits, set_collapsed, set_comments, set_icon, split,
    subtree, toggle_done, visible_rows,
};
use nestodo_desktop::sync::Sync;
use nestodo_desktop::title::{link_label, link_target, tokens};
use nestodo_desktop::todos::{MAX_COMMENT_LENGTH, MAX_TITLE_LENGTH};
use nestodo_desktop::undo::UndoHistory;
use nestodo_desktop::wire::Todo;

use crate::ui::avatar::{Avatar, decode_image};
use crate::ui::backend::Backend;
use crate::ui::icon_picker::{IconEvent, IconPicker};
use crate::ui::icons::Icon;
use crate::ui::move_picker::{MovePicker, PickerEvent};
use crate::ui::text_field::{
    Copy, FieldEvent, SelectAll, TextField, offset_from_utf16, offset_to_utf16,
};
use crate::ui::theme::{Theme, theme};
use crate::ui::todo_icon::{Icons, todo_icon};
use crate::ui::widgets::{
    NAMED_TITLE_BAR_WIDTH, PAGE_WIDTH, Tooltip, bullet_mark, done_badge, icon, icon_button, kbd,
    modal, shadow,
};

actions!(
    outline,
    [
        Undo,
        ToggleCollapse,
        DeleteRow,
        OpenMove,
        OpenIcon,
        Complete,
        OpenComments,
        CloseComments,
        Split,
        Indent,
        Outdent,
        MoveUp,
        MoveDown,
        Collapse,
        Expand,
        PrevRow,
        NextRow,
        TitleBackspace,
        PostComment,
        BackToItem,
        SaveComment,
        CancelEdit,
        CloseSearch,
        NextMatch,
        PreviousMatch,
        ToggleShowDone,
        OpenSearch,
    ]
);

pub fn bindings() -> Vec<KeyBinding> {
    let title = Some("Title");
    let composer = Some("Composer");
    let edit = Some("CommentEdit");
    let thread = Some("Thread");
    let log = Some("ThreadLog");
    let outline = Some("Outline");
    let search = Some("Search");
    let workspace = Some("Workspace");
    vec![
        KeyBinding::new("cmd-z", Undo, outline),
        KeyBinding::new("ctrl-z", Undo, outline),
        KeyBinding::new("cmd-z", Undo, title),
        KeyBinding::new("ctrl-z", Undo, title),
        KeyBinding::new("ctrl-space", ToggleCollapse, title),
        KeyBinding::new("cmd-shift-backspace", DeleteRow, title),
        KeyBinding::new("ctrl-shift-backspace", DeleteRow, title),
        KeyBinding::new("cmd-shift-m", OpenMove, title),
        KeyBinding::new("ctrl-shift-m", OpenMove, title),
        KeyBinding::new("cmd-shift-e", OpenIcon, title),
        KeyBinding::new("ctrl-shift-e", OpenIcon, title),
        KeyBinding::new("cmd-enter", Complete, title),
        KeyBinding::new("ctrl-enter", Complete, title),
        KeyBinding::new("shift-enter", OpenComments, title),
        KeyBinding::new("escape", CloseComments, title),
        KeyBinding::new("enter", Split, title),
        KeyBinding::new("tab", Indent, title),
        KeyBinding::new("shift-tab", Outdent, title),
        KeyBinding::new("alt-shift-up", MoveUp, title),
        KeyBinding::new("cmd-shift-up", MoveUp, title),
        KeyBinding::new("ctrl-shift-up", MoveUp, title),
        KeyBinding::new("alt-shift-down", MoveDown, title),
        KeyBinding::new("cmd-shift-down", MoveDown, title),
        KeyBinding::new("ctrl-shift-down", MoveDown, title),
        KeyBinding::new("cmd-up", Collapse, title),
        KeyBinding::new("ctrl-up", Collapse, title),
        KeyBinding::new("cmd-down", Expand, title),
        KeyBinding::new("ctrl-down", Expand, title),
        KeyBinding::new("up", PrevRow, title),
        KeyBinding::new("ctrl-p", PrevRow, title),
        KeyBinding::new("down", NextRow, title),
        KeyBinding::new("ctrl-n", NextRow, title),
        KeyBinding::new("backspace", TitleBackspace, title),
        KeyBinding::new("cmd-z", NoAction, thread),
        KeyBinding::new("ctrl-z", NoAction, thread),
        KeyBinding::new("cmd-c", Copy, log),
        KeyBinding::new("cmd-a", SelectAll, log),
        KeyBinding::new("enter", PostComment, composer),
        KeyBinding::new("cmd-enter", PostComment, composer),
        KeyBinding::new("ctrl-enter", PostComment, composer),
        KeyBinding::new("escape", BackToItem, composer),
        KeyBinding::new("enter", SaveComment, edit),
        KeyBinding::new("cmd-enter", SaveComment, edit),
        KeyBinding::new("ctrl-enter", SaveComment, edit),
        KeyBinding::new("escape", CancelEdit, edit),
        KeyBinding::new("escape", CloseSearch, search),
        KeyBinding::new("enter", NextMatch, search),
        KeyBinding::new("shift-enter", PreviousMatch, search),
        KeyBinding::new("cmd-o", ToggleShowDone, workspace),
        KeyBinding::new("ctrl-o", ToggleShowDone, workspace),
        KeyBinding::new("cmd-f", OpenSearch, workspace),
    ]
}

pub enum OutlineEvent {
    ShowDoneChanged(bool),
    ReloadRequested,
    SignedOut,
}

impl gpui::EventEmitter<OutlineEvent> for OutlineView {}

#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag mirrors a separate piece of state in the web page's component"
)]
pub struct OutlineView {
    backend: Arc<dyn Backend>,
    outline: Outline,
    show_done: bool,
    query: String,
    searching: bool,
    current: Option<Key>,
    thread: Option<thread_pane::Thread>,
    drafts: HashMap<Key, thread_pane::Draft>,
    avatar: Option<Avatar>,
    moving: Option<Focus>,
    picker: Option<Entity<MovePicker>>,
    choosing_icon: Option<Focus>,
    icon_picker: Option<Entity<IconPicker>>,
    icons: Icons,
    icons_loading: bool,
    asked_icons: HashSet<String>,
    sync: Sync,
    history: UndoHistory,
    counter: u64,
    editing: Option<Key>,
    title_field: Entity<TextField>,
    composer: Entity<TextField>,
    comment_field: Entity<TextField>,
    search_field: Entity<TextField>,
    focus_handle: FocusHandle,
    log_focus: FocusHandle,
    scroll: ScrollHandle,
    refreshing: bool,
    pending_link: Option<String>,
    flush_task: Option<Task<()>>,
    needs_editor_check: bool,
    subscriptions: Vec<Subscription>,
}

impl Focusable for OutlineView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

struct Mirror {
    text: SharedString,
    runs: Vec<TextRun>,
    segments: Vec<Segment>,
    backgrounds: Vec<(Range<usize>, Hsla, Pixels, Pixels)>,
    icons: Vec<Range<usize>>,
}

struct Segment {
    display: Range<usize>,
    title: Range<usize>,
    href: Option<String>,
}

impl Mirror {
    fn link_at(&self, display: usize) -> Option<&String> {
        self.segments
            .iter()
            .find(|segment| segment.display.contains(&display))
            .and_then(|segment| segment.href.as_ref())
    }

    fn title_index(&self, display: usize) -> usize {
        for segment in &self.segments {
            if display < segment.display.end
                || (display == segment.display.end && segment.href.is_none())
            {
                if segment.href.is_some() {
                    return segment.title.start;
                }
                return segment.title.start + display.saturating_sub(segment.display.start);
            }
        }
        self.segments.last().map_or(0, |segment| segment.title.end)
    }
}

const THIN: &str = "\u{2009}";
const ICON_SLOT: &str = "\u{2003}";

fn mirror_for(
    text: &str,
    query: &str,
    colors: &Theme,
    base: Hsla,
    struck: bool,
    chips: bool,
    font: &Font,
) -> Mirror {
    let strikethrough = struck.then_some(StrikethroughStyle {
        thickness: px(1.),
        color: Some(base),
    });
    let mut display = String::new();
    let mut runs = Vec::new();
    let mut segments = Vec::new();
    let mut backgrounds = Vec::new();
    let mut icons = Vec::new();
    for part in tokens(text, query) {
        let start = display.len();
        if let Some(href) = &part.href {
            let label = link_label(href);
            let (before, after) = if chips {
                (
                    format!("{THIN}{THIN}{label}{THIN}"),
                    format!("{THIN}{THIN}"),
                )
            } else {
                (format!("{label}{THIN}"), String::new())
            };
            let icon_at = start + before.len();
            icons.push(icon_at..icon_at + ICON_SLOT.len());
            let shown = format!("{before}{ICON_SLOT}{after}");
            display.push_str(&shown);
            let background = if part.matched {
                colors.matched
            } else {
                colors.accent_soft_on_bg
            };
            runs.push(TextRun {
                len: shown.len(),
                font: font.clone(),
                color: colors.accent,
                background_color: (!chips).then_some(background),
                underline: None,
                strikethrough,
            });
            if chips {
                backgrounds.push((start..display.len(), background, px(22.), px(7.)));
            }
            segments.push(Segment {
                display: start..display.len(),
                title: part.from..part.to,
                href: Some(href.clone()),
            });
        } else {
            display.push_str(&part.text);
            runs.push(TextRun {
                len: part.text.len(),
                font: font.clone(),
                color: base,
                background_color: (part.matched && !chips).then_some(colors.matched),
                underline: None,
                strikethrough,
            });
            if part.matched && chips {
                backgrounds.push((start..display.len(), colors.matched, px(20.), px(2.)));
            }
            segments.push(Segment {
                display: start..display.len(),
                title: part.from..part.to,
                href: None,
            });
        }
    }
    let runs = runs.into_iter().filter(|run| run.len > 0).collect();
    Mirror {
        text: display.into(),
        runs,
        segments,
        backgrounds,
        icons,
    }
}

fn focus_after_leaving(rows: &[Row], key: &str, previous_first: bool) -> Option<Focus> {
    let index = rows.iter().position(|row| row.key == key)?;
    let previous = index.checked_sub(1).map(|previous| &rows[previous]);
    let next = rows[index + 1..]
        .iter()
        .find(|row| row.depth <= rows[index].depth);
    let row = if previous_first {
        previous.or(next)
    } else {
        next.or(previous)
    }?;
    Some(Focus {
        key: row.key.clone(),
        pos: END,
    })
}

impl OutlineView {
    pub fn new(
        backend: Arc<dyn Backend>,
        todos: &[Todo],
        show_done: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let outline = Outline::from_todos(todos, key_for);
        let sync = Sync::new(todos.iter().map(|todo| (key_for(todo.id), todo.id)));
        let title_field = cx.new(|cx| {
            TextField::new("Title", cx)
                .max_len(MAX_TITLE_LENGTH)
                .encode_urls()
        });
        let composer = cx.new(|cx| {
            TextField::new("Composer", cx)
                .multiline()
                .max_len(MAX_COMMENT_LENGTH)
                .encode_urls()
                .placeholder("Add to the log\u{2026}")
        });
        let comment_field = cx.new(|cx| {
            TextField::new("CommentEdit", cx)
                .multiline()
                .max_len(MAX_COMMENT_LENGTH)
                .encode_urls()
        });
        let search_field = cx.new(|cx| TextField::new("Search", cx).placeholder("Search"));
        let mut subscriptions = vec![
            cx.subscribe_in(&title_field, window, Self::on_title_event),
            cx.subscribe_in(&search_field, window, Self::on_search_event),
            cx.subscribe(&composer, |_, _, _: &FieldEvent, cx| cx.notify()),
            cx.subscribe(&comment_field, |_, _, _: &FieldEvent, cx| cx.notify()),
        ];
        let title_focus = title_field.read(cx).focus_handle(cx);
        let search_focus = search_field.read(cx).focus_handle(cx);
        subscriptions.push(cx.on_blur(&title_focus, window, |this, window, cx| {
            if window.is_window_active() && this.editing.is_some() {
                this.leave_editor(window, cx);
                this.editing = None;
                cx.notify();
            }
        }));
        subscriptions.push(cx.on_blur(&search_focus, window, |this, window, cx| {
            if window.is_window_active() && this.query.is_empty() && this.searching {
                this.searching = false;
                cx.notify();
            }
        }));
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh(cx);
                this.refresh_thread(cx);
            } else {
                this.flush_now(cx);
            }
        }));
        subscriptions.push(cx.on_app_quit(|this, cx| {
            this.flush_now(cx);
            async {}
        }));
        let clock = cx.background_executor().clone();
        let start = clock.now();
        let mut view = Self {
            backend,
            outline,
            show_done,
            query: String::new(),
            searching: false,
            current: None,
            thread: None,
            drafts: HashMap::new(),
            avatar: None,
            moving: None,
            picker: None,
            choosing_icon: None,
            icon_picker: None,
            icons: Icons::default(),
            icons_loading: false,
            asked_icons: HashSet::new(),
            sync,
            history: UndoHistory::with_clock(move || clock.now().saturating_duration_since(start)),
            counter: 0,
            editing: None,
            title_field,
            composer,
            comment_field,
            search_field,
            focus_handle: cx.focus_handle(),
            log_focus: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            refreshing: false,
            pending_link: None,
            flush_task: None,
            needs_editor_check: false,
            subscriptions,
        };
        view.load_icons(cx);
        view
    }

    pub fn has_unsaved_changes(&self) -> bool {
        self.sync.pending()
    }

    pub fn save_failed(&self) -> bool {
        self.sync.failed
    }

    pub fn flush_now(&mut self, cx: &mut Context<Self>) {
        self.history.end();
        self.sync.flush();
        self.pump(cx);
    }

    pub fn has_modal(&self) -> bool {
        self.picker.is_some() || self.icon_picker.is_some()
    }

    fn rows(&self) -> Vec<Row> {
        visible_rows(&self.outline, self.show_done)
    }

    fn hits(&self, rows: &[Row]) -> Vec<Key> {
        search_hits(&self.outline, rows, &self.query)
    }

    fn current_match(&self, hits: &[Key]) -> Option<Key> {
        self.current
            .clone()
            .filter(|current| hits.contains(current))
    }

    fn is_shown(&self, key: &str) -> bool {
        self.rows().iter().any(|row| row.key == key)
    }

    fn shown(rows: &[Row]) -> HashSet<Key> {
        rows.iter().map(|row| row.key.clone()).collect()
    }

    fn fresh(&mut self) -> Key {
        self.counter += 1;
        format!("new{}", self.counter)
    }

    fn focused(&self, cx: &App) -> Option<Focus> {
        let key = self.editing.as_ref()?;
        Some(Focus {
            key: key.clone(),
            pos: self.title_field.read(cx).selection().start,
        })
    }

    fn leave_editor(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.flush_now(cx);
        }
    }

    fn focus_row(&mut self, focus: Focus, window: &mut Window, cx: &mut Context<Self>) {
        let Focus { key, pos } = focus;
        let rows = self.rows();
        let Some(index) = rows.iter().position(|row| row.key == key) else {
            return;
        };
        if self.editing.as_ref() != Some(&key) {
            self.leave_editor(window, cx);
        }
        let Some(item) = self.outline.get(&key) else {
            return;
        };
        let text = item.title.clone();
        let at = caret_in(&text, pos);
        let field = self.title_field.clone();
        field.update(cx, |field, cx| {
            field.set_text(&text, at, cx);
            field.set_read_only(self.sync.failed, cx);
        });
        self.editing = Some(key.clone());
        window.focus(&field.focus_handle(cx), cx);
        self.scroll.scroll_to_item(self.row_child_offset() + index);
        self.follow_caret(&key, cx);
        cx.notify();
    }

    fn row_child_offset(&self) -> usize {
        usize::from(self.sync.failed)
    }

    fn reveal(&self, key: &str, rows: &[Row]) {
        if let Some(index) = rows.iter().position(|row| row.key == key) {
            self.scroll.scroll_to_item(self.row_child_offset() + index);
        }
    }

    fn choose_match(&mut self) {
        let rows = self.rows();
        let hits = self.hits(&rows);
        if self.current_match(&hits).is_none() {
            self.current = next_hit(&rows, &hits, self.current.as_deref(), 1);
        }
        if let Some(current) = &self.current {
            self.reveal(current, &rows);
        }
    }

    fn step_match(&mut self, step: isize, cx: &mut Context<Self>) {
        let rows = self.rows();
        let hits = self.hits(&rows);
        self.current = next_hit(&rows, &hits, self.current.as_deref(), step);
        if let Some(current) = &self.current {
            self.reveal(current, &rows);
        }
        cx.notify();
    }

    fn detach_if_hidden(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.editing.clone() else {
            return;
        };
        if !self.is_shown(&key) || !self.outline.contains(&key) {
            self.editing = None;
            self.flush_now(cx);
            window.focus(&self.focus_handle, cx);
            return;
        }
        let text = self.outline.item(&key).title.clone();
        self.title_field.update(cx, |field, cx| {
            if !field.is_composing() {
                field.sync_text(&text, cx);
            }
        });
    }

    fn apply(
        &mut self,
        change: Option<nestodo_desktop::outline::Change>,
        focus: Option<Focus>,
        before: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(change) = change else {
            return false;
        };
        if self.sync.failed {
            return false;
        }
        self.history.record(&self.outline, &change, before);
        self.commit(change, focus, window, cx);
        true
    }

    fn commit(
        &mut self,
        change: nestodo_desktop::outline::Change,
        focus: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.outline = change.outline;
        self.sync.apply(&change.ops);
        self.pump(cx);
        let next = focus.or(change.focus);
        self.detach_if_hidden(window, cx);
        if let Some(next) = next {
            self.focus_row(next, window, cx);
        }
        self.sync_thread(cx);
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed || self.has_modal() {
            return;
        }
        if let Some(change) = self.history.undo(&self.outline) {
            self.commit(change, None, window, cx);
        }
    }

    pub fn toggle_show_done(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_done = !self.show_done;
        cx.emit(OutlineEvent::ShowDoneChanged(self.show_done));
        self.detach_if_hidden(window, cx);
        cx.notify();
    }

    pub fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.searching = true;
        self.search_field.update(cx, TextField::select_all_text);
        window.focus(&self.search_field.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows();
        let found = self.current_match(&self.hits(&rows)).map(|key| {
            let pos = tokens(&self.outline.item(&key).title, &self.query)
                .into_iter()
                .find(|part| part.matched)
                .map_or(END, |part| part.from);
            Focus { key, pos }
        });
        self.query.clear();
        self.current = None;
        self.searching = false;
        self.search_field
            .update(cx, |field, cx| field.set_text("", 0, cx));
        match found {
            Some(focus) => self.focus_row(focus, window, cx),
            None => window.focus(&self.focus_handle, cx),
        }
        cx.notify();
    }

    pub fn set_avatar(&mut self, avatar: Option<Avatar>, cx: &mut Context<Self>) {
        self.avatar = avatar;
        cx.notify();
    }

    fn name_of(&self, key: &str) -> String {
        match self.outline.item(key).title.as_str() {
            "" => "Untitled".to_string(),
            title => title.to_string(),
        }
    }

    fn confirm_done(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
        toggle: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if self.sync.failed {
            return;
        }
        let open = open_under(&self.outline, key).len();
        if self.outline.item(key).done || open == 0 {
            toggle(self, window, cx);
            return;
        }
        let name = self.name_of(key);
        let message = format!("Mark \u{201c}{name}\u{201d} as done?");
        let detail = if open == 1 {
            "1 item under it isn\u{2019}t done yet. It\u{2019}ll be marked as done too.".to_string()
        } else {
            format!(
                "{open} items under it aren\u{2019}t done yet. They\u{2019}ll be marked as done too."
            )
        };
        let answer = window.prompt(
            PromptLevel::Info,
            &message,
            Some(&detail),
            &["Mark all as done", "Cancel"],
            cx,
        );
        let key = key.to_string();
        cx.spawn_in(window, async move |this, cx| {
            if answer.await.ok() == Some(0) {
                this.update_in(cx, |this, window, cx| {
                    if this.outline.get(&key).is_some_and(|item| !item.done) {
                        toggle(this, window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn complete(&mut self, key: &str, pos: usize, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_shown(key) {
            return;
        }
        let before = self.focused(cx);
        let target = key.to_string();
        self.confirm_done(key, window, cx, move |this, window, cx| {
            this.complete_confirmed(&target, pos, before, window, cx);
        });
    }

    fn complete_confirmed(
        &mut self,
        key: &str,
        pos: usize,
        before: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.rows();
        if !rows.iter().any(|row| row.key == key) {
            return;
        }
        let change = toggle_done(&self.outline, key);
        if self.show_done || !change.outline.item(key).done {
            let focus = Focus {
                key: key.to_string(),
                pos,
            };
            self.apply(Some(change), Some(focus), before, window, cx);
            return;
        }
        let next = focus_after_leaving(&rows, key, false);
        self.apply(Some(change), next, before, window, cx);
    }

    fn remove_row(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let under = subtree(&self.outline, key).len() - 1;
        if under == 0 {
            self.remove_confirmed(key, self.focused(cx), window, cx);
            return;
        }
        let name = self.name_of(key);
        let noun = if under == 1 { "item" } else { "items" };
        let message = format!("Delete \u{201c}{name}\u{201d} and the {under} {noun} under it?");
        let before = self.focused(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            None,
            &["Delete all", "Cancel"],
            cx,
        );
        let key = key.to_string();
        cx.spawn_in(window, async move |this, cx| {
            if answer.await.ok() == Some(0) {
                this.update_in(cx, |this, window, cx| {
                    if this.outline.contains(&key) {
                        this.remove_confirmed(&key, before, window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    fn remove_confirmed(
        &mut self,
        key: &str,
        before: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = focus_after_leaving(&self.rows(), key, true);
        self.apply(Some(remove(&self.outline, key)), next, before, window, cx);
    }

    fn open_move(&mut self, focus: Focus, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        self.leave_editor(window, cx);
        self.editing = None;
        let outline = self.outline.clone();
        let key = focus.key.clone();
        let show_done = self.show_done;
        let picker = cx.new(|cx| MovePicker::new(outline, key, show_done, window, cx));
        let subscription =
            cx.subscribe_in(&picker, window, |this, _, event, window, cx| match event {
                PickerEvent::Picked(target) => this.move_row(target, window, cx),
                PickerEvent::Dismissed => this.close_move(window, cx),
            });
        self.subscriptions.push(subscription);
        self.moving = Some(focus);
        self.picker = Some(picker);
        cx.notify();
    }

    fn move_row(&mut self, target: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(moving) = self.moving.take() else {
            return;
        };
        self.picker = None;
        let next = focus_after_leaving(&self.rows(), &moving.key, false);
        let still_allowed = self.outline.contains(&moving.key)
            && destinations(&self.outline, &moving.key, self.show_done, "")
                .iter()
                .any(|destination| destination.key == target);
        let change = still_allowed.then(|| move_to(&self.outline, &moving.key, target));
        if !self.apply(change, next, Some(moving.clone()), window, cx) {
            self.focus_row(moving, window, cx);
        }
        cx.notify();
    }

    fn close_move(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(moving) = self.moving.take() else {
            return;
        };
        self.picker = None;
        self.focus_row(moving, window, cx);
        cx.notify();
    }

    fn open_icon(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed || self.has_modal() || !self.outline.contains(key) {
            return;
        }
        let pos = match self.focused(cx) {
            Some(here) if here.key == key => here.pos,
            _ => END,
        };
        self.leave_editor(window, cx);
        self.editing = None;
        let item = self.outline.item(key);
        let (current, title) = (item.icon.clone(), item.title.clone());
        let backend = self.backend.clone();
        let icons = self.icons.clone();
        let uses = icon_uses(&self.outline);
        let picker = cx.new(|cx| IconPicker::new(current, title, backend, icons, uses, window, cx));
        let subscription =
            cx.subscribe_in(
                &picker,
                window,
                |this, picker, event, window, cx| match event {
                    IconEvent::Chosen(icon) => this.choose_icon(icon.as_deref(), window, cx),
                    IconEvent::Added {
                        image,
                        picture,
                        waiting,
                    } => {
                        let open = this.icon_picker.as_ref() == Some(picker);
                        this.image_added(image, picture, *waiting && open, window, cx);
                    }
                    IconEvent::Renamed(image) => this.image_renamed(image, cx),
                    IconEvent::Deleted(id) => this.image_deleted(id, cx),
                    IconEvent::Dismissed => this.close_icon(window, cx),
                },
            );
        self.subscriptions.push(subscription);
        self.choosing_icon = Some(Focus {
            key: key.to_owned(),
            pos,
        });
        self.icon_picker = Some(picker);
        cx.notify();
    }

    fn choose_icon(&mut self, icon: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.choosing_icon.take() else {
            return;
        };
        self.icon_picker = None;
        let change = self
            .outline
            .contains(&row.key)
            .then(|| set_icon(&self.outline, &row.key, icon))
            .flatten();
        if !self.apply(change, Some(row.clone()), Some(row.clone()), window, cx) {
            self.focus_row(row, window, cx);
        }
        cx.notify();
    }

    fn close_icon(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(row) = self.choosing_icon.take() else {
            return;
        };
        self.icon_picker = None;
        self.focus_row(row, window, cx);
        cx.notify();
    }

    fn image_added(
        &mut self,
        image: &ImageIcon,
        picture: &Arc<Image>,
        use_it: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.icons.list.iter().any(|known| known.id == image.id) {
            self.icons.list.push(image.clone());
        }
        self.icons
            .pictures
            .insert(image.id.clone(), picture.clone());
        if use_it {
            self.choose_icon(Some(&image.value()), window, cx);
        } else {
            self.sync_icon_picker(cx);
            cx.notify();
        }
    }

    fn image_renamed(&mut self, image: &ImageIcon, cx: &mut Context<Self>) {
        if let Some(known) = self
            .icons
            .list
            .iter_mut()
            .find(|known| known.id == image.id)
        {
            known.clone_from(image);
        }
        cx.notify();
    }

    fn image_deleted(&mut self, id: &str, cx: &mut Context<Self>) {
        self.icons.list.retain(|known| known.id != id);
        self.icons.pictures.remove(id);
        self.outline = clear_icon(&self.outline, &image_value(id));
        self.history.clear();
        self.sync_icon_picker(cx);
        cx.notify();
    }

    fn image_ids(&self) -> Vec<String> {
        icon_uses(&self.outline)
            .into_keys()
            .filter_map(|icon| image_id(&icon).map(str::to_owned))
            .collect()
    }

    fn load_icons(&mut self, cx: &mut Context<Self>) {
        if self.icons_loading {
            return;
        }
        self.icons_loading = true;
        self.asked_icons.extend(self.image_ids());
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let asked = backend.clone();
            let listed = cx.background_spawn(async move { asked.icons() }).await;
            let Ok(missing) = this.update(cx, |this, cx| this.icons_listed(listed.ok(), cx)) else {
                return;
            };
            if missing.is_empty() {
                return;
            }
            let pictures: Vec<(String, Arc<Image>)> = cx
                .background_spawn(async move {
                    missing
                        .into_iter()
                        .filter_map(|image| {
                            let bytes = backend.icon_image(&image.url).ok()?;
                            Some((image.id, decode_image(bytes)?))
                        })
                        .collect()
                })
                .await;
            this.update(cx, |this, cx| {
                this.icons.pictures.extend(pictures);
                this.sync_icon_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn icons_listed(
        &mut self,
        listed: Option<Vec<ImageIcon>>,
        cx: &mut Context<Self>,
    ) -> Vec<ImageIcon> {
        self.icons_loading = false;
        if let Some(listed) = listed {
            self.icons.list = listed;
        }
        let missing = self
            .icons
            .list
            .iter()
            .filter(|image| !self.icons.pictures.contains_key(&image.id))
            .cloned()
            .collect();
        self.check_unknown_icons(cx);
        self.sync_icon_picker(cx);
        cx.notify();
        missing
    }

    fn check_unknown_icons(&mut self, cx: &mut Context<Self>) {
        if self.icons_loading {
            return;
        }
        let unknown = self.image_ids().into_iter().any(|id| {
            !self.asked_icons.contains(&id) && !self.icons.list.iter().any(|known| known.id == id)
        });
        if unknown {
            self.load_icons(cx);
        }
    }

    fn sync_icon_picker(&self, cx: &mut Context<Self>) {
        if let Some(picker) = &self.icon_picker {
            let icons = self.icons.clone();
            let uses = icon_uses(&self.outline);
            picker.update(cx, |picker, cx| picker.follow(icons, uses, cx));
        }
    }

    fn on_title(&mut self, key: &str, title: &str, composing: bool, cx: &mut Context<Self>) {
        let next = rename(&self.outline, key, title);
        self.history.type_text(&self.outline, &next, key, composing);
        self.outline = next;
        let now = cx.background_executor().now();
        self.sync.title(key, title, now);
        self.schedule_flush(cx);
        cx.notify();
    }

    fn schedule_flush(&mut self, cx: &mut Context<Self>) {
        let Some(due) = self.sync.next_due() else {
            return;
        };
        let wait = due.saturating_duration_since(cx.background_executor().now());
        self.flush_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(wait).await;
            this.update(cx, |this, cx| {
                let now = cx.background_executor().now();
                this.sync.flush_due(now);
                this.pump(cx);
                this.schedule_flush(cx);
            })
            .ok();
        }));
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        let Some(request) = self.sync.start() else {
            self.check_failed(cx);
            return;
        };
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let mut request = request;
            loop {
                let reply = cx
                    .background_spawn({
                        let backend = backend.clone();
                        async move { backend.request(&request) }
                    })
                    .await;
                let next = this.update(cx, |this, cx| {
                    if let Some(saved) = this.sync.finish(reply.map_err(|_| ())) {
                        this.comment_saved(saved, cx);
                    }
                    let next = this.sync.start();
                    if next.is_none() {
                        this.check_failed(cx);
                        this.sync_thread(cx);
                    }
                    next
                });
                match next {
                    Ok(Some(next)) => request = next,
                    _ => break,
                }
            }
        })
        .detach();
    }

    fn check_failed(&mut self, cx: &mut Context<Self>) {
        if self.sync.failed {
            for field in [&self.title_field, &self.composer, &self.comment_field] {
                field.update(cx, |field, cx| field.set_read_only(true, cx));
            }
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || self.sync.pending() || self.sync.failed {
            return;
        }
        self.refreshing = true;
        self.load_icons(cx);
        let revision = self.sync.revision;
        let backend = self.backend.clone();
        cx.spawn(async move |this, cx| {
            let latest = cx.background_spawn(async move { backend.list() }).await;
            this.update(cx, |this, cx| {
                this.refreshing = false;
                if this.sync.revision != revision || this.sync.pending() || this.sync.failed {
                    return;
                }
                let latest = match latest {
                    Ok(latest) => latest,
                    Err(Error::NotSignedIn) => {
                        cx.emit(OutlineEvent::SignedOut);
                        return;
                    }
                    Err(_) => return,
                };
                let keys: Vec<(Key, u64)> = latest
                    .iter()
                    .map(|todo| {
                        (
                            this.sync
                                .key_of(todo.id)
                                .unwrap_or_else(|| key_for(todo.id)),
                            todo.id,
                        )
                    })
                    .collect();
                this.sync.track(keys.iter().cloned());
                let sync = &this.sync;
                this.outline = Outline::from_todos(&latest, |id| {
                    sync.key_of(id).unwrap_or_else(|| key_for(id))
                });
                this.needs_editor_check = true;
                this.check_unknown_icons(cx);
                this.sync_icon_picker(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn on_title_event(
        &mut self,
        field: &Entity<TextField>,
        event: &FieldEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(key) = self
            .editing
            .clone()
            .filter(|key| self.outline.contains(key))
        else {
            return;
        };
        match event {
            FieldEvent::Changed { composing } => {
                let text = field.read(cx).text().to_string();
                if self.outline.get(&key).map(|item| &item.title) != Some(&text) {
                    self.on_title(&key, &text, *composing, cx);
                }
            }
            FieldEvent::CompositionEnded => self.history.composed(),
        }
    }

    fn on_search_event(
        &mut self,
        field: &Entity<TextField>,
        event: &FieldEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FieldEvent::Changed { .. } = event {
            let before = std::mem::replace(&mut self.query, field.read(cx).text().to_string());
            if before.trim() != self.query.trim() {
                self.choose_match();
            }
            cx.notify();
        }
    }

    fn title_context(&self, cx: &App) -> Option<(Key, usize, Vec<Row>, usize)> {
        let key = self.editing.as_ref()?;
        let rows = self.rows();
        let index = rows.iter().position(|row| &row.key == key)?;
        let pos = self.title_field.read(cx).selection().start;
        Some((key.clone(), pos, rows, index))
    }

    fn on_undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        self.undo(window, cx);
    }

    fn on_toggle_collapse(
        &mut self,
        _: &ToggleCollapse,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(here) = self.focused(cx) else { return };
        let collapsed = self.outline.item(&here.key).collapsed;
        let change = set_collapsed(&self.outline, &here.key, !collapsed);
        let before = Some(here.clone());
        self.apply(change, Some(here), before, window, cx);
    }

    fn on_delete_row(&mut self, _: &DeleteRow, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else { return };
        self.remove_row(&here.key, window, cx);
    }

    fn on_open_move(&mut self, _: &OpenMove, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else {
            return;
        };
        self.open_move(here, window, cx);
    }

    fn on_open_icon(&mut self, _: &OpenIcon, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else {
            return;
        };
        self.open_icon(&here.key, window, cx);
    }

    fn on_complete(&mut self, _: &Complete, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else { return };
        self.complete(&here.key, here.pos, window, cx);
    }

    fn on_split(&mut self, _: &Split, window: &mut Window, cx: &mut Context<Self>) {
        if self.title_field.read(cx).is_composing() {
            return;
        }
        let Some((key, pos, rows, _)) = self.title_context(cx) else {
            return;
        };
        let fresh = self.fresh();
        let shown = Self::shown(&rows);
        let change = split(&self.outline, &key, pos, &fresh, &shown);
        let before = self.focused(cx);
        self.apply(Some(change), None, before, window, cx);
    }

    fn on_indent(&mut self, _: &Indent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, rows, _)) = self.title_context(cx) else {
            return;
        };
        let change = indent(&self.outline, &key, &Self::shown(&rows));
        let here = Focus { key, pos };
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn on_outdent(&mut self, _: &Outdent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, _, _)) = self.title_context(cx) else {
            return;
        };
        let change = outdent(&self.outline, &key);
        let here = Focus { key, pos };
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn move_step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, rows, _)) = self.title_context(cx) else {
            return;
        };
        let change = move_by(&self.outline, &key, step, &Self::shown(&rows));
        let here = Focus { key, pos };
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn on_move_up(&mut self, _: &MoveUp, window: &mut Window, cx: &mut Context<Self>) {
        self.move_step(-1, window, cx);
    }

    fn on_move_down(&mut self, _: &MoveDown, window: &mut Window, cx: &mut Context<Self>) {
        self.move_step(1, window, cx);
    }

    fn collapse_to(&mut self, collapsed: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else { return };
        let change = set_collapsed(&self.outline, &here.key, collapsed);
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn on_collapse(&mut self, _: &Collapse, window: &mut Window, cx: &mut Context<Self>) {
        self.collapse_to(true, window, cx);
    }

    fn on_expand(&mut self, _: &Expand, window: &mut Window, cx: &mut Context<Self>) {
        self.collapse_to(false, window, cx);
    }

    fn step_row(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, rows, index)) = self.title_context(cx) else {
            cx.propagate();
            return;
        };
        let Some(target) = index
            .checked_add_signed(step)
            .and_then(|target| rows.get(target))
        else {
            cx.propagate();
            return;
        };
        let title = &self.outline.item(&key).title;
        let units = offset_to_utf16(title, pos);
        let next_title = &self.outline.item(&target.key).title;
        let focus = Focus {
            key: target.key.clone(),
            pos: offset_from_utf16(next_title, units),
        };
        self.focus_row(focus, window, cx);
    }

    fn on_prev_row(&mut self, _: &PrevRow, window: &mut Window, cx: &mut Context<Self>) {
        self.step_row(-1, window, cx);
    }

    fn on_next_row(&mut self, _: &NextRow, window: &mut Window, cx: &mut Context<Self>) {
        self.step_row(1, window, cx);
    }

    fn on_title_backspace(
        &mut self,
        _: &TitleBackspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let at_start = self.title_field.read(cx).is_at_start();
        let Some((key, _, rows, _)) = self.title_context(cx).filter(|_| at_start) else {
            cx.propagate();
            return;
        };
        let item = self.outline.item(&key);
        if item.title.is_empty()
            && item.comments == 0
            && item.icon.is_none()
            && item.children.is_empty()
        {
            self.remove_row(&key, window, cx);
            return;
        }
        if let Some(merged) = merge_into_previous(&self.outline, &rows, &key) {
            let before = self.focused(cx);
            self.apply(Some(merged), None, before, window, cx);
            return;
        }
        cx.propagate();
    }

    fn on_close_search(&mut self, _: &CloseSearch, window: &mut Window, cx: &mut Context<Self>) {
        if self.search_field.read(cx).is_composing() {
            return;
        }
        self.close_search(window, cx);
    }

    fn on_next_match(&mut self, _: &NextMatch, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_field.read(cx).is_composing() {
            self.step_match(1, cx);
        }
    }

    fn on_previous_match(&mut self, _: &PreviousMatch, _: &mut Window, cx: &mut Context<Self>) {
        if !self.search_field.read(cx).is_composing() {
            self.step_match(-1, cx);
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.is_focused(window) || self.has_modal() {
            return;
        }
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if keystroke.key_char.as_deref() == Some("/")
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            self.open_search(window, cx);
            cx.stop_propagation();
        }
    }

    fn click_row_title(
        &mut self,
        key: &str,
        pos: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_row(
            Focus {
                key: key.to_string(),
                pos,
            },
            window,
            cx,
        );
        if self.editing.as_deref() == Some(key) {
            self.title_field
                .update(cx, super::text_field::TextField::begin_mouse_selection);
        }
    }

    fn blur_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.leave_editor(window, cx);
            self.editing = None;
        }
        window.focus(&self.focus_handle, cx);
    }

    fn toggle_row(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.outline.contains(key) {
            return;
        }
        let before = self.focused(cx);
        self.blur_editor(window, cx);
        let collapsed = self.outline.item(key).collapsed;
        let change = set_collapsed(&self.outline, key, !collapsed);
        self.apply(change, None, before, window, cx);
    }

    fn check_row(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.outline.contains(key) {
            return;
        }
        let target = key.to_string();
        self.confirm_done(key, window, cx, move |this, window, cx| {
            let before = this.focused(cx);
            this.blur_editor(window, cx);
            let change = toggle_done(&this.outline, &target);
            this.apply(Some(change), None, before, window, cx);
        });
    }

    fn add_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fresh = self.fresh();
        let before = self.focused(cx);
        self.blur_editor(window, cx);
        let change = append(&self.outline, ROOT, &fresh);
        self.apply(Some(change), None, before, window, cx);
    }

    pub fn render_tools(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let roomy = window.viewport_size().width >= px(NAMED_TITLE_BAR_WIDTH);
        let search: AnyElement = if self.searching {
            let rows = self.rows();
            let hits = self.hits(&rows);
            let at = self
                .current_match(&hits)
                .and_then(|current| hits.iter().position(|hit| *hit == current))
                .map_or(0, |index| index + 1);
            let step_button = |id: &'static str, glyph: Icon, label: &'static str, step: isize| {
                div()
                    .id(id)
                    .debug_selector(move || id.into())
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(24.))
                    .rounded(px(7.))
                    .when(hits.is_empty(), |button| button.opacity(0.4))
                    .when(!hits.is_empty(), |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.hover))
                            .on_click(cx.listener(move |this, _, _, cx| this.step_match(step, cx)))
                    })
                    .tooltip(Tooltip::text(label))
                    .child(icon(glyph, 15., colors.muted))
            };
            let steps = (!self.query.trim().is_empty()).then(|| {
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.))
                    .when(roomy, |steps| {
                        steps.child(
                            div()
                                .debug_selector(|| "match-count".into())
                                .mr(px(4.))
                                .text_size(px(13.))
                                .text_color(colors.muted)
                                .child(format!("{at} / {}", hits.len())),
                        )
                    })
                    .child(step_button(
                        "previous-match",
                        Icon::Up,
                        "Previous match (\u{21e7} Enter)",
                        -1,
                    ))
                    .child(step_button(
                        "next-match",
                        Icon::Down,
                        "Next match (Enter)",
                        1,
                    ))
            });
            div()
                .id("search")
                .debug_selector(|| "search".into())
                .on_action(cx.listener(Self::on_close_search))
                .on_action(cx.listener(Self::on_next_match))
                .on_action(cx.listener(Self::on_previous_match))
                .flex()
                .flex_1()
                .items_center()
                .gap(px(8.))
                .w(px(340.))
                .min_w(px(120.))
                .h(px(32.))
                .pl(px(10.))
                .pr(px(3.))
                .border_1()
                .border_color(colors.accent)
                .rounded(px(9.))
                .bg(colors.surface)
                .when(roomy, |search| {
                    search.child(icon(Icon::Filter, 17., colors.muted))
                })
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(colors.text)
                        .child(self.search_field.clone()),
                )
                .children(steps)
                .child(
                    div()
                        .id("close-search")
                        .debug_selector(|| "close-search".into())
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(24.))
                        .rounded(px(7.))
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.hover))
                        .on_click(cx.listener(|this, _, window, cx| this.close_search(window, cx)))
                        .child(icon(Icon::Close, 15., colors.muted)),
                )
                .into_any_element()
        } else {
            icon_button("open-search", Icon::Filter, 19., colors.hover_on_bg, cx)
                .tooltip(Tooltip::text("Search (\u{2318} F)"))
                .on_click(cx.listener(|this, _, window, cx| this.open_search(window, cx)))
                .into_any_element()
        };
        let pressed = self.show_done;
        let show_done = div()
            .id("show-completed")
            .debug_selector(|| "show-completed".into())
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(32.))
            .rounded(px(9.))
            .cursor_pointer()
            .when(pressed, |button| button.bg(colors.accent_soft_on_bg))
            .when(!pressed, move |button| {
                button.hover(move |style| style.bg(colors.hover_on_bg))
            })
            .tooltip(Tooltip::text("Show completed (Ctrl/\u{2318} O)"))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_show_done(window, cx)))
            .child(icon(
                Icon::Completed,
                19.,
                if pressed { colors.accent } else { colors.muted },
            ));
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .min_w_0()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(search)
            .child(show_done)
            .into_any_element()
    }

    fn render_row(
        &mut self,
        row: &Row,
        guide: &Guide,
        is_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(cx);
        let colors = if is_current {
            Theme {
                matched: colors.matched_current,
                ..colors
            }
        } else {
            colors
        };
        let item = self.outline.item(&row.key);
        let key = row.key.clone();
        let has_children = !item.children.is_empty();
        let editing_title = self.editing.as_ref() == Some(&key);
        let active = editing_title;
        let threaded = self.thread.as_ref().is_some_and(|thread| thread.key == key);
        let muted_text = item.done || row.under;
        let base = if muted_text {
            colors.muted
        } else {
            colors.text
        };
        let group: SharedString = format!("row-{key}").into();
        let name = self.name_of(&key);
        let weight = if has_children {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        };

        let glyph = item
            .icon
            .as_deref()
            .and_then(|value| todo_icon(value, 17., item.done, &self.icons));
        let with_icon = glyph.is_some();

        let rail = || div().absolute().left(px(12.)).w(px(2.)).bg(colors.guide);
        let guides = guide.through.iter().enumerate().map(|(level, &through)| {
            div()
                .id(level)
                .relative()
                .flex_none()
                .w(px(26.))
                .when(through, |column| column.child(rail().top_0().bottom_0()))
                .when(level + 1 == row.depth, |column| {
                    column.child(
                        div()
                            .absolute()
                            .top_0()
                            .left(px(12.))
                            .w(px(if with_icon { 16. } else { 18. }))
                            .h(px(18.))
                            .border_l_2()
                            .border_b_2()
                            .border_color(colors.guide)
                            .rounded_bl(px(9.)),
                    )
                })
        });

        let fold = has_children.then(|| {
            let key = key.clone();
            div()
                .id("fold")
                .absolute()
                .top(px(5.))
                .left(px(-24.))
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded(px(7.))
                .cursor_pointer()
                .when(!active, |fold| {
                    fold.opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .hover(move |style| style.bg(colors.hover_strong_on_bg))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| this.toggle_row(&key, window, cx)))
                .child(icon(Icon::Expand, 16., colors.muted).with_transformation(
                    Transformation::rotate(Radians(if item.collapsed {
                        0.
                    } else {
                        std::f32::consts::FRAC_PI_2
                    })),
                ))
        });

        let bullet = {
            let key = key.clone();
            let selector_key = key.clone();
            let badge_key = key.clone();
            let ring = if threaded {
                colors.accent_soft_on_bg
            } else if active {
                colors.hover_on_bg
            } else {
                colors.bg
            };
            let mark = match glyph {
                Some(glyph) => glyph,
                None => bullet_mark(item.done, &colors).into_any_element(),
            };
            div()
                .id("bullet")
                .relative()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(if with_icon { 26. } else { 22. }))
                .rounded(px(7.))
                .when(with_icon, |bullet| {
                    bullet.debug_selector(move || format!("icon-{selector_key}"))
                })
                .when(has_children && item.collapsed, |bullet| {
                    bullet.bg(colors.accent_soft_on_bg)
                })
                .when(has_children, move |bullet| {
                    bullet
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.accent_soft_on_bg))
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when(has_children, |bullet| {
                    bullet.on_click(
                        cx.listener(move |this, _, window, cx| this.toggle_row(&key, window, cx)),
                    )
                })
                .child(mark)
                .when(with_icon && item.done, |bullet| {
                    bullet.child(
                        done_badge(ring, &colors)
                            .debug_selector(move || format!("done-badge-{badge_key}"))
                            .when(!threaded, |badge| {
                                badge.group_hover(group.clone(), |style| {
                                    style.border_color(colors.hover_on_bg)
                                })
                            }),
                    )
                })
        };

        let marker = div()
            .relative()
            .flex()
            .flex_none()
            .justify_center()
            .w(px(26.))
            .min_h(px(34.))
            .pt(px(if with_icon { 4. } else { 6. }))
            .when(guide.stem, |marker| {
                marker.child(rail().top(px(if with_icon { 30. } else { 21. })).bottom_0())
            })
            .children(fold)
            .child(bullet);

        let line: AnyElement = if editing_title {
            let selector_key = key.clone();
            div()
                .debug_selector(move || format!("title-{selector_key}"))
                .relative()
                .h(px(34.))
                .text_size(px(15.))
                .line_height(px(34.))
                .font_weight(weight)
                .text_color(if item.done { colors.muted } else { colors.text })
                .child(
                    div()
                        .absolute()
                        .inset_0()
                        .px(px(4.))
                        .flex()
                        .child(self.title_field.clone()),
                )
                .into_any_element()
        } else {
            let mut font = window.text_style().font();
            font.weight = weight;
            let mirror = Rc::new(mirror_for(
                &item.title,
                &self.query,
                &colors,
                base,
                item.done,
                true,
                &font,
            ));
            let text = StyledText::new(mirror.text.clone()).with_runs(mirror.runs.clone());
            let layout = text.layout().clone();
            let backgrounds = mirror.backgrounds.clone();
            let icons = mirror.icons.clone();
            let paint_layout = layout.clone();
            let down_layout = layout.clone();
            let down_mirror = mirror.clone();
            let up_layout = layout;
            let up_mirror = mirror.clone();
            let down_key = key.clone();
            let selector_key = key.clone();
            div()
                .id("mirror")
                .debug_selector(move || format!("title-{selector_key}"))
                .relative()
                .h(px(34.))
                .px(px(4.))
                .text_size(px(15.))
                .line_height(px(34.))
                .font_weight(weight)
                .text_color(base)
                .cursor_text()
                .child(
                    canvas(
                        |_, _, _| (),
                        move |_, (), window, cx| {
                            paint_backgrounds(&paint_layout, &backgrounds, window);
                            paint_icons(&paint_layout, &icons, colors.accent, window, cx);
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .child(div().truncate().child(text))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        let index = display_index(&down_layout, event.position);
                        if let Some(href) = down_mirror.link_at(index) {
                            this.pending_link = Some(href.clone());
                        } else {
                            let caret = caret_index(&down_layout, event.position);
                            this.pending_link = None;
                            this.click_row_title(
                                &down_key,
                                down_mirror.title_index(caret),
                                window,
                                cx,
                            );
                        }
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                        let Some(pending) = this.pending_link.take() else {
                            return;
                        };
                        let index = display_index(&up_layout, event.position);
                        if up_mirror.link_at(index) == Some(&pending)
                            && let Some(target) = link_target(&pending)
                        {
                            cx.open_url(&target);
                        }
                    }),
                )
                .into_any_element()
        };

        let title = div().flex().flex_col().flex_1().min_w_0().child(line);

        let icon_button = {
            let key = key.clone();
            let selector_key = key.clone();
            let label = if item.icon.is_some() {
                format!("Change icon of {name}")
            } else {
                format!("Set icon for {name}")
            };
            div()
                .id("icon-button")
                .debug_selector(move || format!("set-icon-{selector_key}"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .h(px(28.))
                .min_w(px(28.))
                .px(px(5.))
                .mt(px(3.))
                .rounded(px(8.))
                .cursor_pointer()
                .hover(move |style| style.bg(colors.hover_strong_on_bg))
                .when(!active, |button| {
                    button
                        .opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .tooltip(Tooltip::text(format!("{label} (Ctrl/\u{2318} \u{21e7} E)")))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_icon(&key, window, cx);
                }))
                .child(icon(Icon::AddEmoji, 17., colors.muted))
        };

        let comment_button = {
            let key = key.clone();
            let count = item.comments;
            let color = if threaded {
                colors.on_accent
            } else {
                colors.muted
            };
            let selector_key = key.clone();
            div()
                .id("comment-button")
                .debug_selector(move || format!("comments-{selector_key}"))
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .gap(px(3.))
                .h(px(28.))
                .min_w(px(28.))
                .px(px(5.))
                .mt(px(3.))
                .rounded(px(8.))
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .cursor_pointer()
                .when(threaded, |button| button.bg(colors.accent))
                .when(!threaded, |button| {
                    button.hover(move |style| style.bg(colors.hover_strong_on_bg))
                })
                .when(!active && !threaded && count == 0, |button| {
                    button
                        .opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .tooltip(Tooltip::text("Comments (\u{21e7} Enter)"))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_comments(&key, window, cx);
                }))
                .child(icon(Icon::Comment, 17., color))
                .when(count > 0, |button| button.child(count.to_string()))
        };

        let check = {
            let key = key.clone();
            let done = item.done;
            div()
                .id("check")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .mt(px(3.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(!active, |button| {
                    button
                        .opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .hover(move |style| style.bg(colors.hover_strong_on_bg))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| this.check_row(&key, window, cx)))
                .child(if done {
                    icon(Icon::Completed, 17., colors.accent)
                } else {
                    icon(Icon::Done, 17., colors.muted)
                })
        };

        div()
            .id(SharedString::from(format!("row-{key}")))
            .group(group.clone())
            .flex()
            .min_h(px(34.))
            .pl(px(28.))
            .rounded(px(9.))
            .when(threaded, |row| row.bg(colors.accent_soft_on_bg))
            .when(active && !threaded, |row| row.bg(colors.hover_on_bg))
            .when(!threaded, |row| {
                row.hover(move |style| style.bg(colors.hover_on_bg))
            })
            .children(guides)
            .child(marker)
            .child(title)
            .child(icon_button)
            .child(comment_button)
            .child(check)
            .into_any_element()
    }
}

fn display_index(layout: &TextLayout, position: gpui::Point<Pixels>) -> usize {
    let len = layout.len();
    match layout.index_for_position(position) {
        Ok(index) => index,
        Err(index) => index.min(len),
    }
}

fn caret_index(layout: &TextLayout, position: gpui::Point<Pixels>) -> usize {
    let index = display_index(layout, position);
    let next = layout
        .text()
        .get(index..)
        .and_then(|rest| rest.graphemes(true).next())
        .map_or(index, |grapheme| index + grapheme.len());
    match (
        layout.position_for_index(index),
        layout.position_for_index(next),
    ) {
        (Some(start), Some(end)) if position.x > start.x + (end.x - start.x) / 2. => next,
        _ => index,
    }
}

fn paint_icons(
    layout: &TextLayout,
    icons: &[Range<usize>],
    color: Hsla,
    window: &mut Window,
    cx: &App,
) {
    let len = layout.len();
    let line_height = layout.line_height();
    for range in icons {
        if range.end > len {
            continue;
        }
        let (Some(start), Some(end)) = (
            layout.position_for_index(range.start),
            layout.position_for_index(range.end),
        ) else {
            continue;
        };
        if (end.y - start.y).abs() > px(0.5) {
            continue;
        }
        let side = px(11.);
        let origin = point(
            start.x + (end.x - start.x - side) / 2.,
            start.y + (line_height - side) / 2.,
        );
        window
            .paint_svg(
                Bounds::new(origin, size(side, side)),
                Icon::Link.path(),
                None,
                TransformationMatrix::unit(),
                color,
                cx,
            )
            .ok();
    }
}

fn paint_backgrounds(
    layout: &TextLayout,
    backgrounds: &[(Range<usize>, Hsla, Pixels, Pixels)],
    window: &mut Window,
) {
    if backgrounds.is_empty() {
        return;
    }
    let len = layout.len();
    let line_height = layout.line_height();
    for (range, color, height, radius) in backgrounds {
        if range.start >= len {
            continue;
        }
        let Some(start) = layout.position_for_index(range.start) else {
            continue;
        };
        let Some(end) = layout.position_for_index(range.end.min(len)) else {
            continue;
        };
        if (end.y - start.y).abs() > px(0.5) {
            continue;
        }
        let top = start.y + (line_height - *height) / 2.;
        window.paint_quad(
            fill(
                gpui::Bounds::new(point(start.x, top), size(end.x - start.x, *height)),
                *color,
            )
            .corner_radii(*radius),
        );
    }
}

impl Render for OutlineView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.needs_editor_check) {
            self.detach_if_hidden(window, cx);
        }
        self.sync_thread(cx);
        let colors = theme(cx);
        let rows = self.rows();
        let lines = guides(&rows);
        let mut sheet: Vec<AnyElement> = Vec::new();
        if self.sync.failed {
            sheet.push(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .ml(px(32.))
                    .mb(px(12.))
                    .text_size(px(14.))
                    .text_color(colors.danger)
                    .child("Couldn\u{2019}t save your last change, so editing is paused.")
                    .child(
                        div()
                            .id("reload")
                            .px(px(6.))
                            .py(px(4.))
                            .text_color(colors.accent)
                            .cursor_pointer()
                            .on_click(cx.listener(|_, _, _, cx| {
                                cx.emit(OutlineEvent::ReloadRequested);
                            }))
                            .child("Reload"),
                    )
                    .into_any_element(),
            );
        }
        let current = self.current_match(&self.hits(&rows));
        for (row, guide) in rows.iter().zip(&lines) {
            let is_current = current.as_ref() == Some(&row.key);
            sheet.push(self.render_row(row, guide, is_current, window, cx));
        }
        let add = div()
            .id("add-item")
            .debug_selector(|| "add-item".into())
            .absolute()
            .right(px(16.))
            .bottom(px(16.))
            .size(px(60.))
            .rounded(px(15.))
            .bg(colors.accent)
            .shadow(shadow(&colors, false))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .occlude()
            .hover(move |style| style.bg(colors.accent_strong))
            .tooltip(Tooltip::text("Add item"))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| this.add_item(window, cx)))
            .child(icon(Icon::Add, 26., colors.on_accent));
        let page = sheet.into_iter().map(|part| {
            div()
                .w_full()
                .max_w(px(PAGE_WIDTH))
                .pr(px(10.))
                .pl(px(6.))
                .flex()
                .flex_col()
                .child(part)
        });

        let icon_picker = self.icon_picker.clone().map(|picker| {
            let backdrop = cx.entity().downgrade();
            modal(
                px(384.),
                picker.into_any_element(),
                move |window, cx| {
                    backdrop
                        .update(cx, |this, cx| this.close_icon(window, cx))
                        .ok();
                },
                window,
                cx,
            )
            .into_any_element()
        });

        let thread = self.render_thread(window, cx);
        let picker = self.picker.clone().map(|picker| {
            let backdrop = cx.entity().downgrade();
            modal(
                px(520.),
                picker.into_any_element(),
                move |window, cx| {
                    backdrop
                        .update(cx, |this, cx| this.close_move(window, cx))
                        .ok();
                },
                window,
                cx,
            )
        });

        div()
            .id("outline")
            .key_context("Outline")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .text_color(colors.text)
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_toggle_collapse))
            .on_action(cx.listener(Self::on_delete_row))
            .on_action(cx.listener(Self::on_open_move))
            .on_action(cx.listener(Self::on_open_icon))
            .on_action(cx.listener(Self::on_complete))
            .on_action(cx.listener(Self::on_open_comments))
            .on_action(cx.listener(Self::on_close_comments))
            .on_action(cx.listener(Self::on_split))
            .on_action(cx.listener(Self::on_indent))
            .on_action(cx.listener(Self::on_outdent))
            .on_action(cx.listener(Self::on_move_up))
            .on_action(cx.listener(Self::on_move_down))
            .on_action(cx.listener(Self::on_collapse))
            .on_action(cx.listener(Self::on_expand))
            .on_action(cx.listener(Self::on_prev_row))
            .on_action(cx.listener(Self::on_next_row))
            .on_action(cx.listener(Self::on_title_backspace))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        div()
                            .id("outline-scroll")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.blur_editor(window, cx)),
                            )
                            .flex()
                            .flex_col()
                            .items_center()
                            .px(px(16.))
                            .pt(px(16.))
                            .pb(px(96.))
                            .children(page),
                    )
                    .child(add),
            )
            .children(thread)
            .children(picker)
            .children(icon_picker)
    }
}

#[path = "thread_pane.rs"]
mod thread_pane;

#[cfg(test)]
#[path = "outline_view_tests.rs"]
mod tests;
