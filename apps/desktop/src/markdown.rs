use std::ops::Range;

use pulldown_cmark::{Event, Options, Parser, Tag};

use crate::title::{find_links, link_target};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Inline {
    Text(String),
    Code(String),
    Strong(Vec<Inline>),
    Em(Vec<Inline>),
    Del(Vec<Inline>),
    Link { href: String, children: Vec<Inline> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        children: Vec<Inline>,
    },
    Code(String),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Vec<Block>>,
    },
    Rule,
}

fn opens(url: &str) -> bool {
    let scheme = url.to_ascii_lowercase();
    (scheme.starts_with("http://") || scheme.starts_with("https://")) && link_target(url).is_some()
}

fn linked(href: &str, label: Vec<Inline>) -> Inline {
    let children = if label.is_empty() {
        vec![Inline::Text(href.to_owned())]
    } else {
        label
    };
    Inline::Link {
        href: href.to_owned(),
        children,
    }
}

fn push(out: &mut Vec<Inline>, node: Inline) {
    match (out.last_mut(), node) {
        (Some(Inline::Text(last)), Inline::Text(text)) => last.push_str(&text),
        (_, Inline::Text(text)) if text.is_empty() => {}
        (_, node) => out.push(node),
    }
}

fn with_bare_links(nodes: Vec<Inline>) -> Vec<Inline> {
    let mut out = Vec::new();
    for node in nodes {
        let Inline::Text(text) = node else {
            out.push(node);
            continue;
        };
        let mut at = 0;
        for link in find_links(&text) {
            if link.start > at {
                out.push(Inline::Text(text[at..link.start].to_owned()));
            }
            out.push(linked(&link.url, Vec::new()));
            at = link.end;
        }
        if at < text.len() {
            out.push(Inline::Text(text[at..].to_owned()));
        }
    }
    out
}

fn as_written(text: &str) -> Block {
    Block::Paragraph(with_bare_links(vec![Inline::Text(text.to_owned())]))
}

fn ends_paragraph(event: &Event) -> bool {
    matches!(
        event,
        Event::Rule
            | Event::End(_)
            | Event::Start(
                Tag::Paragraph
                    | Tag::Heading { .. }
                    | Tag::CodeBlock(_)
                    | Tag::HtmlBlock
                    | Tag::BlockQuote(_)
                    | Tag::List(_)
            )
    )
}

struct Reader<'a> {
    source: &'a str,
    events: Vec<(Event<'a>, Range<usize>)>,
    at: usize,
}

impl<'a> Reader<'a> {
    fn peek(&self) -> Option<(Event<'a>, Range<usize>)> {
        self.events.get(self.at).cloned()
    }

    fn next(&mut self) -> Option<(Event<'a>, Range<usize>)> {
        let event = self.peek()?;
        self.at += 1;
        Some(event)
    }

    fn text_until_end(&mut self) -> String {
        let mut text = String::new();
        let mut depth = 0;
        while let Some((event, _)) = self.next() {
            match event {
                Event::Start(_) => depth += 1,
                Event::End(_) if depth == 0 => break,
                Event::End(_) => depth -= 1,
                Event::Text(part) | Event::Code(part) | Event::Html(part) => text.push_str(&part),
                _ => {}
            }
        }
        text
    }

    fn link(
        &self,
        out: &mut Vec<Inline>,
        in_link: bool,
        url: &str,
        label: Vec<Inline>,
        range: Range<usize>,
    ) {
        if in_link || url.is_empty() {
            for node in label {
                push(out, node);
            }
        } else if opens(url) {
            out.push(linked(url, label));
        } else {
            push(out, Inline::Text(self.source[range].to_owned()));
        }
    }

    fn inlines(&mut self, in_link: bool, implicit: bool) -> Vec<Inline> {
        let mut out = Vec::new();
        while let Some((event, range)) = self.peek() {
            if implicit && ends_paragraph(&event) {
                break;
            }
            self.at += 1;
            match event {
                Event::End(_) => break,
                Event::Text(text) | Event::InlineHtml(text) | Event::Html(text) => {
                    push(&mut out, Inline::Text(text.into_string()));
                }
                Event::SoftBreak | Event::HardBreak => push(&mut out, Inline::Text("\n".into())),
                Event::Code(text) => out.push(Inline::Code(text.into_string())),
                Event::Start(Tag::Strong) => {
                    let children = self.inlines(in_link, false);
                    out.push(Inline::Strong(children));
                }
                Event::Start(Tag::Emphasis) => {
                    let children = self.inlines(in_link, false);
                    out.push(Inline::Em(children));
                }
                Event::Start(Tag::Strikethrough) => {
                    let children = self.inlines(in_link, false);
                    if self.source[range].starts_with("~~") {
                        out.push(Inline::Del(children));
                    } else {
                        push(&mut out, Inline::Text("~".into()));
                        for node in children {
                            push(&mut out, node);
                        }
                        push(&mut out, Inline::Text("~".into()));
                    }
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    let label = self.inlines(true, false);
                    self.link(&mut out, in_link, &dest_url, label, range);
                }
                Event::Start(Tag::Image { dest_url, .. }) => {
                    let alt = self.text_until_end();
                    let label = if alt.is_empty() {
                        Vec::new()
                    } else {
                        vec![Inline::Text(alt)]
                    };
                    self.link(&mut out, in_link, &dest_url, label, range);
                }
                _ => push(&mut out, Inline::Text(self.source[range].to_owned())),
            }
        }
        if in_link { out } else { with_bare_links(out) }
    }

    fn blocks(&mut self) -> Vec<Block> {
        let mut out = Vec::new();
        while let Some((event, range)) = self.next() {
            match event {
                Event::End(_) => break,
                Event::Start(Tag::Paragraph) => {
                    out.push(Block::Paragraph(self.inlines(false, false)));
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let children = self.inlines(false, false);
                    out.push(if children.is_empty() {
                        as_written(self.source[range].trim())
                    } else {
                        Block::Heading {
                            level: level as u8,
                            children,
                        }
                    });
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    let mut text = self.text_until_end();
                    if text.ends_with('\n') {
                        text.pop();
                    }
                    out.push(Block::Code(text));
                }
                Event::Start(Tag::HtmlBlock) => {
                    let text = self.text_until_end();
                    out.push(as_written(text.trim_end()));
                }
                Event::Start(Tag::BlockQuote(_)) => out.push(Block::Quote(self.blocks())),
                Event::Start(Tag::List(start)) => {
                    let mut items = Vec::new();
                    while let Some((Event::Start(Tag::Item), _)) = self.next() {
                        items.push(self.blocks());
                    }
                    out.push(Block::List { start, items });
                }
                Event::Rule => out.push(Block::Rule),
                _ => {
                    self.at -= 1;
                    out.push(Block::Paragraph(self.inlines(false, true)));
                }
            }
        }
        out
    }
}

#[must_use]
pub fn markdown(source: &str) -> Vec<Block> {
    let events = Parser::new_ext(source, Options::ENABLE_STRIKETHROUGH)
        .into_offset_iter()
        .collect();
    let shown = Reader {
        source,
        events,
        at: 0,
    }
    .blocks();
    if shown.is_empty() {
        vec![as_written(source.trim())]
    } else {
        shown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(value: &str) -> Inline {
        Inline::Text(value.to_owned())
    }

    fn code(value: &str) -> Inline {
        Inline::Code(value.to_owned())
    }

    fn link(href: &str, children: Vec<Inline>) -> Inline {
        linked(href, children)
    }

    fn p(children: Vec<Inline>) -> Block {
        Block::Paragraph(children)
    }

    fn list(start: Option<u64>, items: Vec<Vec<Block>>) -> Block {
        Block::List { start, items }
    }

    #[test]
    fn keeps_every_line_break_and_run_of_spaces_inside_a_paragraph_so_a_log_pasted_without_markdown_in_mind_reads_as_it_was_written()
     {
        assert_eq!(
            markdown("built  ok\ntests   2 failed"),
            [p(vec![text("built  ok\ntests   2 failed")])]
        );
    }

    #[test]
    fn formats_bold_italic_struck_and_code_text_the_marks_an_assistant_writes_a_summary_with() {
        assert_eq!(
            markdown("**done** and *maybe*, ~~dropped~~, run `vp test`"),
            [p(vec![
                Inline::Strong(vec![text("done")]),
                text(" and "),
                Inline::Em(vec![text("maybe")]),
                text(", "),
                Inline::Del(vec![text("dropped")]),
                text(", run "),
                code("vp test"),
            ])]
        );
    }

    #[test]
    fn formats_marks_that_touch_japanese_text_which_has_no_spaces_to_set_them_apart() {
        assert_eq!(
            markdown("これは**太字**です"),
            [p(vec![
                text("これは"),
                Inline::Strong(vec![text("太字")]),
                text("です"),
            ])]
        );
    }

    #[test]
    fn leaves_a_single_tilde_alone_so_a_range_such_as_10_to_20_is_not_struck_through() {
        assert_eq!(
            markdown("10~20 items, 30~40 at most and ~one~ **~two~**"),
            [p(vec![
                text("10~20 items, 30~40 at most and ~one~ "),
                Inline::Strong(vec![text("~two~")]),
            ])]
        );
    }

    #[test]
    fn nests_lists_and_starts_a_numbered_list_at_the_number_written_so_steps_keep_their_order_when_a_comment_continues_an_earlier_one()
     {
        assert_eq!(
            markdown("3. build\n   - web\n   - cli\n4. release"),
            [list(
                Some(3),
                vec![
                    vec![
                        p(vec![text("build")]),
                        list(
                            None,
                            vec![vec![p(vec![text("web")])], vec![p(vec![text("cli")])]]
                        ),
                    ],
                    vec![p(vec![text("release")])],
                ]
            )]
        );
    }

    #[test]
    fn reads_a_list_whose_items_are_set_apart_by_blank_lines_the_same_as_one_written_tight() {
        assert_eq!(
            markdown("- a\n\n- b\n\n  more"),
            [list(
                None,
                vec![
                    vec![p(vec![text("a")])],
                    vec![p(vec![text("b")]), p(vec![text("more")])],
                ]
            )]
        );
    }

    #[test]
    fn shows_a_fenced_block_exactly_as_written_so_pasted_output_keeps_its_indentation_and_its_asterisks()
     {
        assert_eq!(
            markdown("```sh\n$ ls  *.ts\n  a.ts  **b.ts**\n```"),
            [Block::Code("$ ls  *.ts\n  a.ts  **b.ts**".to_owned())]
        );
    }

    #[test]
    fn reads_headings_quotes_and_rules_so_a_long_comment_can_be_divided_into_parts() {
        assert_eq!(
            markdown("## Plan\n\n> wait for the review\n\n---"),
            [
                Block::Heading {
                    level: 2,
                    children: vec![text("Plan")],
                },
                Block::Quote(vec![p(vec![text("wait for the review")])]),
                Block::Rule,
            ]
        );
    }

    #[test]
    fn shows_a_link_as_its_label_with_the_labels_own_formatting_and_opens_the_address_behind_it() {
        assert_eq!(
            markdown("see [the **pull** request](https://example.com/pull/7) first"),
            [p(vec![
                text("see "),
                link(
                    "https://example.com/pull/7",
                    vec![
                        text("the "),
                        Inline::Strong(vec![text("pull")]),
                        text(" request"),
                    ]
                ),
                text(" first"),
            ])]
        );
    }

    #[test]
    fn turns_a_url_written_on_its_own_into_a_link_that_shows_it_as_written_so_a_comment_that_is_only_a_link_can_still_be_opened()
     {
        assert_eq!(
            markdown("https://example.com/a_b_c"),
            [p(vec![link("https://example.com/a_b_c", vec![])])]
        );
        assert_eq!(
            markdown("<https://example.org> and **https://example.net/x**"),
            [p(vec![
                link("https://example.org", vec![]),
                text(" and "),
                Inline::Strong(vec![link("https://example.net/x", vec![])]),
            ])]
        );
    }

    #[test]
    fn ends_a_url_written_on_its_own_where_japanese_text_starts_so_a_sentence_that_goes_on_after_it_is_not_opened_as_part_of_the_address()
     {
        assert_eq!(
            markdown("詳細はhttps://example.com/を参照"),
            [p(vec![
                text("詳細は"),
                link("https://example.com/", vec![]),
                text("を参照"),
            ])]
        );
    }

    #[test]
    fn leaves_a_url_inside_code_or_inside_a_links_label_as_text_so_code_reads_as_written_and_one_link_is_never_nested_in_another()
     {
        assert_eq!(
            markdown(
                "`curl https://example.com/api` or [see https://example.com/a](https://example.com/b)"
            ),
            [p(vec![
                code("curl https://example.com/api"),
                text(" or "),
                link(
                    "https://example.com/b",
                    vec![text("see https://example.com/a")]
                ),
            ])]
        );
    }

    #[test]
    fn keeps_a_picture_inside_a_link_as_its_description_so_one_link_is_never_nested_in_another() {
        assert_eq!(
            markdown("[![logo](https://example.com/logo.png) home](https://example.com)"),
            [p(vec![link(
                "https://example.com",
                vec![text("logo home")]
            )])]
        );
    }

    #[test]
    fn opens_the_address_a_link_names_when_it_is_written_with_character_references() {
        assert_eq!(
            markdown("AT&amp;T [search](https://example.com/?q=1&amp;page=2)"),
            [p(vec![
                text("AT&T "),
                link("https://example.com/?q=1&page=2", vec![text("search")]),
            ])]
        );
    }

    #[test]
    fn leaves_a_link_that_is_not_http_or_https_as_written_so_nothing_in_a_comment_runs_a_script_or_opens_another_app()
     {
        let written = "[run](javascript:alert(1)), [mail](mailto:a@example.com), [here](/settings) and <a@example.com>";
        assert_eq!(markdown(written), [p(vec![text(written)])]);
    }

    #[test]
    fn shows_only_the_label_of_a_link_with_no_address_so_nothing_that_looks_like_a_link_opens_nowhere()
     {
        assert_eq!(
            markdown("[later]() and ![chart]()"),
            [p(vec![text("later and chart")])]
        );
    }

    #[test]
    fn shows_the_address_of_a_link_with_no_label_so_no_link_is_too_small_to_see_or_click() {
        assert_eq!(
            markdown("![](https://example.com/plain.png) and [](https://example.net/b)"),
            [p(vec![
                link("https://example.com/plain.png", vec![]),
                text(" and "),
                link("https://example.net/b", vec![]),
            ])]
        );
    }

    #[test]
    fn shows_html_tables_and_task_boxes_as_written_since_they_are_not_drawn() {
        assert_eq!(
            markdown(
                "<b>bold</b>\n\n<script>alert(1)</script>\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n- [ ] later"
            ),
            [
                p(vec![text("<b>bold</b>")]),
                p(vec![text("<script>alert(1)</script>")]),
                p(vec![text("| a | b |\n|---|---|\n| 1 | 2 |")]),
                list(None, vec![vec![p(vec![text("[ ] later")])]]),
            ]
        );
    }

    #[test]
    fn turns_a_picture_into_a_link_to_it_labelled_with_its_description_since_comments_do_not_load_pictures()
     {
        assert_eq!(
            markdown("![the chart](https://example.com/chart.png)"),
            [p(vec![link(
                "https://example.com/chart.png",
                vec![text("the chart")]
            )])]
        );
    }

    #[test]
    fn follows_a_reference_to_the_address_defined_for_it_elsewhere_in_the_comment() {
        assert_eq!(
            markdown("see [the doc][1] today\n\n[1]: https://example.com/doc"),
            [p(vec![
                text("see "),
                link("https://example.com/doc", vec![text("the doc")]),
                text(" today"),
            ])]
        );
    }

    #[test]
    fn shows_a_comment_that_is_only_a_link_definition_or_an_empty_heading_as_written_so_no_comment_is_ever_blank()
     {
        assert_eq!(
            markdown("[1]: https://example.com/doc"),
            [p(vec![
                text("[1]: "),
                link("https://example.com/doc", vec![]),
            ])]
        );
        assert_eq!(markdown("#"), [p(vec![text("#")])]);
    }
}
