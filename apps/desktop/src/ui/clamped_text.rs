use std::cell::RefCell;
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, Element, ElementId, GlobalElementId, Hsla,
    InspectorElementId, IntoElement, LayoutId, Pixels, Point, SharedString, Style, TextAlign,
    TextRun, TransformationMatrix, Window, WrappedLine, WrappedLineLayout, point, px, size,
};

pub const ELLIPSIS: &str = "\u{2026}";

#[derive(Clone, Default)]
pub struct TextHits(Rc<RefCell<Option<Placed>>>);

struct Placed {
    paragraphs: Vec<(Point<Pixels>, usize, Arc<WrappedLineLayout>)>,
    line_height: Pixels,
    visible: usize,
}

impl TextHits {
    pub fn index_for(&self, position: Point<Pixels>) -> Option<usize> {
        let placed = self.0.borrow();
        let placed = placed.as_ref()?;
        let (origin, start, line) = placed.paragraphs.iter().find(|(origin, _, line)| {
            position.y < origin.y + line.size(placed.line_height).height
        })?;
        let local = point(position.x - origin.x, (position.y - origin.y).max(px(0.)));
        let index = start + line.index_for_position(local, placed.line_height).ok()?;
        (index < placed.visible).then_some(index)
    }
}

pub struct ClampedText {
    text: SharedString,
    runs: Vec<TextRun>,
    clamp: Option<usize>,
    ellipsis_color: Hsla,
    hits: TextHits,
    icons: Vec<Range<usize>>,
    icon_path: SharedString,
    icon_color: Hsla,
}

impl ClampedText {
    pub fn new(
        text: SharedString,
        runs: Vec<TextRun>,
        clamp: Option<usize>,
        ellipsis_color: Hsla,
        hits: TextHits,
    ) -> Self {
        Self {
            text,
            runs,
            clamp,
            ellipsis_color,
            hits,
            icons: Vec::new(),
            icon_path: SharedString::default(),
            icon_color: ellipsis_color,
        }
    }

    pub fn with_icons(mut self, icons: Vec<Range<usize>>, path: SharedString, color: Hsla) -> Self {
        self.icons = icons;
        self.icon_path = path;
        self.icon_color = color;
        self
    }
}

pub struct Laid {
    paragraphs: Vec<(usize, WrappedLine)>,
    visible: usize,
    height: Pixels,
    width: Pixels,
}

struct VisualLine {
    paragraph: usize,
    range: Range<usize>,
}

fn shape(
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    width: Option<Pixels>,
    window: &mut Window,
) -> Vec<(usize, WrappedLine)> {
    let lines = window
        .text_system()
        .shape_text(
            SharedString::from(text.to_string()),
            font_size,
            runs,
            width,
            None,
        )
        .unwrap_or_default();
    let mut start = 0;
    lines
        .into_iter()
        .map(|line| {
            let at = start;
            start += line.len() + 1;
            (at, line)
        })
        .collect()
}

fn visual_lines(paragraphs: &[(usize, WrappedLine)]) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    for (paragraph, (start, line)) in paragraphs.iter().enumerate() {
        let mut from = *start;
        for boundary in line.wrap_boundaries() {
            let glyph = &line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix];
            let to = start + glyph.index;
            lines.push(VisualLine {
                paragraph,
                range: from..to,
            });
            from = to;
        }
        lines.push(VisualLine {
            paragraph,
            range: from..start + line.len(),
        });
    }
    lines
}

pub fn cut_line(line_count: usize, clamp: usize) -> Option<usize> {
    (clamp > 0 && line_count > clamp).then(|| clamp - 1)
}

pub fn fit_ellipsis(
    text: &str,
    line: Range<usize>,
    x_of: impl Fn(usize) -> Pixels,
    ellipsis: Pixels,
    width: Pixels,
) -> usize {
    let mut cut = line.end;
    while cut > line.start {
        if x_of(cut) + ellipsis <= width {
            break;
        }
        cut = text[..cut]
            .char_indices()
            .next_back()
            .map_or(line.start, |(index, _)| index)
            .max(line.start);
    }
    while cut > line.start {
        let Some((index, ch)) = text[..cut].char_indices().next_back() else {
            break;
        };
        if !ch.is_whitespace() {
            break;
        }
        cut = index;
    }
    cut
}

pub fn runs_up_to(runs: &[TextRun], cut: usize) -> Vec<TextRun> {
    let mut kept = Vec::new();
    let mut at = 0;
    for run in runs {
        if at >= cut {
            break;
        }
        let len = run.len.min(cut - at);
        if len > 0 {
            kept.push(TextRun { len, ..run.clone() });
        }
        at += run.len;
    }
    kept
}

#[derive(Clone, Copy)]
struct Metrics {
    font_size: Pixels,
    line_height: Pixels,
}

impl Metrics {
    fn of(window: &Window) -> Self {
        Self {
            font_size: window.text_style().font_size.to_pixels(window.rem_size()),
            line_height: window.line_height(),
        }
    }
}

impl ClampedText {
    fn lay_out(
        text: &str,
        runs: &[TextRun],
        clamp: Option<usize>,
        ellipsis_color: Hsla,
        metrics: Metrics,
        width: Option<Pixels>,
        window: &mut Window,
    ) -> Laid {
        let Metrics {
            font_size,
            line_height,
        } = metrics;
        let paragraphs = shape(text, runs, font_size, width, window);
        let lines = visual_lines(&paragraphs);
        let widest = paragraphs
            .iter()
            .map(|(_, line)| line.width())
            .fold(px(0.), std::cmp::Ord::max);
        let cut_at = clamp.and_then(|clamp| cut_line(lines.len(), clamp));
        let (Some(width), Some(last)) = (width, cut_at.map(|index| &lines[index])) else {
            let shown = clamp.map_or(lines.len(), |clamp| lines.len().min(clamp));
            return Laid {
                paragraphs,
                visible: text.len(),
                height: line_height * shown.max(1) as f32,
                width: widest,
            };
        };
        let Some(base) = runs.first() else {
            return Laid {
                paragraphs,
                visible: text.len(),
                height: line_height,
                width: widest,
            };
        };
        let ellipsis_run = TextRun {
            len: ELLIPSIS.len(),
            font: base.font.clone(),
            color: ellipsis_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let ellipsis = window
            .text_system()
            .shape_line(
                SharedString::new_static(ELLIPSIS),
                font_size,
                std::slice::from_ref(&ellipsis_run),
                None,
            )
            .width;
        let (start, line) = &paragraphs[last.paragraph];
        let line_start = last.range.start;
        let cut = fit_ellipsis(
            text,
            last.range.clone(),
            |index| {
                if index == line_start {
                    return px(0.);
                }
                line.position_for_index(index - start, line_height)
                    .map_or(width, |at| at.x)
            },
            ellipsis,
            width,
        );
        let shown = format!("{}{ELLIPSIS}", &text[..cut]);
        let mut shown_runs = runs_up_to(runs, cut);
        shown_runs.push(ellipsis_run);
        let lines_shown = cut_at.unwrap_or(0) + 1;
        Laid {
            paragraphs: shape(&shown, &shown_runs, font_size, Some(width), window),
            visible: cut,
            height: line_height * lines_shown as f32,
            width,
        }
    }
}

impl IntoElement for ClampedText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for ClampedText {
    type RequestLayoutState = ();
    type PrepaintState = Laid;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let metrics = Metrics::of(window);
        let text = self.text.clone();
        let runs = self.runs.clone();
        let clamp = self.clamp;
        let ellipsis_color = self.ellipsis_color;
        let layout_id =
            window.request_measured_layout(Style::default(), move |known, available, window, _| {
                let width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                let laid =
                    Self::lay_out(&text, &runs, clamp, ellipsis_color, metrics, width, window);
                size(width.unwrap_or(laid.width), laid.height)
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        let metrics = Metrics::of(window);
        let line_height = metrics.line_height;
        let laid = Self::lay_out(
            &self.text,
            &self.runs,
            self.clamp,
            self.ellipsis_color,
            metrics,
            Some(bounds.size.width),
            window,
        );
        let mut top = bounds.top();
        let paragraphs = laid
            .paragraphs
            .iter()
            .map(|(start, line)| {
                let origin = point(bounds.left(), top);
                top += line.size(line_height).height;
                (origin, *start, Arc::clone(line))
            })
            .collect();
        self.hits.0.replace(Some(Placed {
            paragraphs,
            line_height,
            visible: laid.visible,
        }));
        laid
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        (): &mut Self::RequestLayoutState,
        laid: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = window.line_height();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            let mut origin = bounds.origin;
            for (start, line) in &laid.paragraphs {
                line.paint_background(origin, line_height, TextAlign::Left, None, window, cx)
                    .ok();
                line.paint(origin, line_height, TextAlign::Left, None, window, cx)
                    .ok();
                for icon in &self.icons {
                    if icon.end > laid.visible
                        || icon.start < *start
                        || icon.end > start + line.len()
                    {
                        continue;
                    }
                    let (Some(from), Some(to)) = (
                        line.position_for_index(icon.start - start, line_height),
                        line.position_for_index(icon.end - start, line_height),
                    ) else {
                        continue;
                    };
                    if (to.y - from.y).abs() > px(0.5) {
                        continue;
                    }
                    let side = (line_height * 0.55).min(to.x - from.x + px(4.));
                    let at = origin
                        + point(
                            from.x + (to.x - from.x - side) / 2.,
                            from.y + (line_height - side) / 2.,
                        );
                    window
                        .paint_svg(
                            Bounds::new(at, size(side, side)),
                            self.icon_path.clone(),
                            None,
                            TransformationMatrix::unit(),
                            self.icon_color,
                            cx,
                        )
                        .ok();
                }
                origin.y += line.size(line_height).height;
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::font;

    fn run(len: usize, color: Hsla) -> TextRun {
        TextRun {
            len,
            font: font(".SystemUIFont"),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }

    #[test]
    fn a_note_that_fits_in_two_lines_is_shown_whole_like_the_web_line_clamp() {
        assert_eq!(cut_line(1, 2), None);
        assert_eq!(cut_line(2, 2), None);
    }

    #[gpui::test]
    fn a_longer_note_stops_after_its_second_line_counting_every_paragraph_together(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let text = "one\ntwo\nthree";
        let laid = cx.update(|window, _| {
            ClampedText::lay_out(
                text,
                &[run(text.len(), gpui::black())],
                Some(2),
                gpui::black(),
                Metrics {
                    font_size: px(14.),
                    line_height: px(21.),
                },
                Some(px(400.)),
                window,
            )
        });
        assert!(laid.visible > "one\n".len());
        assert!(laid.visible <= "one\ntwo".len());
        assert_eq!(laid.height, px(42.));
    }

    #[test]
    fn the_ellipsis_takes_the_room_of_the_last_characters_so_it_never_wraps_to_a_third_line() {
        let text = "abcdefghij";
        let cut = fit_ellipsis(
            text,
            0..10,
            |index| px(index as f32 * 10.),
            px(25.),
            px(100.),
        );
        assert_eq!(cut, 7);
    }

    #[test]
    fn the_ellipsis_follows_the_last_word_rather_than_a_trailing_space() {
        let text = "abc def ghi";
        let cut = fit_ellipsis(text, 0..8, |index| px(index as f32 * 10.), px(5.), px(100.));
        assert_eq!(&text[..cut], "abc def");
    }

    #[test]
    fn the_ellipsis_is_placed_after_a_whole_japanese_character_never_inside_one() {
        let text = "長い長い長い";
        let cut = fit_ellipsis(
            text,
            0..text.len(),
            |index| px(text[..index].chars().count() as f32 * 14.),
            px(14.),
            px(60.),
        );
        assert!(text.is_char_boundary(cut));
        assert_eq!(&text[..cut], "長い長");
    }

    #[test]
    fn the_ellipsis_starts_the_second_line_when_nothing_else_fits_on_it() {
        let text = "first\nsecond";
        let cut = fit_ellipsis(
            text,
            6..12,
            |index| px((index - 6) as f32 * 10.),
            px(200.),
            px(100.),
        );
        assert_eq!(cut, 6);
    }

    #[test]
    fn colors_are_kept_up_to_the_cut_so_a_link_stays_highlighted() {
        let link = gpui::blue();
        let plain = gpui::black();
        let kept = runs_up_to(&[run(4, plain), run(6, link), run(5, plain)], 7);
        assert_eq!(
            kept.iter()
                .map(|run| (run.len, run.color))
                .collect::<Vec<_>>(),
            [(4, plain), (3, link)]
        );
    }

    fn hits_for(text: &str, window: &mut Window) -> (TextHits, WrappedLine) {
        let runs = [run(text.len(), gpui::black())];
        let (_, line) = shape(text, &runs, px(14.), None, window).remove(0);
        let hits = TextHits::default();
        hits.0.replace(Some(Placed {
            paragraphs: vec![(point(px(0.), px(0.)), 0, Arc::clone(&line))],
            line_height: px(21.),
            visible: text.len(),
        }));
        (hits, line)
    }

    #[gpui::test]
    fn a_click_on_the_right_half_of_a_links_last_glyph_still_lands_on_the_link_so_the_arrow_opens_it(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let text = "see example.com\u{2197}";
        let arrow = text.rfind('\u{2197}').unwrap();
        let index = cx.update(|window, _| {
            let (hits, line) = hits_for(text, window);
            let start = line.position_for_index(arrow, px(21.)).unwrap().x;
            let end = line.position_for_index(text.len(), px(21.)).unwrap().x;
            hits.index_for(point(start + (end - start) * 0.75, px(5.)))
        });
        assert_eq!(index, Some(arrow));
    }

    #[gpui::test]
    fn a_click_in_the_blank_space_after_a_wrapped_line_lands_on_no_character_so_it_cannot_open_a_link_on_the_next_line(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let text = "see the page example.com\u{2197}";
        let (wrapped_at, found) = cx.update(|window, _| {
            let runs = [run(text.len(), gpui::black())];
            let (_, line) = shape(text, &runs, px(14.), Some(px(100.)), window).remove(0);
            let boundary = line.wrap_boundaries()[0];
            let wrapped_at = line.runs()[boundary.run_ix].glyphs[boundary.glyph_ix].index;
            let hits = TextHits::default();
            hits.0.replace(Some(Placed {
                paragraphs: vec![(point(px(0.), px(0.)), 0, Arc::clone(&line))],
                line_height: px(21.),
                visible: text.len(),
            }));
            (wrapped_at, hits.index_for(point(px(99.), px(5.))))
        });
        assert!(wrapped_at < text.len());
        assert_eq!(found, None);
    }

    #[gpui::test]
    fn a_click_on_the_ellipsis_of_a_cut_note_lands_on_no_character_so_it_cannot_open_the_link_it_cut(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let shown = format!("see exa{ELLIPSIS}");
        let cut = "see exa".len();
        let found = cx.update(|window, _| {
            let runs = [run(shown.len(), gpui::black())];
            let (_, line) = shape(&shown, &runs, px(14.), None, window).remove(0);
            let start = line.position_for_index(cut, px(21.)).unwrap().x;
            let end = line.position_for_index(shown.len(), px(21.)).unwrap().x;
            let hits = TextHits::default();
            hits.0.replace(Some(Placed {
                paragraphs: vec![(point(px(0.), px(0.)), 0, Arc::clone(&line))],
                line_height: px(21.),
                visible: cut,
            }));
            hits.index_for(point((start + end) / 2., px(5.)))
        });
        assert_eq!(found, None);
    }

    #[gpui::test]
    fn a_single_paragraph_that_wraps_past_two_lines_is_cut_with_an_ellipsis_like_the_web_line_clamp(
        cx: &mut gpui::TestAppContext,
    ) {
        let cx = cx.add_empty_window();
        let text = "word ".repeat(40);
        let laid = cx.update(|window, _| {
            ClampedText::lay_out(
                &text,
                &[run(text.len(), gpui::black())],
                Some(2),
                gpui::black(),
                Metrics {
                    font_size: px(14.),
                    line_height: px(21.),
                },
                Some(px(120.)),
                window,
            )
        });
        assert!(laid.visible < text.len());
        assert_eq!(laid.height, px(42.));
        let (_, shown) = laid.paragraphs.last().expect("the note was laid out");
        assert!(shown.text.ends_with(ELLIPSIS));
    }
}
