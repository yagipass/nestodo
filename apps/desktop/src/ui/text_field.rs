use std::ops::Range;
use std::rc::Rc;
use std::time::Duration;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, Hsla, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    SharedString, Style, Task, TextRun, UTF16Selection, UnderlineStyle, Window, WrappedLine,
    actions, div, fill, point, prelude::*, px, size,
};
use unicode_segmentation::UnicodeSegmentation;

use nestodo_desktop::outline::caret_in;
use nestodo_desktop::title::encode_pasted_url;
use nestodo_desktop::todos::utf16_len;

use crate::ui::theme::theme;

actions!(
    text_field,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        DeleteToLineStart,
        DeleteToLineEnd,
        Left,
        Right,
        Up,
        Down,
        WordLeft,
        WordRight,
        LineStart,
        LineEnd,
        DocStart,
        DocEnd,
        SelectLeft,
        SelectRight,
        SelectUp,
        SelectDown,
        SelectWordLeft,
        SelectWordRight,
        SelectLineStart,
        SelectLineEnd,
        SelectDocStart,
        SelectDocEnd,
        SelectAll,
        Newline,
        Copy,
        Cut,
        Paste,
        ShowCharacterPalette,
    ]
);

pub const CONTEXT: &str = "TextField";

const BULLET: char = '\u{2022}';

pub fn bindings() -> Vec<KeyBinding> {
    let c = Some(CONTEXT);
    let multi = Some("TextField && multiline");
    vec![
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("shift-backspace", Backspace, c),
        KeyBinding::new("ctrl-h", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("ctrl-d", Delete, c),
        KeyBinding::new("alt-backspace", DeleteWordLeft, c),
        KeyBinding::new("ctrl-backspace", DeleteWordLeft, c),
        KeyBinding::new("alt-delete", DeleteWordRight, c),
        KeyBinding::new("cmd-backspace", DeleteToLineStart, c),
        KeyBinding::new("ctrl-k", DeleteToLineEnd, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("ctrl-b", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("ctrl-f", Right, c),
        KeyBinding::new("alt-left", WordLeft, c),
        KeyBinding::new("alt-right", WordRight, c),
        KeyBinding::new("cmd-left", LineStart, c),
        KeyBinding::new("ctrl-a", LineStart, c),
        KeyBinding::new("home", LineStart, c),
        KeyBinding::new("cmd-right", LineEnd, c),
        KeyBinding::new("ctrl-e", LineEnd, c),
        KeyBinding::new("end", LineEnd, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("ctrl-p", Up, multi),
        KeyBinding::new("ctrl-n", Down, multi),
        KeyBinding::new("cmd-up", DocStart, c),
        KeyBinding::new("cmd-down", DocEnd, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("alt-shift-left", SelectWordLeft, c),
        KeyBinding::new("alt-shift-right", SelectWordRight, c),
        KeyBinding::new("cmd-shift-left", SelectLineStart, c),
        KeyBinding::new("shift-home", SelectLineStart, c),
        KeyBinding::new("cmd-shift-right", SelectLineEnd, c),
        KeyBinding::new("shift-end", SelectLineEnd, c),
        KeyBinding::new("cmd-shift-up", SelectDocStart, c),
        KeyBinding::new("cmd-shift-down", SelectDocEnd, c),
        KeyBinding::new("cmd-a", SelectAll, c),
        KeyBinding::new("enter", Newline, multi),
        KeyBinding::new("cmd-c", Copy, c),
        KeyBinding::new("cmd-x", Cut, c),
        KeyBinding::new("cmd-v", Paste, c),
        KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, c),
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldEvent {
    Changed { composing: bool },
    CompositionEnded,
}

#[derive(Clone)]
struct Paragraph {
    start: usize,
    line: Rc<WrappedLine>,
    top: Pixels,
}

#[derive(Clone)]
struct FieldLayout {
    text: String,
    masked: bool,
    paragraphs: Vec<Paragraph>,
    origin: Point<Pixels>,
    bounds: Bounds<Pixels>,
    line_height: Pixels,
}

fn mask(text: &str) -> String {
    text.chars().map(|_| BULLET).collect()
}

fn shown_offset(text: &str, masked: bool, index: usize) -> usize {
    match text.get(..index) {
        Some(before) if masked => before.chars().count() * BULLET.len_utf8(),
        _ => index,
    }
}

impl FieldLayout {
    fn shown(&self, index: usize) -> usize {
        shown_offset(&self.text, self.masked, index)
    }

    fn actual(&self, shown: usize) -> usize {
        if self.masked {
            self.text
                .char_indices()
                .nth(shown / BULLET.len_utf8())
                .map_or(self.text.len(), |(index, _)| index)
        } else {
            shown
        }
    }

    fn position_for(&self, index: usize) -> Point<Pixels> {
        let index = self.shown(index);
        let paragraph = self.paragraph_at(index);
        let local = index - paragraph.start;
        let at = paragraph
            .line
            .position_for_index(local, self.line_height)
            .unwrap_or_default();
        point(self.origin.x + at.x, self.origin.y + paragraph.top + at.y)
    }

    fn paragraph_at(&self, index: usize) -> &Paragraph {
        self.paragraphs
            .iter()
            .rev()
            .find(|paragraph| paragraph.start <= index)
            .unwrap_or(&self.paragraphs[0])
    }

    fn index_for(&self, position: Point<Pixels>) -> usize {
        let y = position.y - self.origin.y;
        let paragraph = self
            .paragraphs
            .iter()
            .rev()
            .find(|paragraph| paragraph.top <= y)
            .unwrap_or(&self.paragraphs[0]);
        let height = paragraph.line.size(self.line_height).height;
        let local_y = (y - paragraph.top).clamp(px(0.), height - px(1.));
        let local = point(position.x - self.origin.x, local_y);
        let index = match paragraph
            .line
            .closest_index_for_position(local, self.line_height)
        {
            Ok(index) | Err(index) => index,
        };
        self.actual(paragraph.start + index)
    }

    fn visual_line_starts(paragraph: &Paragraph) -> Vec<usize> {
        let mut starts = vec![paragraph.start];
        for boundary in paragraph.line.wrap_boundaries() {
            let glyph = &paragraph.line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix];
            starts.push(paragraph.start + glyph.index);
        }
        starts
    }

    fn line_range_at(&self, index: usize) -> Range<usize> {
        let paragraph = self.paragraph_at(index);
        let starts = Self::visual_line_starts(paragraph);
        let end_of_paragraph = paragraph.start + paragraph.line.len();
        let line = starts
            .iter()
            .rposition(|start| *start <= index)
            .unwrap_or(0);
        let start = starts[line];
        let end = starts.get(line + 1).copied().unwrap_or(end_of_paragraph);
        start..end
    }
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "the field's options and its caret state are independent switches"
)]
pub struct TextField {
    focus_handle: FocusHandle,
    content: String,
    placeholder: SharedString,
    context: SharedString,
    multiline: bool,
    masked: bool,
    max_len: Option<usize>,
    read_only: bool,
    encode_urls: bool,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    goal_x: Option<Pixels>,
    scroll_x: Pixels,
    layout: Option<FieldLayout>,
    selecting: bool,
    cursor_visible: bool,
    focused: bool,
    blink_task: Option<Task<()>>,
}

impl EventEmitter<FieldEvent> for TextField {}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl TextField {
    pub fn new(context: &str, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: String::new(),
            placeholder: SharedString::default(),
            context: SharedString::from(format!("{CONTEXT} {context}")),
            multiline: false,
            masked: false,
            max_len: None,
            read_only: false,
            encode_urls: false,
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            goal_x: None,
            scroll_x: px(0.),
            layout: None,
            selecting: false,
            cursor_visible: true,
            focused: false,
            blink_task: None,
        }
    }

    pub fn multiline(mut self) -> Self {
        self.multiline = true;
        self.context = SharedString::from(format!("{} multiline", self.context));
        self
    }

    pub fn set_masked(&mut self, masked: bool, cx: &mut Context<Self>) {
        if self.multiline || self.masked == masked {
            return;
        }
        self.masked = masked;
        self.layout = None;
        cx.notify();
    }

    #[cfg(test)]
    pub fn placeholder_text(&self) -> &str {
        &self.placeholder
    }

    #[cfg(test)]
    pub fn is_masked(&self) -> bool {
        self.masked
    }

    pub fn max_len(mut self, max: usize) -> Self {
        self.max_len = Some(max);
        self
    }

    pub fn encode_urls(mut self) -> Self {
        self.encode_urls = true;
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn set_placeholder(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        let text = text.into();
        if self.placeholder != text {
            self.placeholder = text;
            cx.notify();
        }
    }

    pub fn set_read_only(&mut self, read_only: bool, cx: &mut Context<Self>) {
        if self.read_only != read_only {
            self.read_only = read_only;
            cx.notify();
        }
    }

    pub fn text(&self) -> &str {
        &self.content
    }

    pub fn cursor(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    pub fn selection(&self) -> Range<usize> {
        self.selected_range.clone()
    }

    pub fn is_composing(&self) -> bool {
        self.marked_range.is_some()
    }

    pub fn set_text(&mut self, text: &str, cursor: usize, cx: &mut Context<Self>) {
        self.content = text.to_string();
        let at = self.content.ceil_char_boundary(cursor);
        self.selected_range = at..at;
        self.selection_reversed = false;
        self.marked_range = None;
        self.goal_x = None;
        self.scroll_x = px(0.);
        self.layout = None;
        self.selecting = false;
        self.reset_blink(cx);
        cx.notify();
    }

    pub fn sync_text(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.content == text {
            return;
        }
        let start = carried_over(&self.content, text, self.selected_range.start);
        let end = carried_over(&self.content, text, self.selected_range.end);
        self.content = text.to_string();
        self.selected_range = start..end;
        self.marked_range = None;
        cx.notify();
    }

    pub fn begin_mouse_selection(&mut self, cx: &mut Context<Self>) {
        self.selecting = true;
        cx.notify();
    }

    pub fn is_at_start(&self) -> bool {
        self.selected_range == (0..0)
    }

    fn reset_blink(&mut self, cx: &mut Context<Self>) {
        self.cursor_visible = true;
        if !self.focused {
            self.blink_task = None;
            return;
        }
        self.blink_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let toggled = this.update(cx, |this, cx| {
                    this.cursor_visible = !this.cursor_visible;
                    cx.notify();
                });
                if toggled.is_err() {
                    break;
                }
            }
        }));
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        self.selection_reversed = false;
        self.reset_blink(cx);
        cx.notify();
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset;
        } else {
            self.selected_range.end = offset;
        }
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        self.reset_blink(cx);
        cx.notify();
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(index, _)| (index < offset).then_some(index))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(index, _)| (index > offset).then_some(index))
            .unwrap_or(self.content.len())
    }

    fn previous_word(&self, offset: usize) -> usize {
        if self.masked {
            return 0;
        }
        self.content
            .unicode_word_indices()
            .map(|(index, _)| index)
            .take_while(|&index| index < offset)
            .last()
            .unwrap_or(0)
    }

    fn next_word(&self, offset: usize) -> usize {
        if self.masked {
            return self.content.len();
        }
        for (index, word) in self.content.unicode_word_indices() {
            if index + word.len() > offset {
                return index + word.len();
            }
        }
        self.content.len()
    }

    fn line_start(&self, offset: usize) -> usize {
        if let Some(layout) = &self.layout
            && self.multiline
            && layout_matches(layout, &self.content)
        {
            return layout.line_range_at(offset).start;
        }
        self.content[..offset].rfind('\n').map_or(0, |at| at + 1)
    }

    fn line_end(&self, offset: usize) -> usize {
        if let Some(layout) = &self.layout
            && self.multiline
            && layout_matches(layout, &self.content)
        {
            let range = layout.line_range_at(offset);
            let paragraph_end = self.content[offset..]
                .find('\n')
                .map_or(self.content.len(), |at| offset + at);
            if range.end < paragraph_end {
                return self.previous_boundary(range.end).max(range.start);
            }
            return paragraph_end;
        }
        self.content[offset..]
            .find('\n')
            .map_or(self.content.len(), |at| offset + at)
    }

    fn vertical(&self, offset: usize, lines: f32) -> Option<(usize, Pixels)> {
        let layout = self.layout.as_ref()?;
        if !layout_matches(layout, &self.content) {
            return None;
        }
        let at = layout.position_for(offset);
        let x = self.goal_x.unwrap_or(at.x);
        let y = at.y + layout.line_height * lines + layout.line_height / 2.;
        let top = layout.origin.y;
        let bottom = top
            + layout
                .paragraphs
                .iter()
                .map(|paragraph| paragraph.line.size(layout.line_height).height)
                .fold(px(0.), |sum, height| sum + height);
        if y < top {
            return Some((0, x));
        }
        if y >= bottom {
            return Some((self.content.len(), x));
        }
        Some((layout.index_for(point(x, y)), x))
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor()), cx);
        } else {
            self.move_to(self.selected_range.start, cx);
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx);
        }
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            self.goal_x = None;
            self.move_to(0, cx);
            return;
        }
        let from = if self.selected_range.is_empty() {
            self.cursor()
        } else {
            self.selected_range.start
        };
        if let Some((to, x)) = self.vertical(from, -1.) {
            self.move_to(to, cx);
            self.goal_x = Some(x);
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            self.goal_x = None;
            self.move_to(self.content.len(), cx);
            return;
        }
        let from = if self.selected_range.is_empty() {
            self.cursor()
        } else {
            self.selected_range.end
        };
        if let Some((to, x)) = self.vertical(from, 1.) {
            self.move_to(to, cx);
            self.goal_x = Some(x);
        }
    }

    fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(self.previous_word(self.cursor()), cx);
    }

    fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(self.next_word(self.cursor()), cx);
    }

    fn line_start_action(&mut self, _: &LineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(self.line_start(self.cursor()), cx);
    }

    fn line_end_action(&mut self, _: &LineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(self.line_end(self.cursor()), cx);
    }

    fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(0, cx);
    }

    fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.move_to(self.content.len(), cx);
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.select_to(self.previous_boundary(self.cursor()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.goal_x = None;
        self.select_to(self.next_boundary(self.cursor()), cx);
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            self.select_to(0, cx);
            return;
        }
        if let Some((to, x)) = self.vertical(self.cursor(), -1.) {
            self.select_to(to, cx);
            self.goal_x = Some(x);
        }
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            self.select_to(self.content.len(), cx);
            return;
        }
        if let Some((to, x)) = self.vertical(self.cursor(), 1.) {
            self.select_to(to, cx);
            self.goal_x = Some(x);
        }
    }

    fn select_word_left(&mut self, _: &SelectWordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_word(self.cursor()), cx);
    }

    fn select_word_right(&mut self, _: &SelectWordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_word(self.cursor()), cx);
    }

    fn select_line_start(&mut self, _: &SelectLineStart, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_start(self.cursor()), cx);
    }

    fn select_line_end(&mut self, _: &SelectLineEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.line_end(self.cursor()), cx);
    }

    fn select_doc_start(&mut self, _: &SelectDocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(0, cx);
    }

    fn select_doc_end(&mut self, _: &SelectDocEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.content.len(), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.reset_blink(cx);
        cx.notify();
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let from = self.previous_char_boundary(self.cursor());
            self.select_to(from, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn previous_char_boundary(&self, offset: usize) -> usize {
        let grapheme = self.previous_boundary(offset);
        let cluster = &self.content[grapheme..offset];
        if cluster.chars().count() > 1 && cluster.chars().any(is_combining) {
            return self.content[..offset]
                .char_indices()
                .next_back()
                .map_or(0, |(index, _)| index);
        }
        grapheme
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let to = self.next_boundary(self.cursor());
            self.select_to(to, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_left(
        &mut self,
        _: &DeleteWordLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let from = self.previous_word(self.cursor());
            self.select_to(from, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_word_right(
        &mut self,
        _: &DeleteWordRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let to = self.next_word(self.cursor());
            self.select_to(to, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_to_line_start(
        &mut self,
        _: &DeleteToLineStart,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let from = self.line_start(self.cursor());
            self.select_to(from, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn delete_to_line_end(
        &mut self,
        _: &DeleteToLineEnd,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_range.is_empty() {
            let cursor = self.cursor();
            let mut to = self.line_end(cursor);
            if to == cursor && to < self.content.len() {
                to = self.next_boundary(cursor);
            }
            self.select_to(to, cx);
        }
        self.replace_text_in_range(None, "", window, cx);
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        self.replace_text_in_range(None, "\n", window, cx);
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() && !self.read_only && !self.masked {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pasted) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        if self.encode_urls
            && let Some(url) = encode_pasted_url(&pasted)
            && url != pasted.trim()
        {
            let replaced = utf16_len(&self.content[self.selected_range.clone()]);
            let fits = self
                .max_len
                .is_none_or(|max| utf16_len(&self.content) - replaced + utf16_len(&url) <= max);
            if fits {
                self.replace_text_in_range(None, &url, window, cx);
            }
            return;
        }
        let text = if self.multiline {
            pasted.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            pasted.replace("\r\n", " ").replace(['\r', '\n'], " ")
        };
        self.replace_text_in_range(None, &text, window, cx);
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.goal_x = None;
        let index = self.index_for_mouse(event.position);
        match event.click_count {
            2 => {
                let (start, end) = self.word_around(index);
                self.selected_range = start..end;
                self.selection_reversed = false;
                self.reset_blink(cx);
                cx.notify();
            }
            count if count >= 3 => {
                let start = self.content[..index].rfind('\n').map_or(0, |at| at + 1);
                let end = self.content[index..]
                    .find('\n')
                    .map_or(self.content.len(), |at| index + at);
                self.selected_range = start..end;
                self.selection_reversed = false;
                self.reset_blink(cx);
                cx.notify();
            }
            _ => {
                self.selecting = true;
                if event.modifiers.shift {
                    self.select_to(index, cx);
                } else {
                    self.move_to(index, cx);
                }
            }
        }
        cx.stop_propagation();
    }

    fn word_around(&self, index: usize) -> (usize, usize) {
        if self.masked {
            return (0, self.content.len());
        }
        for (start, word) in self.content.split_word_bound_indices() {
            let end = start + word.len();
            if start <= index && index < end || end == index && end == self.content.len() {
                return (start, end);
            }
        }
        (index, index)
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.selecting && event.pressed_button == Some(MouseButton::Left) {
            let index = self.index_for_mouse(event.position);
            self.select_to(index, cx);
        } else if self.selecting {
            self.selecting = false;
        }
    }

    pub fn index_for_mouse(&self, position: Point<Pixels>) -> usize {
        let Some(layout) = self.layout.as_ref() else {
            return self.content.len();
        };
        if !layout_matches(layout, &self.content) {
            return self.content.len();
        }
        if !self.multiline {
            if position.y < layout.bounds.top() && position.x < layout.bounds.left() {
                return 0;
            }
            let line = &layout.paragraphs[0].line;
            let local = point(position.x - layout.origin.x, px(0.));
            return layout.actual(
                match line.closest_index_for_position(local, layout.line_height) {
                    Ok(index) | Err(index) => index,
                },
            );
        }
        layout.index_for(position)
    }

    fn fits(&self, range: &Range<usize>, text: &str) -> String {
        let Some(max) = self.max_len else {
            return text.to_string();
        };
        let kept = utf16_len(&self.content) - utf16_len(&self.content[range.clone()]);
        let room = max.saturating_sub(kept);
        let mut used = 0;
        let mut out = String::new();
        for ch in text.chars() {
            used += ch.len_utf16();
            if used > room {
                break;
            }
            out.push(ch);
        }
        out
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        offset_from_utf16(&self.content, range.start)..offset_from_utf16(&self.content, range.end)
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        offset_to_utf16(&self.content, range.start)..offset_to_utf16(&self.content, range.end)
    }
}

fn is_combining(ch: char) -> bool {
    matches!(ch as u32, 0x0300..=0x036f | 0x3099..=0x309a)
}

fn carried_over(old: &str, new: &str, offset: usize) -> usize {
    let prefix = old
        .bytes()
        .zip(new.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = old
        .bytes()
        .rev()
        .zip(new.bytes().rev())
        .take(old.len().min(new.len()) - prefix)
        .take_while(|(a, b)| a == b)
        .count();
    let mapped = if offset <= prefix {
        offset
    } else if offset >= old.len() - suffix {
        new.len() - (old.len() - offset)
    } else {
        new.len() - suffix
    };
    caret_in(new, mapped)
}

fn layout_matches(layout: &FieldLayout, content: &str) -> bool {
    layout.text == content
}

pub fn offset_from_utf16(text: &str, offset: usize) -> usize {
    let mut utf8 = 0;
    let mut utf16 = 0;
    for ch in text.chars() {
        if utf16 >= offset {
            break;
        }
        utf16 += ch.len_utf16();
        utf8 += ch.len_utf8();
    }
    utf8
}

pub fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    let mut utf8 = 0;
    for ch in text.chars() {
        if utf8 >= offset {
            break;
        }
        utf8 += ch.len_utf8();
        utf16 += ch.len_utf16();
    }
    utf16
}

impl EntityInputHandler for TextField {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        let text = &self.content[range];
        Some(if self.masked {
            BULLET.to_string().repeat(utf16_len(text))
        } else {
            text.to_string()
        })
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.take().is_some() {
            cx.emit(FieldEvent::CompositionEnded);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let was_composing = self.marked_range.is_some();
        if self.read_only {
            self.marked_range = None;
            cx.notify();
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let text = if self.multiline {
            new_text.to_string()
        } else {
            new_text.replace(['\r', '\n'], " ")
        };
        let text = self.fits(&range, &text);
        let unchanged = self.content[range.clone()] == text;
        self.content.replace_range(range.clone(), &text);
        let at = range.start + text.len();
        self.selected_range = at..at;
        self.selection_reversed = false;
        self.marked_range = None;
        self.goal_x = None;
        self.reset_blink(cx);
        if !unchanged || was_composing {
            cx.emit(FieldEvent::Changed { composing: false });
        }
        if was_composing {
            cx.emit(FieldEvent::CompositionEnded);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.read_only {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range| self.range_from_utf16(range))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        let text = if self.multiline {
            new_text.to_string()
        } else {
            new_text.replace(['\r', '\n'], " ")
        };
        self.content.replace_range(range.clone(), &text);
        self.marked_range = (!text.is_empty()).then(|| range.start..range.start + text.len());
        self.selected_range = new_selected_range_utf16.as_ref().map_or_else(
            || range.start + text.len()..range.start + text.len(),
            |selected| {
                let start = range.start + offset_from_utf16(&text, selected.start);
                let end = range.start + offset_from_utf16(&text, selected.end);
                start..end
            },
        );
        self.selection_reversed = false;
        self.goal_x = None;
        self.reset_blink(cx);
        cx.emit(FieldEvent::Changed {
            composing: self.marked_range.is_some(),
        });
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let start = layout.position_for(offset_from_utf16(&layout.text, range_utf16.start));
        let end = layout.position_for(offset_from_utf16(&layout.text, range_utf16.end));
        let end_x = if end.y == start.y { end.x } else { start.x };
        Some(Bounds::from_corners(
            start,
            point(end_x, start.y + layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        position: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        let index = self.index_for_mouse(position);
        Some(offset_to_utf16(&self.content, index))
    }
}

struct FieldElement {
    field: Entity<TextField>,
}

struct Prepainted {
    layout: FieldLayout,
    quads: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
    placeholder: bool,
}

impl IntoElement for FieldElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn runs_for(
    content: &str,
    marked: Option<&Range<usize>>,
    color: Hsla,
    font: gpui::Font,
) -> Vec<TextRun> {
    let run = TextRun {
        len: content.len(),
        font,
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let Some(marked) = marked else {
        return vec![run];
    };
    let marked = marked.start.min(content.len())..marked.end.min(content.len());
    [
        TextRun {
            len: marked.start,
            ..run.clone()
        },
        TextRun {
            len: marked.end - marked.start,
            underline: Some(UnderlineStyle {
                color: Some(color),
                thickness: px(1.),
                wavy: false,
            }),
            ..run.clone()
        },
        TextRun {
            len: content.len() - marked.end,
            ..run
        },
    ]
    .into_iter()
    .filter(|run| run.len > 0)
    .collect()
}

#[derive(Clone)]
struct Shape {
    color: Hsla,
    font: gpui::Font,
    font_size: Pixels,
    line_height: Pixels,
    wrap_width: Option<Pixels>,
}

fn shape_paragraphs(
    text: &str,
    marked: Option<&Range<usize>>,
    shape: &Shape,
    window: &mut Window,
) -> Vec<Paragraph> {
    let Shape {
        color,
        font,
        font_size,
        line_height,
        wrap_width,
    } = shape.clone();
    let mut paragraphs = Vec::new();
    let mut start = 0;
    let mut top = px(0.);
    for piece in text.split('\n') {
        let local_marked = marked.and_then(|marked| {
            let from = marked.start.max(start);
            let to = marked.end.min(start + piece.len());
            (from < to).then(|| from - start..to - start)
        });
        let runs = runs_for(piece, local_marked.as_ref(), color, font.clone());
        let line = window
            .text_system()
            .shape_text(
                SharedString::from(piece.to_string()),
                font_size,
                &runs,
                wrap_width,
                None,
            )
            .ok()
            .and_then(|mut lines| lines.pop())
            .unwrap_or_default();
        let height = line.size(line_height).height.max(line_height);
        paragraphs.push(Paragraph {
            start,
            line: Rc::new(line),
            top,
        });
        top += height;
        start += piece.len() + 1;
    }
    paragraphs
}

impl Element for FieldElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepainted;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let field = self.field.read(cx);
        let style_text = window.text_style();
        let font_size = style_text.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let mut style = Style {
            flex_grow: 1.,
            flex_shrink: 1.,
            flex_basis: px(0.).into(),
            ..Default::default()
        };
        style.min_size.width = px(0.).into();
        if !field.multiline {
            style.size.height = line_height.into();
            return (window.request_layout(style, [], cx), ());
        }
        let text = if field.content.is_empty() {
            " ".to_string()
        } else {
            field.content.clone()
        };
        let font = style_text.font();
        let color = style_text.color;
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    gpui::AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let shape = Shape {
                    color,
                    font: font.clone(),
                    font_size,
                    line_height,
                    wrap_width: width,
                };
                let paragraphs = shape_paragraphs(&text, None, &shape, window);
                let height = paragraphs
                    .iter()
                    .map(|paragraph| paragraph.line.size(line_height).height.max(line_height))
                    .fold(px(0.), |sum, height| sum + height);
                size(width.unwrap_or(px(0.)), height)
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let colors = theme(cx);
        let field = self.field.read(cx);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.line_height();
        let placeholder = field.content.is_empty() && !field.placeholder.is_empty();
        let dotted = field.masked && !placeholder;
        let (text, color) = if placeholder {
            (field.placeholder.to_string(), colors.muted)
        } else if dotted {
            (mask(&field.content), style.color)
        } else {
            (field.content.clone(), style.color)
        };
        let shown = |index| shown_offset(&field.content, dotted, index);
        let marked = field
            .marked_range
            .as_ref()
            .filter(|_| !placeholder)
            .map(|range| shown(range.start)..shown(range.end));
        let wrap = field.multiline.then_some(bounds.size.width);
        let paragraphs = shape_paragraphs(
            &text,
            marked.as_ref(),
            &Shape {
                color,
                font: style.font(),
                font_size,
                line_height,
                wrap_width: wrap,
            },
            window,
        );
        let content = field.content.clone();
        let selection = field.selected_range.clone();
        let cursor_at = field.cursor();
        let multiline = field.multiline;
        let mut scroll_x = field.scroll_x;

        let mut layout = FieldLayout {
            text: if dotted { content.clone() } else { text },
            masked: dotted,
            paragraphs,
            origin: bounds.origin,
            bounds,
            line_height,
        };
        if !multiline && !placeholder {
            let cursor_x = layout.paragraphs[0]
                .line
                .position_for_index(layout.shown(cursor_at), line_height)
                .map_or(px(0.), |at| at.x);
            let width = bounds.size.width - px(2.);
            let line_width = layout.paragraphs[0].line.width();
            if cursor_x - scroll_x > width {
                scroll_x = cursor_x - width;
            } else if cursor_x < scroll_x {
                scroll_x = cursor_x;
            }
            scroll_x = scroll_x.min((line_width - width).max(px(0.))).max(px(0.));
            if cursor_x - scroll_x > width {
                scroll_x = cursor_x - width;
            }
        } else {
            scroll_x = px(0.);
        }
        layout.origin.x -= scroll_x;

        let mut quads = Vec::new();
        let focused = field.focused;
        let mut cursor = None;
        if !placeholder || content.is_empty() {
            if selection.is_empty() || placeholder {
                if focused && field.cursor_visible {
                    let at = if placeholder {
                        layout.origin
                    } else {
                        layout.position_for(cursor_at)
                    };
                    let caret = (font_size * 1.25).min(line_height);
                    let inset = (line_height - caret) / 2.;
                    cursor = Some(fill(
                        Bounds::new(point(at.x, at.y + inset), size(px(1.5), caret)),
                        colors.text,
                    ));
                }
            } else {
                let start = layout.position_for(selection.start);
                let end = layout.position_for(selection.end);
                let right = if multiline {
                    bounds.right()
                } else {
                    end.x.max(start.x)
                };
                if (start.y - end.y).abs() < px(0.5) {
                    let inset = if multiline {
                        px(0.)
                    } else {
                        (line_height - (font_size * 1.25).min(line_height)) / 2.
                    };
                    quads.push(fill(
                        Bounds::from_corners(
                            point(start.x, start.y + inset),
                            point(end.x, end.y + line_height - inset),
                        ),
                        colors.selection,
                    ));
                } else {
                    quads.push(fill(
                        Bounds::from_corners(start, point(right, start.y + line_height)),
                        colors.selection,
                    ));
                    if end.y - start.y > line_height + px(0.5) {
                        quads.push(fill(
                            Bounds::from_corners(
                                point(layout.origin.x, start.y + line_height),
                                point(right, end.y),
                            ),
                            colors.selection,
                        ));
                    }
                    quads.push(fill(
                        Bounds::from_corners(
                            point(layout.origin.x, end.y),
                            point(end.x, end.y + line_height),
                        ),
                        colors.selection,
                    ));
                }
            }
        }
        self.field.update(cx, |field, _| {
            field.scroll_x = scroll_x;
        });
        Prepainted {
            layout,
            quads,
            cursor,
            placeholder,
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.field.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.field.clone()),
            cx,
        );
        let clip = Bounds::from_corners(
            point(bounds.left() - px(2.), bounds.top()),
            point(bounds.right() + px(2.), bounds.bottom()),
        );
        window.with_content_mask(Some(gpui::ContentMask { bounds: clip }), |window| {
            for quad in prepaint.quads.drain(..) {
                window.paint_quad(quad);
            }
            let line_height = prepaint.layout.line_height;
            for paragraph in &prepaint.layout.paragraphs {
                let origin = point(
                    prepaint.layout.origin.x,
                    prepaint.layout.origin.y + paragraph.top,
                );
                paragraph
                    .line
                    .paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                    .ok();
            }
            if let Some(cursor) = prepaint.cursor.take() {
                window.paint_quad(cursor);
            }
        });
        let layout = prepaint.layout.clone();
        let placeholder = prepaint.placeholder;
        self.field.update(cx, |field, _| {
            field.layout = (!placeholder).then_some(layout).or_else(|| {
                field.content.is_empty().then(|| FieldLayout {
                    text: String::new(),
                    masked: false,
                    paragraphs: vec![Paragraph {
                        start: 0,
                        line: Rc::default(),
                        top: px(0.),
                    }],
                    origin: bounds.origin,
                    bounds,
                    line_height: window.line_height(),
                })
            });
        });
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focused = self.focus_handle.is_focused(window) && window.is_window_active();
        if self.focused {
            if self.blink_task.is_none() {
                self.reset_blink(cx);
            }
        } else if self.blink_task.take().is_some() {
            self.cursor_visible = true;
        }
        div()
            .id("text-field")
            .key_context(self.context.as_ref())
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .flex()
            .flex_1()
            .min_w_0()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::delete_word_left))
            .on_action(cx.listener(Self::delete_word_right))
            .on_action(cx.listener(Self::delete_to_line_start))
            .on_action(cx.listener(Self::delete_to_line_end))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::line_start_action))
            .on_action(cx.listener(Self::line_end_action))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::select_line_start))
            .on_action(cx.listener(Self::select_line_end))
            .on_action(cx.listener(Self::select_doc_start))
            .on_action(cx.listener(Self::select_doc_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(|_: &ShowCharacterPalette, window, _| window.show_character_palette())
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(FieldElement { field: cx.entity() })
    }
}

#[cfg(test)]
mod tests {
    use gpui::{EntityInputHandler, TestAppContext};

    use super::*;
    use crate::ui::theme::Theme;

    #[gpui::test]
    fn a_click_handled_before_the_next_frame_never_lands_inside_a_character_even_when_the_edit_kept_the_byte_length(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| cx.set_global(Theme::light()));
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new("Note", cx).multiline());
        field.update(cx, |field, cx| field.set_text("ab", 2, cx));
        cx.run_until_parked();
        let between_a_and_b = field.read_with(cx, |field, _| {
            field.layout.as_ref().map(|layout| layout.position_for(1))
        });

        let clicked = field.update_in(cx, |field, window, cx| {
            field.replace_text_in_range(Some(0..2), "\u{e9}", window, cx);
            between_a_and_b.map(|at| field.index_for_mouse(at))
        });

        let text = field.read_with(cx, |field, _| field.text().to_string());
        assert_eq!(text, "\u{e9}");
        assert!(clicked.is_some_and(|index| text.is_char_boundary(index)));
    }

    #[test]
    fn a_caret_keeps_its_place_in_text_another_device_changed_around_it() {
        assert_eq!(carried_over("Buy milk", "Buy oat milk", 1), 1);
        assert_eq!(carried_over("Buy milk", "Buy oat milk", 8), 12);
        assert_eq!(carried_over("Buy milk", "Buy soy", 6), 7);
        assert_eq!(carried_over("Milk", "Milk 2L", 4), 4);
        assert_eq!(carried_over("Milk", "Oat Milk", 0), 0);
        assert_eq!(carried_over("abab", "ab", 3), 2);
        assert_eq!(carried_over("ab", "a\u{301}b", 1), 3);
    }

    #[gpui::test]
    fn the_caret_stops_blinking_while_another_app_is_in_front_so_a_background_window_does_not_keep_redrawing(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| cx.set_global(Theme::light()));
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new("Title", cx));
        cx.update(|window, _| window.activate_window());
        field.update_in(cx, |field, window, cx| {
            window.focus(&field.focus_handle, cx);
        });
        cx.run_until_parked();
        assert!(field.read_with(cx, |field, _| field.blink_task.is_some()));

        cx.deactivate_window();
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();

        assert!(field.read_with(cx, |field, _| field.blink_task.is_none()));
    }

    #[gpui::test]
    fn the_input_method_gets_a_rect_beside_the_text_before_the_composition_is_drawn_so_its_candidates_stay_by_the_caret(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| cx.set_global(Theme::light()));
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new("Title", cx));
        field.update(cx, |field, cx| field.set_text("ab", 2, cx));
        cx.run_until_parked();
        let after_b = field.read_with(cx, |field, _| {
            field
                .layout
                .as_ref()
                .map(|layout| layout.position_for(2))
                .expect("the field was drawn")
        });

        let rect = field.update_in(cx, |field, window, cx| {
            field.replace_and_mark_text_in_range(None, "\u{304b}", None, window, cx);
            field.bounds_for_range(2..3, Bounds::default(), window, cx)
        });

        assert_eq!(rect.map(|rect| rect.origin), Some(after_b));
    }

    fn secret_field<'a>(
        cx: &'a mut TestAppContext,
        text: &str,
    ) -> (Entity<TextField>, &'a mut gpui::VisualTestContext) {
        cx.update(|cx| cx.set_global(Theme::light()));
        let (field, cx) = cx.add_window_view(|_, cx| TextField::new("Token", cx));
        field.update(cx, |field, cx| {
            field.set_masked(true, cx);
            field.set_text(text, text.len(), cx);
        });
        cx.run_until_parked();
        (field, cx)
    }

    #[gpui::test]
    fn a_secret_field_draws_a_dot_for_each_character_so_someone_looking_at_the_screen_cannot_read_it(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) = secret_field(cx, "t\u{f6}ken 1");

        let drawn = field.read_with(cx, |field, _| {
            field
                .layout
                .as_ref()
                .map(|layout| layout.paragraphs[0].line.text.to_string())
        });

        assert_eq!(drawn.as_deref(), Some("\u{2022}".repeat(7).as_str()));
        assert_eq!(
            field.read_with(cx, |field, _| field.text().to_string()),
            "t\u{f6}ken 1"
        );
    }

    #[gpui::test]
    fn a_secret_stays_off_the_clipboard_so_it_cannot_be_pasted_somewhere_else_by_mistake(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) = secret_field(cx, "e2e-token");
        cx.write_to_clipboard(ClipboardItem::new_string("kept".into()));

        field.update_in(cx, |field, window, cx| {
            field.select_all(&SelectAll, window, cx);
            field.copy(&Copy, window, cx);
            field.cut(&Cut, window, cx);
        });

        assert_eq!(
            cx.read_from_clipboard().and_then(|item| item.text()),
            Some("kept".into())
        );
        assert_eq!(
            field.read_with(cx, |field, _| field.text().to_string()),
            "e2e-token"
        );
    }

    #[gpui::test]
    fn a_click_between_two_dots_puts_the_caret_between_the_characters_they_stand_for(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) = secret_field(cx, "\u{e9}a\u{e9}");
        let after_a = "\u{e9}a".len();

        let (clicked, caret, second_dot) = field.read_with(cx, |field, _| {
            let layout = field.layout.as_ref().expect("the field was drawn");
            let second_dot = layout.paragraphs[0]
                .line
                .position_for_index(2 * BULLET.len_utf8(), layout.line_height)
                .expect("the second dot was drawn");
            let second_dot = point(layout.origin.x + second_dot.x, layout.origin.y);
            (
                field.index_for_mouse(second_dot),
                layout.position_for(after_a),
                second_dot,
            )
        });

        assert_eq!(clicked, after_a);
        assert_eq!(caret, second_dot);
    }

    #[gpui::test]
    fn moving_by_word_crosses_the_whole_secret_so_the_caret_does_not_reveal_where_its_spaces_are(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) = secret_field(cx, "two words");

        let (left, right, around) = field.read_with(cx, |field, _| {
            (
                field.previous_word(9),
                field.next_word(0),
                field.word_around(1),
            )
        });

        assert_eq!((left, right, around), (0, 9, (0, 9)));
    }

    #[gpui::test]
    fn the_input_method_reads_dots_instead_of_the_secret(cx: &mut TestAppContext) {
        let (field, cx) = secret_field(cx, "e2e");

        let read = field.update_in(cx, |field, window, cx| {
            field.text_for_range(0..3, &mut None, window, cx)
        });

        assert_eq!(read.as_deref(), Some("\u{2022}".repeat(3).as_str()));
    }

    #[gpui::test]
    fn turning_secret_off_shows_the_characters_again_so_a_url_can_be_checked_before_saving(
        cx: &mut TestAppContext,
    ) {
        let (field, cx) = secret_field(cx, "https://a");

        field.update(cx, |field, cx| field.set_masked(false, cx));
        cx.run_until_parked();

        let drawn = field.read_with(cx, |field, _| {
            field
                .layout
                .as_ref()
                .map(|layout| layout.paragraphs[0].line.text.to_string())
        });
        assert_eq!(drawn.as_deref(), Some("https://a"));
    }
}
