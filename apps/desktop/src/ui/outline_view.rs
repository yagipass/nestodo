use std::collections::HashSet;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, Hsla, KeyBinding,
    KeyDownEvent, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, PromptLevel, Radians,
    ScrollHandle, SharedString, StrikethroughStyle, StyledText, Subscription, Task, TextLayout,
    TextRun, Transformation, Window, actions, canvas, div, fill, point, prelude::*, px, size,
};
use unicode_segmentation::UnicodeSegmentation;

use nestodo_desktop::error::Error;
use nestodo_desktop::outline::{
    END, Focus, Key, Outline, ROOT, Row, append, caret_in, destinations, indent, key_for,
    merge_into_previous, move_by, move_to, outdent, remove, rename, set_collapsed, set_note, split,
    subtree, toggle_done, visible_rows,
};
use nestodo_desktop::sync::Sync;
use nestodo_desktop::title::{link_label, link_target, tokens};
use nestodo_desktop::todos::{MAX_NOTE_LENGTH, MAX_TITLE_LENGTH};
use nestodo_desktop::undo::{Field, UndoHistory};
use nestodo_desktop::wire::Todo;

use crate::ui::backend::Backend;
use crate::ui::clamped_text::{ClampedText, TextHits};
use crate::ui::icons::Icon;
use crate::ui::move_picker::{MovePicker, PickerEvent};
use crate::ui::text_field::{FieldEvent, TextField, offset_from_utf16, offset_to_utf16};
use crate::ui::theme::{Theme, theme};
use crate::ui::widgets::{PAGE_WIDTH, Tooltip, icon, icon_button, kbd, modal};

actions!(
    outline,
    [
        Undo,
        ToggleCollapse,
        DeleteRow,
        OpenMove,
        Complete,
        OpenNote,
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
        BackToTitle,
        NoteUp,
        NoteDown,
        NoteTab,
        CloseFilter,
        ToggleShowDone,
    ]
);

pub fn bindings() -> Vec<KeyBinding> {
    let title = Some("Title");
    let note = Some("Note");
    let outline = Some("Outline");
    let filter = Some("Filter");
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
        KeyBinding::new("cmd-enter", Complete, title),
        KeyBinding::new("ctrl-enter", Complete, title),
        KeyBinding::new("shift-enter", OpenNote, title),
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
        KeyBinding::new("cmd-z", Undo, note),
        KeyBinding::new("ctrl-z", Undo, note),
        KeyBinding::new("escape", BackToTitle, note),
        KeyBinding::new("shift-enter", BackToTitle, note),
        KeyBinding::new("cmd-shift-m", OpenMove, note),
        KeyBinding::new("ctrl-shift-m", OpenMove, note),
        KeyBinding::new("cmd-enter", Complete, note),
        KeyBinding::new("ctrl-enter", Complete, note),
        KeyBinding::new("up", NoteUp, note),
        KeyBinding::new("ctrl-p", NoteUp, note),
        KeyBinding::new("down", NoteDown, note),
        KeyBinding::new("ctrl-n", NoteDown, note),
        KeyBinding::new("tab", NoteTab, note),
        KeyBinding::new("shift-tab", NoteTab, note),
        KeyBinding::new("escape", CloseFilter, filter),
        KeyBinding::new("cmd-o", ToggleShowDone, workspace),
        KeyBinding::new("ctrl-o", ToggleShowDone, workspace),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Editing {
    key: Key,
    note: bool,
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
    filtering: bool,
    kept: HashSet<Key>,
    note_key: Option<Key>,
    moving: Option<Focus>,
    picker: Option<Entity<MovePicker>>,
    sync: Sync,
    history: UndoHistory,
    counter: u64,
    editing: Option<Editing>,
    title_field: Entity<TextField>,
    note_field: Entity<TextField>,
    filter_field: Entity<TextField>,
    focus_handle: FocusHandle,
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

fn mirror_for(
    text: &str,
    query: &str,
    colors: &Theme,
    base: Hsla,
    struck: bool,
    chips: bool,
    window: &Window,
) -> Mirror {
    let font = window.text_style().font();
    let strikethrough = struck.then_some(StrikethroughStyle {
        thickness: px(1.),
        color: Some(base),
    });
    let mut display = String::new();
    let mut runs = Vec::new();
    let mut segments = Vec::new();
    let mut backgrounds = Vec::new();
    for part in tokens(text, query) {
        let start = display.len();
        if let Some(href) = &part.href {
            let label = link_label(href);
            let shown = if chips {
                format!("{THIN}{THIN}{label}{THIN}\u{2197}{THIN}{THIN}")
            } else {
                format!("{label}\u{2197}")
            };
            display.push_str(&shown);
            let background = if part.matched {
                colors.matched
            } else {
                colors.hover
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
                backgrounds.push((start..display.len(), background, px(22.), px(4.)));
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
        note: false,
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
        let note_field = cx.new(|cx| {
            TextField::new("Note", cx)
                .multiline()
                .max_len(MAX_NOTE_LENGTH)
                .encode_urls()
        });
        let filter_field = cx.new(|cx| TextField::new("Filter", cx).placeholder("Filter"));
        let mut subscriptions = vec![
            cx.subscribe_in(&title_field, window, Self::on_title_event),
            cx.subscribe_in(&note_field, window, Self::on_note_event),
            cx.subscribe_in(&filter_field, window, Self::on_filter_event),
        ];
        let title_focus = title_field.read(cx).focus_handle(cx);
        let note_focus = note_field.read(cx).focus_handle(cx);
        let filter_focus = filter_field.read(cx).focus_handle(cx);
        subscriptions.push(cx.on_blur(&title_focus, window, |this, window, cx| {
            if window.is_window_active()
                && this.editing.as_ref().is_some_and(|editing| !editing.note)
            {
                this.leave_editor(window, cx);
                this.editing = None;
                cx.notify();
            }
        }));
        subscriptions.push(cx.on_blur(&note_focus, window, |this, window, cx| {
            if window.is_window_active()
                && this.editing.as_ref().is_some_and(|editing| editing.note)
            {
                this.leave_editor(window, cx);
                this.editing = None;
                cx.notify();
            }
        }));
        subscriptions.push(cx.on_blur(&filter_focus, window, |this, window, cx| {
            if window.is_window_active() && this.query.is_empty() && this.filtering {
                this.filtering = false;
                cx.notify();
            }
        }));
        subscriptions.push(cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.refresh(cx);
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
        Self {
            backend,
            outline,
            show_done,
            query: String::new(),
            filtering: false,
            kept: HashSet::new(),
            note_key: None,
            moving: None,
            picker: None,
            sync,
            history: UndoHistory::with_clock(move || clock.now().saturating_duration_since(start)),
            counter: 0,
            editing: None,
            title_field,
            note_field,
            filter_field,
            focus_handle: cx.focus_handle(),
            scroll: ScrollHandle::new(),
            refreshing: false,
            pending_link: None,
            flush_task: None,
            needs_editor_check: false,
            subscriptions,
        }
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
        self.picker.is_some()
    }

    fn rows(&self) -> Vec<Row> {
        visible_rows(&self.outline, self.show_done, &self.query, &self.kept)
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
        let editing = self.editing.as_ref()?;
        let field = if editing.note {
            &self.note_field
        } else {
            &self.title_field
        };
        Some(Focus {
            key: editing.key.clone(),
            pos: field.read(cx).selection().start,
            note: editing.note,
        })
    }

    fn leave_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.clone() else {
            return;
        };
        if editing.note {
            self.close_note(&editing.key, window, cx);
        } else {
            self.flush_now(cx);
        }
    }

    fn close_note(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let note = self
            .outline
            .get(key)
            .map(|item| item.note.clone())
            .unwrap_or_default();
        if !note.is_empty() && note.trim().is_empty() {
            self.on_note(key, "", false, window, cx);
        }
        self.flush_now(cx);
        if self.note_key.as_deref() == Some(key) {
            self.note_key = None;
        }
    }

    fn focus_row(&mut self, focus: Focus, window: &mut Window, cx: &mut Context<Self>) {
        let Focus { key, pos, note } = focus;
        let rows = self.rows();
        let Some(index) = rows.iter().position(|row| row.key == key) else {
            return;
        };
        if note && self.note_key.as_deref() != Some(key.as_str()) {
            return;
        }
        let target = Editing { key, note };
        if self.editing.as_ref() != Some(&target) {
            self.leave_editor(window, cx);
            if note && self.note_key.as_deref() != Some(target.key.as_str()) {
                return;
            }
        }
        let Some(item) = self.outline.get(&target.key) else {
            return;
        };
        let (field, text) = if note {
            (self.note_field.clone(), item.note.clone())
        } else {
            (self.title_field.clone(), item.title.clone())
        };
        let at = caret_in(&text, pos);
        field.update(cx, |field, cx| {
            field.set_text(&text, at, cx);
            field.set_read_only(self.sync.failed, cx);
        });
        self.editing = Some(target);
        field.read(cx).focus(window);
        self.scroll.scroll_to_item(self.row_child_offset() + index);
        cx.notify();
    }

    fn row_child_offset(&self) -> usize {
        usize::from(self.sync.failed) + usize::from(!self.query.is_empty())
    }

    fn detach_if_hidden(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(editing) = self.editing.clone() else {
            return;
        };
        if !self.is_shown(&editing.key) || !self.outline.contains(&editing.key) {
            self.editing = None;
            if self.note_key.as_deref() == Some(editing.key.as_str()) {
                self.note_key = None;
            }
            self.flush_now(cx);
            window.focus(&self.focus_handle);
            return;
        }
        let text = if editing.note {
            self.outline.item(&editing.key).note.clone()
        } else {
            self.outline.item(&editing.key).title.clone()
        };
        let field = if editing.note {
            &self.note_field
        } else {
            &self.title_field
        };
        field.update(cx, |field, cx| {
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

    fn keep(&mut self, keys: impl IntoIterator<Item = Key>) {
        if !self.query.is_empty() {
            self.kept.extend(keys);
        }
    }

    fn commit(
        &mut self,
        change: nestodo_desktop::outline::Change,
        focus: Option<Focus>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let keys: Vec<Key> = change.ops.iter().map(|op| op.key().clone()).collect();
        self.outline = change.outline;
        self.sync.apply(&change.ops);
        self.pump(cx);
        self.keep(keys);
        let next = focus.or(change.focus);
        if let Some(next) = &next
            && next.note
            && self.is_shown(&next.key)
        {
            self.note_key = Some(next.key.clone());
        }
        self.detach_if_hidden(window, cx);
        if let Some(next) = next {
            self.focus_row(next, window, cx);
        }
        cx.notify();
    }

    fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed || self.picker.is_some() {
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

    pub fn open_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.filtering = true;
        self.filter_field.read(cx).focus(window);
        cx.notify();
    }

    fn close_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.query.clear();
        self.kept.clear();
        self.filtering = false;
        self.filter_field
            .update(cx, |field, cx| field.set_text("", 0, cx));
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn complete(&mut self, key: &str, pos: usize, window: &mut Window, cx: &mut Context<Self>) {
        let rows = self.rows();
        if !rows.iter().any(|row| row.key == key) {
            return;
        }
        let before = self.focused(cx);
        let change = toggle_done(&self.outline, key);
        if self.show_done || !change.outline.item(key).done {
            let focus = Focus {
                key: key.to_string(),
                pos,
                note: false,
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
        let name = match self.outline.item(key).title.as_str() {
            "" => "Untitled".to_string(),
            title => title.to_string(),
        };
        let noun = if under == 1 { "item" } else { "items" };
        let message = format!("Delete \u{201c}{name}\u{201d} and the {under} {noun} under it?");
        let before = self.focused(cx);
        let answer = window.prompt(
            PromptLevel::Warning,
            &message,
            None,
            &["Delete", "Cancel"],
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

    fn on_title(&mut self, key: &str, title: &str, composing: bool, cx: &mut Context<Self>) {
        let next = rename(&self.outline, key, title);
        self.history
            .type_text(&self.outline, &next, key, Field::Title, composing);
        self.outline = next;
        let now = cx.background_executor().now();
        self.sync.title(key, title, now);
        self.schedule_flush(cx);
        self.keep([key.to_string()]);
        cx.notify();
    }

    fn on_note(
        &mut self,
        key: &str,
        note: &str,
        composing: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next = set_note(&self.outline, key, note);
        self.history
            .type_text(&self.outline, &next, key, Field::Note, composing);
        self.outline = next;
        let now = cx.background_executor().now();
        self.sync.note(key, note, now);
        self.schedule_flush(cx);
        self.keep([key.to_string()]);
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
                    this.sync.finish(reply.map_err(|_| ()));
                    let next = this.sync.start();
                    if next.is_none() {
                        this.check_failed(cx);
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
            self.title_field
                .update(cx, |field, cx| field.set_read_only(true, cx));
            self.note_field
                .update(cx, |field, cx| field.set_read_only(true, cx));
            cx.notify();
        }
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.refreshing || self.sync.pending() || self.sync.failed {
            return;
        }
        self.refreshing = true;
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
        let Some(editing) = self
            .editing
            .clone()
            .filter(|editing| !editing.note && self.outline.contains(&editing.key))
        else {
            return;
        };
        match event {
            FieldEvent::Changed { composing } => {
                let text = field.read(cx).text().to_string();
                if self.outline.get(&editing.key).map(|item| &item.title) != Some(&text) {
                    self.on_title(&editing.key, &text, *composing, cx);
                }
            }
            FieldEvent::CompositionEnded => self.history.composed(),
        }
    }

    fn on_note_event(
        &mut self,
        field: &Entity<TextField>,
        event: &FieldEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editing) = self
            .editing
            .clone()
            .filter(|editing| editing.note && self.outline.contains(&editing.key))
        else {
            return;
        };
        match event {
            FieldEvent::Changed { composing } => {
                let text = field.read(cx).text().to_string();
                if self.outline.get(&editing.key).map(|item| &item.note) != Some(&text) {
                    self.on_note(&editing.key, &text, *composing, window, cx);
                }
            }
            FieldEvent::CompositionEnded => self.history.composed(),
        }
    }

    fn on_filter_event(
        &mut self,
        field: &Entity<TextField>,
        event: &FieldEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let FieldEvent::Changed { .. } = event {
            self.query = field.read(cx).text().to_string();
            self.kept.clear();
            cx.notify();
        }
    }

    fn title_context(&self, cx: &App) -> Option<(Key, usize, Vec<Row>, usize)> {
        let editing = self.editing.as_ref()?;
        let rows = self.rows();
        let index = rows.iter().position(|row| row.key == editing.key)?;
        let field = if editing.note {
            &self.note_field
        } else {
            &self.title_field
        };
        let pos = field.read(cx).selection().start;
        Some((editing.key.clone(), pos, rows, index))
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
        let Some(mut here) = self.focused(cx) else {
            return;
        };
        if here.note {
            here = Focus {
                key: here.key,
                pos: END,
                note: false,
            };
        }
        self.open_move(here, window, cx);
    }

    fn on_complete(&mut self, _: &Complete, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else { return };
        let pos = if here.note { END } else { here.pos };
        self.complete(&here.key, pos, window, cx);
    }

    pub fn open_note(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.key == key && editing.note)
        {
            return;
        }
        self.leave_editor(window, cx);
        self.editing = None;
        self.note_key = Some(key.to_string());
        self.focus_row(
            Focus {
                key: key.to_string(),
                pos: END,
                note: true,
            },
            window,
            cx,
        );
    }

    fn on_open_note(&mut self, _: &OpenNote, window: &mut Window, cx: &mut Context<Self>) {
        let Some(here) = self.focused(cx) else { return };
        self.open_note(&here.key, window, cx);
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
        let here = Focus {
            key,
            pos,
            note: false,
        };
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn on_outdent(&mut self, _: &Outdent, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, _, _)) = self.title_context(cx) else {
            return;
        };
        let change = outdent(&self.outline, &key);
        let here = Focus {
            key,
            pos,
            note: false,
        };
        self.apply(change, Some(here.clone()), Some(here), window, cx);
    }

    fn move_step(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((key, pos, rows, _)) = self.title_context(cx) else {
            return;
        };
        let change = move_by(&self.outline, &key, step, &Self::shown(&rows));
        let here = Focus {
            key,
            pos,
            note: false,
        };
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
            note: false,
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
        if item.title.is_empty() && item.note.is_empty() && item.children.is_empty() {
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

    fn on_back_to_title(&mut self, _: &BackToTitle, window: &mut Window, cx: &mut Context<Self>) {
        if self.note_field.read(cx).is_composing() {
            return;
        }
        let Some(here) = self.focused(cx) else { return };
        self.focus_row(
            Focus {
                key: here.key,
                pos: END,
                note: false,
            },
            window,
            cx,
        );
    }

    fn on_note_up(&mut self, _: &NoteUp, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.note_field.read(cx);
        let caret_at_start = field.selection() == (0..0);
        let Some(here) = self.focused(cx).filter(|_| caret_at_start) else {
            cx.propagate();
            return;
        };
        self.focus_row(
            Focus {
                key: here.key,
                pos: END,
                note: false,
            },
            window,
            cx,
        );
    }

    fn on_note_down(&mut self, _: &NoteDown, window: &mut Window, cx: &mut Context<Self>) {
        let field = self.note_field.read(cx);
        let len = field.text().len();
        let caret_at_end = field.selection() == (len..len);
        let Some((_, _, rows, index)) = self.title_context(cx).filter(|_| caret_at_end) else {
            cx.propagate();
            return;
        };
        let Some(next) = rows.get(index + 1) else {
            cx.propagate();
            return;
        };
        let focus = Focus {
            key: next.key.clone(),
            pos: 0,
            note: false,
        };
        self.focus_row(focus, window, cx);
    }

    fn on_close_filter(&mut self, _: &CloseFilter, window: &mut Window, cx: &mut Context<Self>) {
        if self.filter_field.read(cx).is_composing() {
            return;
        }
        self.close_filter(window, cx);
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_handle.is_focused(window) || self.picker.is_some() {
            return;
        }
        let keystroke = &event.keystroke;
        let modifiers = keystroke.modifiers;
        if keystroke.key_char.as_deref() == Some("/")
            && !modifiers.platform
            && !modifiers.control
            && !modifiers.alt
        {
            self.open_filter(window, cx);
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
                note: false,
            },
            window,
            cx,
        );
        if self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.key == key && !editing.note)
        {
            self.title_field
                .update(cx, super::text_field::TextField::begin_mouse_selection);
        }
    }

    fn blur_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.editing.is_some() {
            self.leave_editor(window, cx);
            self.editing = None;
        }
        window.focus(&self.focus_handle);
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
        let before = self.focused(cx);
        self.blur_editor(window, cx);
        let change = toggle_done(&self.outline, key);
        self.apply(Some(change), None, before, window, cx);
    }

    fn add_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let fresh = self.fresh();
        let before = self.focused(cx);
        self.blur_editor(window, cx);
        let change = append(&self.outline, ROOT, &fresh);
        self.apply(Some(change), None, before, window, cx);
    }

    pub fn render_tools(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let filter: AnyElement = if self.filtering {
            div()
                .id("filter")
                .on_action(cx.listener(Self::on_close_filter))
                .flex()
                .flex_1()
                .items_center()
                .gap(px(6.))
                .w(px(280.))
                .min_w(px(120.))
                .h(px(32.))
                .pl(px(10.))
                .pr(px(3.))
                .border_1()
                .border_color(colors.accent)
                .rounded(px(6.))
                .bg(colors.surface)
                .child(icon(Icon::Search, 16., colors.muted))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(14.))
                        .line_height(px(20.))
                        .text_color(colors.text)
                        .child(self.filter_field.clone()),
                )
                .child(
                    div()
                        .id("close-filter")
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(24.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.hover))
                        .on_click(cx.listener(|this, _, window, cx| this.close_filter(window, cx)))
                        .child(icon(Icon::Close, 14., colors.muted)),
                )
                .into_any_element()
        } else {
            icon_button("open-filter", Icon::Search, 18., cx)
                .tooltip(Tooltip::text("Filter (/)"))
                .on_click(cx.listener(|this, _, window, cx| this.open_filter(window, cx)))
                .into_any_element()
        };
        let pressed = self.show_done;
        let show_done = div()
            .id("show-completed")
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(32.))
            .rounded(px(6.))
            .cursor_pointer()
            .when(pressed, |button| button.bg(colors.accent_soft))
            .when(!pressed, move |button| {
                button.hover(move |style| style.bg(colors.hover))
            })
            .tooltip(Tooltip::text("Show completed (Ctrl/\u{2318} O)"))
            .on_click(cx.listener(|this, _, window, cx| this.toggle_show_done(window, cx)))
            .child(icon(
                Icon::ShowCompleted,
                18.,
                if pressed { colors.accent } else { colors.muted },
            ));
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .min_w_0()
            .child(filter)
            .child(show_done)
            .into_any_element()
    }

    fn render_row(&mut self, row: &Row, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let colors = theme(cx);
        let item = self.outline.item(&row.key);
        let key = row.key.clone();
        let has_children = !item.children.is_empty();
        let editing_title = self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.key == key && !editing.note);
        let editing_note = self.note_key.as_deref() == Some(key.as_str());
        let active = self
            .editing
            .as_ref()
            .is_some_and(|editing| editing.key == key);
        let muted_text = item.done || row.under;
        let base = if muted_text {
            colors.muted
        } else {
            colors.text
        };
        let group: SharedString = format!("row-{key}").into();

        let guides = (0..row.depth).map(|level| {
            div().id(level).relative().flex_none().w(px(26.)).child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(12.))
                    .w(px(1.))
                    .bg(colors.border),
            )
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
                .rounded(px(4.))
                .cursor_pointer()
                .when(!active, |fold| {
                    fold.opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .hover(move |style| style.bg(colors.hover))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| this.toggle_row(&key, window, cx)))
                .child(icon(Icon::Chevron, 14., colors.muted).with_transformation(
                    Transformation::rotate(Radians(if item.collapsed {
                        0.
                    } else {
                        std::f32::consts::FRAC_PI_2
                    })),
                ))
        });

        let bullet = {
            let key = key.clone();
            div()
                .id("bullet")
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.))
                .rounded_full()
                .when(has_children && item.collapsed, |bullet| {
                    bullet.bg(colors.border)
                })
                .when(has_children, move |bullet| {
                    bullet
                        .cursor_pointer()
                        .hover(move |style| style.bg(colors.hover))
                })
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .when(has_children, |bullet| {
                    bullet.on_click(
                        cx.listener(move |this, _, window, cx| this.toggle_row(&key, window, cx)),
                    )
                })
                .child(div().size(px(6.)).rounded_full().bg(if item.done {
                    colors.muted
                } else {
                    colors.bullet
                }))
        };

        let marker = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(26.))
            .h(px(34.))
            .children(fold)
            .child(bullet);

        let line: AnyElement = if editing_title {
            div()
                .relative()
                .h(px(34.))
                .text_size(px(16.))
                .line_height(px(34.))
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
            let mirror = Rc::new(mirror_for(
                &item.title,
                &self.query,
                &colors,
                base,
                item.done,
                true,
                window,
            ));
            let text = StyledText::new(mirror.text.clone()).with_runs(mirror.runs.clone());
            let layout = text.layout().clone();
            let backgrounds = mirror.backgrounds.clone();
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
                .text_size(px(16.))
                .line_height(px(34.))
                .text_color(base)
                .cursor_text()
                .child(
                    canvas(
                        |_, _, _| (),
                        move |_, (), window, _| {
                            paint_backgrounds(&paint_layout, &backgrounds, window);
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

        let note: Option<AnyElement> = if editing_note {
            Some(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .mt(px(-4.))
                            .px(px(4.))
                            .pb(px(6.))
                            .text_size(px(14.))
                            .line_height(px(21.))
                            .text_color(colors.text)
                            .child(self.note_field.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(4.))
                            .px(px(4.))
                            .pb(px(8.))
                            .text_size(px(12.))
                            .text_color(colors.muted)
                            .child(kbd("Enter", cx))
                            .child("new line \u{00b7}")
                            .child(kbd("\u{21e7}", cx))
                            .child(kbd("Enter", cx))
                            .child("or")
                            .child(kbd("Esc", cx))
                            .child("back to the title"),
                    )
                    .into_any_element(),
            )
        } else if !item.note.is_empty() {
            let whole = !self.query.trim().is_empty()
                && item
                    .note
                    .to_lowercase()
                    .contains(&self.query.trim().to_lowercase());
            let mirror = Rc::new(mirror_for(
                &item.note,
                &self.query,
                &colors,
                colors.muted,
                false,
                false,
                window,
            ));
            let hits = TextHits::default();
            let text = ClampedText::new(
                mirror.text.clone(),
                mirror.runs.clone(),
                (!whole).then_some(2),
                colors.muted,
                hits.clone(),
            );
            let note_key = key.clone();
            Some(
                div()
                    .id("note")
                    .mt(px(-4.))
                    .mb(px(6.))
                    .px(px(4.))
                    .text_size(px(14.))
                    .line_height(px(21.))
                    .text_color(colors.muted)
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                        let clicked_link = hits
                            .index_for(event.position())
                            .and_then(|index| mirror.link_at(index));
                        match clicked_link {
                            Some(href) => {
                                if let Some(target) = link_target(href) {
                                    cx.open_url(&target);
                                }
                            }
                            None => this.focus_row(
                                Focus {
                                    key: note_key.clone(),
                                    pos: END,
                                    note: false,
                                },
                                window,
                                cx,
                            ),
                        }
                    }))
                    .child(text)
                    .into_any_element(),
            )
        } else {
            None
        };

        let title = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(line)
            .children(note);

        let note_button = {
            let key = key.clone();
            div()
                .id("note-button")
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(28.))
                .mt(px(3.))
                .rounded(px(6.))
                .cursor_pointer()
                .when(!active, |button| {
                    button
                        .opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .hover(move |style| style.bg(colors.hover))
                .tooltip(Tooltip::text("Note (\u{21e7} Enter)"))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| {
                    let open = this.note_key.as_deref() == Some(key.as_str());
                    if open {
                        this.focus_row(
                            Focus {
                                key: key.clone(),
                                pos: END,
                                note: false,
                            },
                            window,
                            cx,
                        );
                    } else {
                        this.open_note(&key, window, cx);
                    }
                }))
                .child(icon(Icon::Note, 16., colors.muted))
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
                .rounded(px(6.))
                .cursor_pointer()
                .when(!active && !done, |button| {
                    button
                        .opacity(0.)
                        .group_hover(group.clone(), |style| style.opacity(1.))
                })
                .hover(move |style| style.bg(colors.hover))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, window, cx| this.check_row(&key, window, cx)))
                .child(icon(
                    Icon::Check,
                    16.,
                    if done { colors.accent } else { colors.muted },
                ))
        };

        div()
            .id(SharedString::from(format!("row-{key}")))
            .group(group.clone())
            .flex()
            .min_h(px(34.))
            .pl(px(28.))
            .rounded(px(6.))
            .when(active, |row| row.bg(colors.surface))
            .hover(move |style| style.bg(colors.surface))
            .children(guides)
            .child(marker)
            .child(title)
            .child(note_button)
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
            if let Some(note_key) = self.note_key.clone()
                && !self.outline.contains(&note_key)
            {
                self.note_key = None;
            }
        }
        let colors = theme(cx);
        let rows = self.rows();
        let column = |child: AnyElement| {
            div()
                .w_full()
                .flex()
                .justify_center()
                .px(px(16.))
                .child(div().w_full().max_w(px(PAGE_WIDTH)).child(child))
        };
        let mut children: Vec<AnyElement> = Vec::new();
        if self.sync.failed {
            children.push(
                column(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .mb(px(16.))
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
                )
                .into_any_element(),
            );
        }
        if !self.query.is_empty() {
            children.push(
                column(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .ml(px(32.))
                        .mb(px(10.))
                        .text_size(px(13.))
                        .text_color(colors.muted)
                        .child("Matches are shown with their parents.")
                        .child(
                            div()
                                .id("clear-filter")
                                .px(px(6.))
                                .py(px(4.))
                                .text_color(colors.accent)
                                .cursor_pointer()
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.close_filter(window, cx);
                                }))
                                .child("Clear filter"),
                        )
                        .into_any_element(),
                )
                .into_any_element(),
            );
        }
        for row in &rows {
            let element = self.render_row(row, window, cx);
            children.push(column(element).into_any_element());
        }
        if !self.query.is_empty() && rows.is_empty() {
            children.push(
                column(
                    div()
                        .ml(px(32.))
                        .mt(px(4.))
                        .text_size(px(13.))
                        .text_color(colors.muted)
                        .child("No matches.")
                        .into_any_element(),
                )
                .into_any_element(),
            );
        }
        children.push(
            column(
                div()
                    .pb(px(24.))
                    .child(
                        div()
                            .id("add-item")
                            .mt(px(6.))
                            .ml(px(24.))
                            .px(px(8.))
                            .py(px(6.))
                            .rounded(px(6.))
                            .text_size(px(14.))
                            .text_color(colors.muted)
                            .cursor_pointer()
                            .w_auto()
                            .flex()
                            .flex_none()
                            .hover(move |style| style.bg(colors.hover).text_color(colors.text))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _, window, cx| this.add_item(window, cx)))
                            .child("+ Add item"),
                    )
                    .flex()
                    .into_any_element(),
            )
            .into_any_element(),
        );

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
            .flex_col()
            .text_color(colors.text)
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_toggle_collapse))
            .on_action(cx.listener(Self::on_delete_row))
            .on_action(cx.listener(Self::on_open_move))
            .on_action(cx.listener(Self::on_complete))
            .on_action(cx.listener(Self::on_open_note))
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
            .on_action(cx.listener(Self::on_back_to_title))
            .on_action(cx.listener(Self::on_note_up))
            .on_action(cx.listener(Self::on_note_down))
            .on_action(|_: &NoteTab, _, _| {})
            .on_key_down(cx.listener(Self::on_key_down))
            .child(
                div()
                    .id("outline-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| this.blur_editor(window, cx)),
                    )
                    .flex()
                    .flex_col()
                    .children(children),
            )
            .children(picker)
    }
}

#[cfg(test)]
#[path = "outline_view_tests.rs"]
mod tests;
