use std::time::Duration;

use chrono::{Local, Utc};
use gpui::{
    ClipboardItem, CursorStyle, DispatchPhase, Div, HitboxBehavior, MouseMoveEvent, Point,
    WeakEntity,
};

use nestodo_desktop::sync::Saved;
use nestodo_desktop::thread::{Comment, Entry, EventKind, TimelineItem, Via, by_day};

use super::*;
use crate::ui::avatar::avatar_circle;
use crate::ui::markdown_view::markdown_view;
use crate::ui::text_selection::{Selectable, Selection};
use crate::ui::widgets::{pill, text_button};

const FETCH_DELAY: Duration = Duration::from_millis(150);
const AUTOSCROLL_TICK: Duration = Duration::from_millis(16);
const PANE_WIDTH: f32 = 420.;
const QUOTE_ROOM: usize = 300;

pub(super) struct Thread {
    pub(super) key: Key,
    pub(super) items: Option<Vec<TimelineItem>>,
    pub(super) load_failed: bool,
    pub(super) local: Vec<Entry>,
    pub(super) editing: Option<u64>,
    shown_done: bool,
    fetched: Option<u64>,
    changes: u64,
    loading: Option<Task<()>>,
    scroll: ScrollHandle,
    shown_entries: usize,
    selection: Option<Selection>,
    readable: HashMap<SharedString, Rc<Selectable>>,
    pressed_link: Option<(SharedString, String)>,
    pointer: Point<Pixels>,
    autoscrolling: bool,
}

#[derive(Default)]
pub(super) struct Draft {
    text: String,
    editing: Option<(u64, String)>,
    pending: Vec<Entry>,
}

impl Draft {
    fn is_empty(&self) -> bool {
        self.text.is_empty() && self.editing.is_none() && self.pending.is_empty()
    }
}

fn is_ticket(entry: &Entry, ticket: u64) -> bool {
    matches!(entry, Entry::Pending { ticket: pending, .. } if *pending == ticket)
}

impl Thread {
    fn new(key: &str, done: bool) -> Self {
        Thread {
            key: key.to_owned(),
            items: None,
            load_failed: false,
            local: Vec::new(),
            editing: None,
            shown_done: done,
            fetched: None,
            changes: 0,
            loading: None,
            scroll: ScrollHandle::new(),
            shown_entries: 0,
            selection: None,
            readable: HashMap::new(),
            pressed_link: None,
            pointer: Point::default(),
            autoscrolling: false,
        }
    }

    pub(super) fn entries(&self) -> Vec<Entry> {
        self.items
            .iter()
            .flatten()
            .cloned()
            .map(Entry::Logged)
            .chain(self.local.iter().cloned())
            .collect()
    }

    fn comment(&self, id: u64) -> Option<&Comment> {
        self.items.iter().flatten().find_map(|item| match item {
            TimelineItem::Comment(comment) if comment.id == id => Some(comment),
            _ => None,
        })
    }

    fn comment_mut(&mut self, id: u64) -> Option<&mut Comment> {
        self.items.iter_mut().flatten().find_map(|item| match item {
            TimelineItem::Comment(comment) if comment.id == id => Some(comment),
            _ => None,
        })
    }

    fn follow_pointer(&mut self) -> bool {
        let viewport = self.scroll.bounds();
        let Some(selection) = self
            .selection
            .as_mut()
            .filter(|selection| selection.dragging)
        else {
            return false;
        };
        let Some(text) = self.readable.get(&selection.owner) else {
            return false;
        };
        let y = self
            .pointer
            .y
            .min(viewport.bottom() - px(1.))
            .max(viewport.top());
        selection.drag_to(text.offset_for(point(self.pointer.x, y)), text);
        viewport.size.height > px(0.)
            && (self.pointer.y < viewport.top() || self.pointer.y > viewport.bottom())
    }
}

fn text_overlay(owner: SharedString, view: WeakEntity<OutlineView>) -> impl IntoElement {
    canvas(
        |bounds, window, _| window.insert_hitbox(bounds, HitboxBehavior::Normal),
        move |_, hitbox, window, _| {
            window.set_cursor_style(CursorStyle::IBeam, &hitbox);
            window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                if !hitbox.is_hovered(window) {
                    if phase == DispatchPhase::Capture {
                        view.update(cx, |this, cx| this.forget_text(&owner, cx))
                            .ok();
                    }
                } else if phase == DispatchPhase::Bubble && !window.default_prevented() {
                    view.update(cx, |this, cx| this.press_text(&owner, event, window, cx))
                        .ok();
                    window.prevent_default();
                }
            });
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

fn drag_tracker(view: WeakEntity<OutlineView>) -> impl IntoElement {
    canvas(
        |_, _, _| {},
        move |_, (), window, _| {
            window.on_mouse_event({
                let view = view.clone();
                move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase == DispatchPhase::Bubble {
                        view.update(cx, |this, cx| this.drag_text(event, cx)).ok();
                    }
                }
            });
            window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
                if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                    view.update(cx, |this, cx| this.release_text(event.position, cx))
                        .ok();
                }
            });
        },
    )
    .absolute()
    .size_full()
}

fn quoted(body: &str) -> String {
    let mut chars = body.chars();
    let shown: String = chars.by_ref().take(QUOTE_ROOM).collect();
    if chars.next().is_some() {
        format!("{shown}\u{2026}")
    } else {
        shown
    }
}

fn rail(first: bool, last: bool, color: Hsla) -> Option<Div> {
    if first && last {
        return None;
    }
    let rail = div().absolute().left(px(13.)).w(px(2.)).bg(color);
    Some(match (first, last) {
        (true, _) => rail.top(px(17.)).bottom_0(),
        (_, true) => rail.top_0().h(px(17.)),
        _ => rail.top_0().bottom_0(),
    })
}

fn chip(text: &'static str, strong: bool, colors: &Theme) -> Div {
    div()
        .flex_none()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(6.))
        .font_weight(FontWeight::SEMIBOLD)
        .bg(if strong {
            colors.accent
        } else {
            colors.accent_soft
        })
        .text_color(if strong {
            colors.on_accent
        } else {
            colors.accent
        })
        .child(text)
}

impl OutlineView {
    pub fn open_comments(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.outline.contains(key) {
            return;
        }
        self.leave_editor(window, cx);
        self.editing = None;
        self.show_thread(key, cx);
        let read_only = self.sync.failed;
        let composer = self.composer.clone();
        composer.update(cx, |field, cx| {
            let text = field.text().to_string();
            field.set_text(&text, text.len(), cx);
            field.set_read_only(read_only, cx);
        });
        window.focus(&composer.focus_handle(cx), cx);
        cx.notify();
    }

    fn show_thread(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.thread.as_ref().is_some_and(|thread| thread.key == key) {
            return;
        }
        self.park_thread(cx);
        let done = self.outline.get(key).is_some_and(|item| item.done);
        let draft = self.drafts.remove(key).unwrap_or_default();
        let mut thread = Thread::new(key, done);
        thread.local = draft.pending;
        thread.editing = draft.editing.as_ref().map(|(id, _)| *id);
        self.thread = Some(thread);
        let read_only = self.sync.failed;
        let edit = draft.editing.map(|(_, text)| text).unwrap_or_default();
        for (field, text) in [(&self.composer, draft.text), (&self.comment_field, edit)] {
            field.update(cx, |field, cx| {
                field.set_text(&text, text.len(), cx);
                field.set_read_only(read_only, cx);
            });
        }
        self.fetch_thread(cx);
    }

    fn park_thread(&mut self, cx: &App) {
        let Some(thread) = self.thread.take() else {
            return;
        };
        let draft = Draft {
            text: self.composer.read(cx).text().to_string(),
            editing: thread
                .editing
                .map(|id| (id, self.comment_field.read(cx).text().to_string())),
            pending: thread
                .local
                .into_iter()
                .filter(|entry| matches!(entry, Entry::Pending { .. }))
                .collect(),
        };
        if draft.is_empty() {
            self.drafts.remove(&thread.key);
        } else {
            self.drafts.insert(thread.key, draft);
        }
    }

    pub(super) fn follow_caret(&mut self, key: &str, cx: &mut Context<Self>) {
        if self.thread.as_ref().is_some_and(|thread| thread.key != key) {
            self.show_thread(key, cx);
        }
    }

    fn close_comments(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.thread.as_ref().map(|thread| thread.key.clone()) else {
            return;
        };
        self.park_thread(cx);
        self.focus_row(Focus { key, pos: END }, window, cx);
        cx.notify();
    }

    fn back_to_item(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(key) = self.thread.as_ref().map(|thread| thread.key.clone()) else {
            return;
        };
        self.focus_row(
            Focus {
                key: key.clone(),
                pos: END,
            },
            window,
            cx,
        );
        if self.editing.as_ref() != Some(&key) {
            window.focus(&self.focus_handle, cx);
        }
    }

    pub(super) fn refresh_thread(&mut self, cx: &mut Context<Self>) {
        if self.thread.is_some() && !self.sync.pending() {
            self.fetch_thread(cx);
        }
    }

    fn fetch_thread(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(id) = self.sync.id_of(&thread.key) else {
            if thread.items.is_none() {
                thread.items = Some(Vec::new());
            }
            return;
        };
        thread.fetched = Some(id);
        let key = thread.key.clone();
        let changes = thread.changes;
        let backend = self.backend.clone();
        thread.loading = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FETCH_DELAY).await;
            let latest = cx
                .background_spawn(async move { backend.timeline(id) })
                .await;
            this.update(cx, |this, cx| this.thread_loaded(&key, changes, latest, cx))
                .ok();
        }));
    }

    fn thread_loaded(
        &mut self,
        key: &str,
        changes: u64,
        latest: Result<Vec<TimelineItem>, Error>,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.thread.as_ref().filter(|thread| thread.key == key) else {
            return;
        };
        if thread.changes != changes {
            self.fetch_thread(cx);
            return;
        }
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        match latest {
            Ok(items) => {
                thread.items = Some(items);
                thread.load_failed = false;
                thread
                    .local
                    .retain(|entry| matches!(entry, Entry::Pending { .. }));
            }
            Err(_) => thread.load_failed = true,
        }
        cx.notify();
    }

    pub(super) fn sync_thread(&mut self, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_ref() else {
            return;
        };
        let Some(done) = self.outline.get(&thread.key).map(|item| item.done) else {
            self.thread = None;
            return;
        };
        let id = self.sync.id_of(&thread.key);
        let unfetched = id.is_some() && id != thread.fetched && !self.sync.pending();
        if let Some(thread) = self.thread.as_mut()
            && thread.shown_done != done
        {
            thread.shown_done = done;
            thread.local.push(Entry::Logged(TimelineItem::Event {
                kind: if done {
                    EventKind::Completed
                } else {
                    EventKind::Reopened
                },
                at: Utc::now(),
            }));
        }
        if unfetched {
            self.fetch_thread(cx);
        }
    }

    fn count_comments(&mut self, key: &str, delta: i64) {
        let Some(count) = self.outline.get(key).map(|item| item.comments) else {
            return;
        };
        self.outline = set_comments(&self.outline, key, count.saturating_add_signed(delta));
    }

    pub(super) fn post_comment(&mut self, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        let composer = self.composer.read(cx);
        if composer.is_composing() {
            return;
        }
        let body = composer.text().trim().to_string();
        let key = self
            .thread
            .as_ref()
            .map(|thread| thread.key.clone())
            .filter(|key| self.outline.contains(key));
        let Some(key) = key.filter(|_| !body.is_empty()) else {
            return;
        };
        let ticket = self.sync.comment(&key, &body);
        if let Some(thread) = self.thread.as_mut() {
            thread.local.push(Entry::Pending {
                ticket,
                body,
                at: Utc::now(),
            });
            thread.changes += 1;
        }
        self.composer
            .update(cx, |field, cx| field.set_text("", 0, cx));
        self.count_comments(&key, 1);
        self.pump(cx);
        cx.notify();
    }

    pub(super) fn comment_saved(&mut self, saved: Saved, cx: &mut Context<Self>) {
        match saved {
            Saved::Posted {
                key,
                ticket,
                comment,
            } => {
                let Some(thread) = self.thread.as_mut().filter(|thread| thread.key == key) else {
                    if let Some(draft) = self.drafts.get_mut(&key) {
                        draft.pending.retain(|entry| !is_ticket(entry, ticket));
                        if draft.is_empty() {
                            self.drafts.remove(&key);
                        }
                    }
                    return;
                };
                let before = thread.local.len();
                thread.local.retain(|entry| !is_ticket(entry, ticket));
                if thread.local.len() == before {
                    return;
                }
                let items = thread.items.get_or_insert_with(Vec::new);
                let known = items.iter().any(
                    |item| matches!(item, TimelineItem::Comment(known) if known.id == comment.id),
                );
                if !known {
                    items.push(TimelineItem::Comment(comment));
                }
            }
            Saved::Edited(comment) => {
                if let Some(known) = self
                    .thread
                    .as_mut()
                    .and_then(|thread| thread.comment_mut(comment.id))
                {
                    *known = comment;
                }
            }
        }
        cx.notify();
    }

    fn start_edit(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(body) = thread.comment(id).map(|comment| comment.body.clone()) else {
            return;
        };
        thread.editing = Some(id);
        let field = self.comment_field.clone();
        field.update(cx, |field, cx| field.set_text(&body, body.len(), cx));
        window.focus(&field.focus_handle(cx), cx);
        cx.notify();
    }

    fn save_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        let field = self.comment_field.read(cx);
        if field.is_composing() {
            return;
        }
        let body = field.text().trim().to_string();
        if body.is_empty() {
            return;
        }
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(id) = thread.editing.take() else {
            return;
        };
        if let Some(comment) = thread.comment_mut(id) {
            comment.body.clone_from(&body);
        }
        thread.changes += 1;
        self.sync.edit_comment(id, &body);
        self.pump(cx);
        window.focus(&self.composer.focus_handle(cx), cx);
        cx.notify();
    }

    fn cancel_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        if thread.editing.take().is_some() {
            window.focus(&self.composer.focus_handle(cx), cx);
            cx.notify();
        }
    }

    fn delete_comment(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        let Some(body) = self
            .thread
            .as_ref()
            .and_then(|thread| thread.comment(id))
            .map(|comment| quoted(&comment.body))
        else {
            return;
        };
        let answer = window.prompt(
            PromptLevel::Warning,
            "Delete this comment?",
            Some(&body),
            &["Delete", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            if answer.await.ok() == Some(0) {
                this.update(cx, |this, cx| this.delete_confirmed(id, cx))
                    .ok();
            }
        })
        .detach();
    }

    fn delete_confirmed(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.sync.failed {
            return;
        }
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(items) = thread.items.as_mut() else {
            return;
        };
        let before = items.len();
        items.retain(|item| !matches!(item, TimelineItem::Comment(comment) if comment.id == id));
        if items.len() == before {
            return;
        }
        thread.changes += 1;
        if thread.editing == Some(id) {
            thread.editing = None;
        }
        let key = thread.key.clone();
        self.sync.delete_comment(id);
        self.count_comments(&key, -1);
        self.pump(cx);
        cx.notify();
    }

    fn path_of(&self, key: &str) -> Vec<String> {
        let mut path = Vec::new();
        let mut current = parent_of(&self.outline, key);
        while let Some(parent) = current.filter(|parent| parent != ROOT) {
            path.insert(0, self.name_of(&parent));
            current = parent_of(&self.outline, &parent);
        }
        path
    }

    pub(super) fn on_open_comments(
        &mut self,
        _: &OpenComments,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.title_field.read(cx).is_composing() {
            return;
        }
        let Some(here) = self.focused(cx) else { return };
        self.open_comments(&here.key, window, cx);
    }

    pub(super) fn on_close_comments(
        &mut self,
        _: &CloseComments,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.thread.is_none() {
            cx.propagate();
            return;
        }
        if self.title_field.read(cx).is_composing() {
            return;
        }
        self.park_thread(cx);
        cx.notify();
    }

    fn on_post_comment(&mut self, _: &PostComment, _: &mut Window, cx: &mut Context<Self>) {
        self.post_comment(cx);
    }

    fn on_back_to_item(&mut self, _: &BackToItem, window: &mut Window, cx: &mut Context<Self>) {
        if self.composer.read(cx).is_composing() {
            return;
        }
        self.back_to_item(window, cx);
    }

    fn on_save_comment(&mut self, _: &SaveComment, window: &mut Window, cx: &mut Context<Self>) {
        self.save_comment(window, cx);
    }

    fn on_cancel_edit(&mut self, _: &CancelEdit, window: &mut Window, cx: &mut Context<Self>) {
        if self.comment_field.read(cx).is_composing() {
            return;
        }
        self.cancel_edit(window, cx);
    }

    fn comment_text(
        &self,
        id: &str,
        body: &str,
        font: &Font,
        readable: &mut HashMap<SharedString, Rc<Selectable>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let owner = SharedString::from(id.to_owned());
        let selected = self
            .thread
            .as_ref()
            .and_then(|thread| thread.selection.as_ref())
            .filter(|selection| selection.owner == owner)
            .map(Selection::range);
        let (element, text) = markdown_view(id, body, &self.query, font, &theme(cx), selected);
        readable.insert(owner.clone(), Rc::new(text));
        div()
            .relative()
            .child(element)
            .child(text_overlay(owner, cx.entity().downgrade()))
            .into_any_element()
    }

    fn press_text(
        &mut self,
        owner: &SharedString,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(text) = thread.readable.get(owner).cloned() else {
            return;
        };
        let offset = text.offset_for(event.position);
        let clicks = event.click_count;
        let extending = event.modifiers.shift
            && clicks <= 1
            && thread
                .selection
                .as_ref()
                .is_some_and(|selection| selection.owner == *owner);
        thread.pressed_link = None;
        if extending {
            if let Some(selection) = thread.selection.as_mut() {
                selection.extend_to(offset);
            }
        } else {
            let mut selection = Selection::press(owner.clone(), offset, clicks, &text);
            if clicks <= 1
                && let Some(href) = text.link_at(event.position)
            {
                thread.pressed_link = Some((owner.clone(), href.to_owned()));
                selection.dragging = false;
            }
            thread.selection = Some(selection);
        }
        thread.pointer = event.position;
        window.focus(&self.log_focus, cx);
        cx.notify();
    }

    fn forget_text(&mut self, owner: &SharedString, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        if thread
            .selection
            .as_ref()
            .is_some_and(|selection| selection.owner == *owner)
        {
            thread.selection = None;
            cx.notify();
        }
    }

    fn drag_text(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(selection) = thread
            .selection
            .as_mut()
            .filter(|selection| selection.dragging)
        else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            selection.dragging = false;
            cx.notify();
            return;
        }
        thread.pointer = event.position;
        let outside = thread.follow_pointer();
        if outside && !thread.autoscrolling {
            thread.autoscrolling = true;
            cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor().timer(AUTOSCROLL_TICK).await;
                    let going = this.update(cx, Self::autoscroll).unwrap_or(false);
                    if !going {
                        break;
                    }
                }
            })
            .detach();
        }
        cx.notify();
    }

    fn autoscroll(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(thread) = self.thread.as_mut() else {
            return false;
        };
        if !thread.follow_pointer() {
            thread.autoscrolling = false;
            return false;
        }
        let viewport = thread.scroll.bounds();
        let beyond = if thread.pointer.y < viewport.top() {
            thread.pointer.y - viewport.top()
        } else {
            thread.pointer.y - viewport.bottom()
        };
        let step = (beyond.abs() / 2.).max(px(4.)).min(px(40.));
        let offset = thread.scroll.offset();
        let y = if beyond < px(0.) {
            offset.y + step
        } else {
            offset.y - step
        };
        let lowest = -thread.scroll.max_offset().y;
        thread
            .scroll
            .set_offset(point(offset.x, y.max(lowest).min(px(0.))));
        cx.notify();
        true
    }

    fn release_text(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        if let Some((owner, href)) = thread.pressed_link.take() {
            let released_on_it = thread
                .readable
                .get(&owner)
                .and_then(|text| text.link_at(position))
                == Some(href.as_str());
            if released_on_it && let Some(target) = link_target(&href) {
                cx.open_url(&target);
            }
            return;
        }
        if let Some(selection) = thread
            .selection
            .as_mut()
            .filter(|selection| selection.dragging)
        {
            selection.dragging = false;
            cx.notify();
        }
    }

    fn on_copy_text(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_ref() else {
            return;
        };
        let Some(selection) = thread.selection.as_ref() else {
            return;
        };
        let range = selection.range();
        let Some(text) = thread.readable.get(&selection.owner) else {
            return;
        };
        if range.is_empty() {
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.text_in(range).to_owned()));
    }

    fn on_select_all_text(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let Some(thread) = self.thread.as_mut() else {
            return;
        };
        let Some(owner) = thread
            .selection
            .as_ref()
            .map(|selection| selection.owner.clone())
        else {
            return;
        };
        let Some(text) = thread.readable.get(&owner) else {
            return;
        };
        thread.selection = Some(Selection::whole(owner, text));
        cx.notify();
    }

    fn render_event(kind: EventKind, time: &str, rail: Option<Div>, colors: &Theme) -> AnyElement {
        let (background, glyph, color) = match kind {
            EventKind::Created => (colors.accent_soft, Icon::Add, colors.accent),
            EventKind::Completed => (colors.accent, Icon::Tick, colors.on_accent),
            EventKind::Reopened => (colors.surface, Icon::Reopen, colors.accent),
        };
        div()
            .relative()
            .flex()
            .items_center()
            .gap(px(12.))
            .min_h(px(34.))
            .text_size(px(13.))
            .text_color(colors.muted)
            .children(rail)
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(22.))
                    .mx(px(3.))
                    .rounded_full()
                    .bg(background)
                    .when(kind == EventKind::Reopened, |node| {
                        node.border_2().border_color(colors.guide)
                    })
                    .child(icon(glyph, 12., color)),
            )
            .child(
                div()
                    .flex()
                    .gap(px(4.))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(colors.text)
                            .child(kind.label()),
                    )
                    .child("at")
                    .child(time.to_string()),
            )
            .into_any_element()
    }

    fn render_comment(
        &self,
        comment: &Comment,
        time: &str,
        rail: Option<Div>,
        font: &Font,
        readable: &mut HashMap<SharedString, Rc<Selectable>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(cx);
        let id = comment.id;
        let editable = !self.sync.failed;
        let editing = self
            .thread
            .as_ref()
            .is_some_and(|thread| thread.editing == Some(id));
        let group: SharedString = format!("comment-{id}").into();
        let mcp = comment.via == Via::Mcp;
        let avatar = if mcp {
            div()
                .size(px(28.))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .rounded_full()
                .bg(colors.surface)
                .border_1()
                .border_color(colors.guide)
                .child(icon(Icon::Bot, 16., colors.accent))
                .into_any_element()
        } else {
            avatar_circle(self.avatar.as_ref(), 28., &colors)
        };
        let actions = (editable && !editing).then(|| {
            div()
                .flex()
                .gap(px(2.))
                .ml_auto()
                .opacity(0.)
                .group_hover(group.clone(), |style| style.opacity(1.))
                .child(
                    icon_button(
                        SharedString::from(format!("edit-comment-{id}")),
                        Icon::Edit,
                        15.,
                        colors.hover,
                        cx,
                    )
                    .debug_selector(move || format!("edit-comment-{id}"))
                    .size(px(28.))
                    .tooltip(Tooltip::text("Edit"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_edit(id, window, cx);
                    })),
                )
                .child(
                    icon_button(
                        SharedString::from(format!("delete-comment-{id}")),
                        Icon::Trash,
                        15.,
                        colors.hover,
                        cx,
                    )
                    .debug_selector(move || format!("delete-comment-{id}"))
                    .size(px(28.))
                    .tooltip(Tooltip::text("Delete"))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.delete_comment(id, window, cx);
                    })),
                )
        });
        let meta = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .min_h(px(28.))
            .text_size(px(12.))
            .text_color(colors.muted)
            .when(mcp, |meta| {
                meta.child(
                    chip("via MCP", false, &colors).debug_selector(move || format!("via-mcp-{id}")),
                )
            })
            .child(time.to_string())
            .when(comment.edited_at.is_some(), |meta| meta.child("(edited)"))
            .children(actions);
        let body = if editing {
            let can_save = !self.comment_field.read(cx).text().trim().is_empty();
            let field = self.comment_field.clone();
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .mt(px(2.))
                .child(
                    div()
                        .debug_selector(|| "comment-editor".into())
                        .px(px(12.))
                        .py(px(10.))
                        .border_1()
                        .border_color(colors.accent)
                        .rounded(px(12.))
                        .bg(colors.surface)
                        .min_h(px(66.))
                        .text_size(px(14.5))
                        .line_height(px(22.))
                        .text_color(colors.text)
                        .cursor_text()
                        .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                            window.focus(&field.focus_handle(cx), cx);
                            window.prevent_default();
                        })
                        .child(self.comment_field.clone()),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            text_button("cancel-comment-edit", "Cancel", false, true, &colors)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.cancel_edit(window, cx);
                                })),
                        )
                        .child(pill("save-comment", "Save", can_save, &colors).when(
                            can_save,
                            |pill| {
                                pill.on_click(cx.listener(|this, _, window, cx| {
                                    this.save_comment(window, cx);
                                }))
                            },
                        )),
                )
                .into_any_element()
        } else {
            self.comment_text(
                &format!("comment-text-{id}"),
                &comment.body,
                font,
                readable,
                cx,
            )
        };
        div()
            .id(SharedString::from(format!("comment-{id}")))
            .group(group)
            .relative()
            .flex()
            .gap(px(12.))
            .children(rail)
            .child(div().mt(px(4.)).child(avatar))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pt(px(4.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .child(meta)
                    .child(body),
            )
            .into_any_element()
    }

    fn render_pending(
        &self,
        ticket: u64,
        body: &str,
        rail: Option<Div>,
        font: &Font,
        readable: &mut HashMap<SharedString, Rc<Selectable>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let colors = theme(cx);
        div()
            .id(SharedString::from(format!("pending-{ticket}")))
            .relative()
            .flex()
            .gap(px(12.))
            .opacity(0.6)
            .children(rail)
            .child(
                div()
                    .mt(px(4.))
                    .child(avatar_circle(self.avatar.as_ref(), 28., &colors)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pt(px(4.))
                    .pb(px(14.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .min_h(px(28.))
                            .text_size(px(12.))
                            .text_color(colors.muted)
                            .child("Sending\u{2026}"),
                    )
                    .child(self.comment_text(
                        &format!("pending-text-{ticket}"),
                        body,
                        font,
                        readable,
                        cx,
                    )),
            )
            .into_any_element()
    }

    pub(super) fn render_thread(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let colors = theme(cx);
        let thread = self.thread.as_mut()?;
        let item = self.outline.get(&thread.key)?.clone();
        let key = thread.key.clone();
        let entries = thread.entries();
        if thread.shown_entries != entries.len() {
            thread.shown_entries = entries.len();
            thread.scroll.scroll_to_bottom();
        }
        let scroll = thread.scroll.clone();
        let loaded = thread.items.is_some();
        let load_failed = thread.load_failed && !loaded;
        let editable = !self.sync.failed;
        let font = window.text_style().font();
        let path = self.path_of(&key);
        let title = if item.title.is_empty() {
            "Untitled".to_owned()
        } else {
            item.title.clone()
        };
        let count = match item.comments {
            0 => "No comments yet".to_owned(),
            1 => "1 comment".to_owned(),
            n => format!("{n} comments"),
        };
        let written = entries.iter().any(Entry::is_written);

        let header = div()
            .flex()
            .items_start()
            .gap(px(8.))
            .pt(px(18.))
            .pr(px(14.))
            .pb(px(14.))
            .pl(px(20.))
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .when(!path.is_empty(), |heading| {
                        heading.child(
                            div()
                                .mb(px(4.))
                                .text_size(px(12.))
                                .text_color(colors.muted)
                                .child(path.join(" \u{203a} ")),
                        )
                    })
                    .child(
                        div()
                            .debug_selector(|| "thread-title".into())
                            .text_size(px(17.))
                            .line_height(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(title),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .mt(px(8.))
                            .text_size(px(12.))
                            .text_color(colors.muted)
                            .child(if item.done {
                                chip("Completed", true, &colors)
                            } else {
                                chip("Open", false, &colors)
                            })
                            .child(count),
                    ),
            )
            .child(
                icon_button("close-comments", Icon::Close, 18., colors.hover, cx)
                    .debug_selector(|| "close-comments".into())
                    .tooltip(Tooltip::text("Close (Esc)"))
                    .on_click(cx.listener(|this, _, window, cx| this.close_comments(window, cx))),
            );

        let mut log: Vec<AnyElement> = Vec::new();
        let mut readable = HashMap::new();
        if load_failed {
            log.push(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(8.))
                    .ml(px(40.))
                    .mt(px(10.))
                    .text_size(px(13.))
                    .text_color(colors.muted)
                    .child("Couldn\u{2019}t load the comments.")
                    .child(
                        text_button("retry-comments", "Try again", false, true, &colors)
                            .debug_selector(|| "retry-comments".into())
                            .on_click(cx.listener(|this, _, _, cx| this.fetch_thread(cx))),
                    )
                    .into_any_element(),
            );
        }
        for day in by_day(&entries, Utc::now(), &Local) {
            let mut shown: Vec<AnyElement> = vec![
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .mt(px(16.))
                    .mb(px(8.))
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.muted)
                    .child(div().flex_1().h(px(1.)).bg(colors.border))
                    .child(day.label)
                    .child(div().flex_1().h(px(1.)).bg(colors.border))
                    .into_any_element(),
            ];
            let last = day.entries.len().saturating_sub(1);
            for (index, (entry, time)) in day.entries.iter().enumerate() {
                let rail = rail(index == 0, index == last, colors.accent_soft);
                shown.push(match entry {
                    Entry::Logged(TimelineItem::Event { kind, .. }) => {
                        Self::render_event(*kind, time, rail, &colors)
                    }
                    Entry::Logged(TimelineItem::Comment(comment)) => {
                        self.render_comment(comment, time, rail, &font, &mut readable, cx)
                    }
                    Entry::Pending { ticket, body, .. } => {
                        self.render_pending(*ticket, body, rail, &font, &mut readable, cx)
                    }
                });
            }
            log.push(div().flex().flex_col().children(shown).into_any_element());
        }
        if loaded && !written {
            log.push(
                div()
                    .ml(px(40.))
                    .mt(px(10.))
                    .text_size(px(13.))
                    .text_color(colors.muted)
                    .child("Nothing written yet. Note what you did or found.")
                    .into_any_element(),
            );
        }

        if let Some(thread) = self.thread.as_mut() {
            thread.selection = thread.selection.take().and_then(|mut selection| {
                selection.fit(readable.get(&selection.owner)?.len());
                Some(selection)
            });
            thread.readable = readable;
        }

        let composer_focused = self.composer.focus_handle(cx).is_focused(window);
        let can_post = editable && !self.composer.read(cx).text().trim().is_empty();
        let composer = self.composer.clone();
        let compose = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .px(px(16.))
            .pt(px(12.))
            .pb(px(16.))
            .border_t_1()
            .border_color(colors.border)
            .child(
                div()
                    .debug_selector(|| "composer".into())
                    .px(px(12.))
                    .py(px(10.))
                    .border_1()
                    .border_color(if composer_focused {
                        colors.accent
                    } else {
                        colors.border
                    })
                    .rounded(px(12.))
                    .bg(colors.surface)
                    .min_h(px(66.))
                    .text_size(px(14.5))
                    .line_height(px(22.))
                    .text_color(colors.text)
                    .cursor_text()
                    .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                        window.focus(&composer.focus_handle(cx), cx);
                        window.prevent_default();
                    })
                    .child(self.composer.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap(px(4.))
                            .text_size(px(12.))
                            .text_color(colors.muted)
                            .child(kbd("Enter", cx))
                            .child("to post,")
                            .child(kbd("Esc", cx))
                            .child("back to the item"),
                    )
                    .child(
                        pill("post-comment", "Post", can_post, &colors).when(can_post, |pill| {
                            pill.on_click(cx.listener(|this, _, _, cx| this.post_comment(cx)))
                        }),
                    ),
            );

        let width = px(PANE_WIDTH).min(window.viewport_size().width / 2.);
        Some(
            div()
                .id("thread")
                .key_context("Thread")
                .debug_selector(|| "thread".into())
                .flex()
                .flex_col()
                .flex_none()
                .w(width)
                .h_full()
                .border_l_1()
                .border_color(colors.border)
                .bg(colors.surface)
                .on_action(cx.listener(Self::on_post_comment))
                .on_action(cx.listener(Self::on_back_to_item))
                .on_action(cx.listener(Self::on_save_comment))
                .on_action(cx.listener(Self::on_cancel_edit))
                .child(header)
                .child(
                    div()
                        .id("thread-log")
                        .debug_selector(|| "thread-log".into())
                        .key_context("ThreadLog")
                        .track_focus(&self.log_focus)
                        .on_action(cx.listener(Self::on_copy_text))
                        .on_action(cx.listener(Self::on_select_all_text))
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .restrict_scroll_to_axis()
                        .track_scroll(&scroll)
                        .px(px(16.))
                        .pb(px(12.))
                        .flex()
                        .flex_col()
                        .children(log),
                )
                .child(compose)
                .child(drag_tracker(cx.entity().downgrade()))
                .into_any_element(),
        )
    }
}
