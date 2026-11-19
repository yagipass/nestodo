use std::ops::Range;

use gpui::{Bounds, Pixels, Point, SharedString, point, px};
use unicode_segmentation::UnicodeSegmentation;

use crate::ui::clamped_text::TextHits;

pub fn selection_bounds(
    start: Point<Pixels>,
    end: Point<Pixels>,
    line_height: Pixels,
    inset: Pixels,
    left: Pixels,
    right: Pixels,
) -> Vec<Bounds<Pixels>> {
    if (start.y - end.y).abs() < px(0.5) {
        return vec![Bounds::from_corners(
            point(start.x, start.y + inset),
            point(end.x, end.y + line_height - inset),
        )];
    }
    let mut areas = vec![Bounds::from_corners(
        start,
        point(right, start.y + line_height),
    )];
    if end.y - start.y > line_height + px(0.5) {
        areas.push(Bounds::from_corners(
            point(left, start.y + line_height),
            point(right, end.y),
        ));
    }
    areas.push(Bounds::from_corners(
        point(left, end.y),
        point(end.x, end.y + line_height),
    ));
    areas
}

pub fn within(selected: &Range<usize>, start: usize, len: usize) -> Option<Range<usize>> {
    if selected.is_empty() || selected.end <= start || selected.start > start + len {
        return None;
    }
    let from = selected.start.saturating_sub(start);
    let to = (selected.end - start).min(len + 1);
    (from < to).then_some(from..to)
}

struct Block {
    start: usize,
    len: usize,
    hits: TextHits,
}

#[derive(Default)]
pub struct Selectable {
    text: String,
    blocks: Vec<Block>,
    links: Vec<(Range<usize>, String)>,
}

impl Selectable {
    pub fn push(&mut self, text: &str, hits: TextHits) -> usize {
        if !self.blocks.is_empty() {
            self.text.push('\n');
        }
        let start = self.text.len();
        self.text.push_str(text);
        self.blocks.push(Block {
            start,
            len: text.len(),
            hits,
        });
        start
    }

    pub fn link(&mut self, range: Range<usize>, href: String) {
        self.links.push((range, href));
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn text_in(&self, range: Range<usize>) -> &str {
        let end = self.text.floor_char_boundary(range.end);
        let start = self.text.floor_char_boundary(range.start).min(end);
        &self.text[start..end]
    }

    pub fn offset_for(&self, position: Point<Pixels>) -> usize {
        let mut before = 0;
        for block in &self.blocks {
            let Some(bounds) = block.hits.bounds() else {
                continue;
            };
            if position.y < bounds.top() {
                return before;
            }
            if position.y < bounds.bottom() {
                let index = block.hits.nearest_index_for(position).unwrap_or(0);
                return block.start + index.min(block.len);
            }
            before = block.start + block.len;
        }
        self.text.len()
    }

    pub fn link_at(&self, position: Point<Pixels>) -> Option<&str> {
        let block = self.blocks.iter().find(|block| {
            block
                .hits
                .bounds()
                .is_some_and(|bounds| bounds.contains(&position))
        })?;
        let at = block.start + block.hits.index_for(position)?;
        self.links
            .iter()
            .find(|(range, _)| range.contains(&at))
            .map(|(_, href)| href.as_str())
    }

    fn word_at(&self, offset: usize) -> Range<usize> {
        for (start, word) in self.text.split_word_bound_indices() {
            let end = start + word.len();
            if start <= offset && offset < end || end == offset && end == self.text.len() {
                return start..end;
            }
        }
        offset..offset
    }

    fn line_at(&self, offset: usize) -> Range<usize> {
        let offset = self.text.floor_char_boundary(offset);
        let start = self.text[..offset].rfind('\n').map_or(0, |at| at + 1);
        let end = self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |at| offset + at);
        start..end
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Unit {
    Character,
    Word(Range<usize>),
    Line(Range<usize>),
}

#[derive(Clone, Debug)]
pub struct Selection {
    pub owner: SharedString,
    start: usize,
    end: usize,
    reversed: bool,
    unit: Unit,
    pub dragging: bool,
}

impl Selection {
    pub fn press(owner: SharedString, offset: usize, clicks: usize, text: &Selectable) -> Self {
        let (range, unit) = match clicks {
            0 | 1 => (offset..offset, Unit::Character),
            2 => {
                let word = text.word_at(offset);
                (word.clone(), Unit::Word(word))
            }
            _ => {
                let line = text.line_at(offset);
                (line.clone(), Unit::Line(line))
            }
        };
        Self {
            owner,
            start: range.start,
            end: range.end,
            reversed: false,
            unit,
            dragging: true,
        }
    }

    pub fn whole(owner: SharedString, text: &Selectable) -> Self {
        Self {
            owner,
            start: 0,
            end: text.len(),
            reversed: false,
            unit: Unit::Character,
            dragging: false,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub fn extend_to(&mut self, head: usize) {
        let tail = self.tail();
        self.unit = Unit::Character;
        self.dragging = true;
        self.reach(head, tail);
    }

    pub fn drag_to(&mut self, head: usize, text: &Selectable) {
        match self.unit.clone() {
            Unit::Character => {
                let tail = self.tail();
                self.reach(head, tail);
            }
            Unit::Word(first) | Unit::Line(first) => {
                let around = if matches!(self.unit, Unit::Word(_)) {
                    text.word_at(head)
                } else {
                    text.line_at(head)
                };
                if head < first.start {
                    self.start = around.start;
                    self.end = first.end;
                    self.reversed = true;
                } else if head >= first.end {
                    self.start = first.start;
                    self.end = around.end;
                    self.reversed = false;
                } else {
                    self.start = first.start;
                    self.end = first.end;
                    self.reversed = false;
                }
            }
        }
    }

    pub fn fit(&mut self, len: usize) {
        self.start = self.start.min(len);
        self.end = self.end.min(len);
    }

    fn tail(&self) -> usize {
        if self.reversed { self.end } else { self.start }
    }

    fn reach(&mut self, head: usize, tail: usize) {
        self.reversed = head < tail;
        (self.start, self.end) = if self.reversed {
            (head, tail)
        } else {
            (tail, head)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_of(blocks: &[&str]) -> Selectable {
        let mut text = Selectable::default();
        for block in blocks {
            text.push(block, TextHits::default());
        }
        text
    }

    #[test]
    fn copying_across_blocks_puts_each_block_on_its_own_line_so_paragraphs_and_list_items_do_not_run_together()
     {
        let text = text_of(&["Call the clinic", "book", "pay"]);

        assert_eq!(text.text_in(5..20), "the clinic\nbook");
        assert_eq!(text.text_in(0..text.len()), "Call the clinic\nbook\npay");
    }

    #[test]
    fn dragging_after_a_double_click_grows_the_selection_a_whole_word_at_a_time_like_a_browser() {
        let text = text_of(&["call the clinic now"]);
        let mut selection = Selection::press("c".into(), 6, 2, &text);
        assert_eq!(selection.range(), 5..8);

        selection.drag_to(11, &text);
        assert_eq!(text.text_in(selection.range()), "the clinic");

        selection.drag_to(1, &text);
        assert_eq!(text.text_in(selection.range()), "call the");
    }

    #[test]
    fn a_triple_click_selects_the_line_under_the_pointer_and_not_the_whole_comment() {
        let text = text_of(&["first line\nsecond line", "next block"]);

        let selection = Selection::press("c".into(), 14, 3, &text);

        assert_eq!(text.text_in(selection.range()), "second line");
    }

    #[test]
    fn a_shift_click_keeps_where_the_selection_started_so_it_can_be_widened_in_either_direction() {
        let text = text_of(&["call the clinic now"]);
        let mut selection = Selection::press("c".into(), 9, 1, &text);
        selection.drag_to(15, &text);

        selection.extend_to(5);

        assert_eq!(text.text_in(selection.range()), "the ");
    }

    #[test]
    fn a_selection_that_runs_into_the_next_block_marks_this_block_as_running_past_its_end_so_the_line_break_shows_as_selected()
     {
        assert_eq!(within(&(3..12), 0, 5), Some(3..6));
        assert_eq!(within(&(3..8), 6, 4), Some(0..2));
        assert_eq!(within(&(3..5), 0, 5), Some(3..5));
        assert_eq!(within(&(3..5), 6, 4), None);
        assert_eq!(within(&(4..4), 0, 5), None);
    }
}
