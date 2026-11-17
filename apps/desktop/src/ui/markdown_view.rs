use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, ClickEvent, Div, Font, FontStyle, FontWeight, HighlightStyle, Hsla, SharedString,
    StrikethroughStyle, StyledText, TextRun, UnderlineStyle, div, prelude::*, px,
};

use nestodo_desktop::markdown::{Block, Inline, markdown};
use nestodo_desktop::title::{highlighted, link_target};

use crate::ui::clamped_text::{ClampedText, TextHits};
use crate::ui::theme::Theme;

const CODE_FONT: &str = "Menlo";

#[derive(Clone)]
struct Marks {
    font: Font,
    struck: bool,
    code: bool,
    linked: bool,
}

#[derive(Default)]
struct Shaped {
    text: String,
    runs: Vec<TextRun>,
    links: Vec<(Range<usize>, String)>,
}

struct View<'a> {
    id: &'a str,
    query: &'a str,
    font: &'a Font,
    colors: &'a Theme,
    parts: usize,
}

impl View<'_> {
    fn part(&mut self) -> SharedString {
        self.parts += 1;
        format!("{}-{}", self.id, self.parts).into()
    }

    fn marks(&self) -> Marks {
        Marks {
            font: self.font.clone(),
            struck: false,
            code: false,
            linked: false,
        }
    }

    fn leaf(&self, text: &str, marks: &Marks, base: Hsla, out: &mut Shaped) {
        let color = if marks.linked {
            self.colors.accent
        } else {
            base
        };
        for part in highlighted(text, self.query) {
            out.text.push_str(&part.text);
            out.runs.push(TextRun {
                len: part.text.len(),
                font: marks.font.clone(),
                color,
                background_color: if part.matched {
                    Some(self.colors.matched)
                } else {
                    marks.code.then_some(self.colors.accent_soft)
                },
                underline: marks.linked.then_some(UnderlineStyle {
                    thickness: px(1.),
                    color: Some(color),
                    wavy: false,
                }),
                strikethrough: marks.struck.then_some(StrikethroughStyle {
                    thickness: px(1.),
                    color: Some(color),
                }),
            });
        }
    }

    fn styled(&self, nodes: &[Inline], marks: &Marks, base: Hsla, out: &mut Shaped) {
        for node in nodes {
            let mut inner = marks.clone();
            match node {
                Inline::Text(text) => self.leaf(text, marks, base, out),
                Inline::Code(text) => {
                    inner.font.family = CODE_FONT.into();
                    inner.code = true;
                    self.leaf(text, &inner, base, out);
                }
                Inline::Strong(children) => {
                    inner.font.weight = FontWeight::BOLD;
                    self.styled(children, &inner, base, out);
                }
                Inline::Em(children) => {
                    inner.font.style = FontStyle::Italic;
                    self.styled(children, &inner, base, out);
                }
                Inline::Del(children) => {
                    inner.struck = true;
                    self.styled(children, &inner, base, out);
                }
                Inline::Link { href, children } => {
                    inner.linked = true;
                    let start = out.text.len();
                    self.styled(children, &inner, base, out);
                    out.links.push((start..out.text.len(), href.clone()));
                }
            }
        }
    }

    fn text_block(&mut self, children: &[Inline], marks: &Marks, base: Hsla) -> AnyElement {
        let mut styled = Shaped::default();
        self.styled(children, marks, base, &mut styled);
        let hits = TextHits::default();
        let shown = ClampedText::new(styled.text.into(), styled.runs, None, base, hits.clone());
        let links = Rc::new(styled.links);
        let id = self.part();
        let selector = id.to_string();
        div()
            .id(id)
            .debug_selector(move || selector)
            .on_click(move |event: &ClickEvent, _, cx| {
                let clicked = hits
                    .index_for(event.position())
                    .and_then(|index| links.iter().find(|(range, _)| range.contains(&index)));
                if let Some(target) = clicked.and_then(|(_, href)| link_target(href)) {
                    cx.open_url(&target);
                }
            })
            .child(shown)
            .into_any_element()
    }

    fn code_block(&mut self, text: &str) -> AnyElement {
        let matched = self.colors.matched;
        let lines: Vec<Div> = text
            .split('\n')
            .map(|line| {
                let shown = if line.is_empty() {
                    " ".to_owned()
                } else {
                    line.replace('\t', "    ")
                };
                let highlights: Vec<_> = highlighted(&shown, self.query)
                    .into_iter()
                    .filter(|part| part.matched)
                    .map(|part| {
                        let style = HighlightStyle {
                            background_color: Some(matched),
                            ..HighlightStyle::default()
                        };
                        (part.from..part.to, style)
                    })
                    .collect();
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(StyledText::new(shown).with_highlights(highlights))
            })
            .collect();
        let id = self.part();
        let selector = id.to_string();
        let written = format!("{id}-lines");
        div()
            .id(id)
            .debug_selector(move || selector)
            .overflow_x_scroll()
            .restrict_scroll_to_axis()
            .flex()
            .flex_col()
            .items_start()
            .px(px(12.))
            .py(px(8.))
            .rounded(px(10.))
            .bg(self.colors.accent_soft)
            .font_family(CODE_FONT)
            .text_size(px(13.))
            .line_height(px(20.))
            .child(
                div()
                    .debug_selector(move || written)
                    .flex_none()
                    .flex()
                    .flex_col()
                    .children(lines),
            )
            .into_any_element()
    }

    fn list(
        &mut self,
        start: Option<u64>,
        items: &[Vec<Block>],
        base: Hsla,
        depth: usize,
    ) -> AnyElement {
        let mut list = div().flex().flex_col();
        let mut number = start;
        for item in items {
            let marker: SharedString = match number {
                Some(n) => format!("{n}.").into(),
                None if depth == 0 => "\u{2022}".into(),
                None => "\u{25e6}".into(),
            };
            number = number.map(|n| n + 1);
            list = list.child(
                div()
                    .flex()
                    .child(
                        div()
                            .flex_none()
                            .min_w(px(20.))
                            .pr(px(6.))
                            .text_color(self.colors.muted)
                            .child(marker),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(self.blocks(item, base, depth + 1)),
                    ),
            );
        }
        list.into_any_element()
    }

    fn blocks(&mut self, blocks: &[Block], base: Hsla, depth: usize) -> Div {
        let mut column = div().flex().flex_col();
        for (index, block) in blocks.iter().enumerate() {
            let hugs = index == 0 || (depth > 0 && matches!(block, Block::List { .. }));
            let shown = match block {
                Block::Paragraph(children) => self.text_block(children, &self.marks(), base),
                Block::Heading { level, children } => {
                    let mut bold = self.marks();
                    bold.font.weight = FontWeight::BOLD;
                    let heading = div().child(self.text_block(children, &bold, base));
                    match level {
                        1 => heading.text_size(px(17.)).line_height(px(24.)),
                        2 => heading.text_size(px(16.)).line_height(px(23.)),
                        _ => heading,
                    }
                    .into_any_element()
                }
                Block::Code(text) => self.code_block(text),
                Block::Quote(children) => div()
                    .pl(px(12.))
                    .border_l(px(3.))
                    .border_color(self.colors.guide)
                    .child(self.blocks(children, self.colors.muted, depth))
                    .into_any_element(),
                Block::List { start, items } => self.list(*start, items, base, depth),
                Block::Rule => div().h(px(1.)).bg(self.colors.border).into_any_element(),
            };
            column = column.child(div().when(!hugs, |gap| gap.mt(px(8.))).child(shown));
        }
        column
    }
}

pub fn markdown_view(id: &str, body: &str, query: &str, font: &Font, colors: &Theme) -> AnyElement {
    let mut view = View {
        id,
        query,
        font,
        colors,
        parts: 0,
    };
    view.blocks(&markdown(body), colors.text, 0)
        .text_size(px(14.5))
        .line_height(px(24.))
        .text_color(colors.text)
        .into_any_element()
}
