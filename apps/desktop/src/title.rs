use crate::outline::caret_in;
use std::fmt::Write;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub start: usize,
    pub end: usize,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub text: String,
    pub from: usize,
    pub to: usize,
    pub matched: bool,
    pub href: Option<String>,
}

#[derive(Default)]
struct UrlEnd {
    parens: usize,
    brackets: usize,
}

impl UrlEnd {
    fn at(&mut self, ch: char) -> bool {
        if !('!'..='~').contains(&ch) || matches!(ch, '"' | '`' | '<' | '>') {
            return true;
        }
        let depth = match ch {
            '(' => {
                self.parens += 1;
                return false;
            }
            '[' => {
                self.brackets += 1;
                return false;
            }
            ')' => &mut self.parens,
            ']' => &mut self.brackets,
            _ => return false,
        };
        if *depth == 0 {
            return true;
        }
        *depth -= 1;
        false
    }
}

fn scheme_len(text: &str) -> Option<usize> {
    ["https://", "http://"]
        .into_iter()
        .find(|scheme| {
            text.get(..scheme.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(scheme))
        })
        .map(str::len)
}

fn host_of(url: &str) -> &str {
    let after = url.find("://").map_or(2, |at| at + 3);
    let rest = &url[caret_in(url, after)..];
    let authority = rest.split(['/', '\\', '?', '#']).next().unwrap_or_default();
    authority.rsplit('@').next().unwrap_or_default()
}

#[must_use]
pub fn link_label(url: &str) -> String {
    let host = host_of(url).to_lowercase();
    match host.strip_prefix("www.") {
        Some(rest)
            if rest
                .starts_with(|ch: char| !matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')) =>
        {
            rest.to_owned()
        }
        _ => host,
    }
}

#[must_use]
pub fn link_target(url: &str) -> Option<String> {
    url::Url::parse(url).ok().map(String::from)
}

#[must_use]
pub fn find_links(title: &str) -> Vec<Link> {
    let mut links: Vec<Link> = Vec::new();
    let mut from = 0;
    while let Some(offset) = title[from..].find(['h', 'H']) {
        let start = from + offset;
        let Some(scheme) = scheme_len(&title[start..]) else {
            from = start + 1;
            continue;
        };
        from = start + scheme;
        if start < links.last().map_or(0, |link| link.end) {
            continue;
        }
        let mut ends = UrlEnd::default();
        let end = title[from..]
            .char_indices()
            .find(|&(_, ch)| ends.at(ch))
            .map_or(title.len(), |(at, _)| from + at);
        let url = &title[start..end];
        if !host_of(url).is_empty() {
            links.push(Link {
                start,
                end,
                url: url.to_owned(),
            });
        }
    }
    links
}

fn is_js_space(ch: char) -> bool {
    matches!(
        ch,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

fn percent_encode(ch: char, out: &mut String) {
    let mut buf = [0; 4];
    for byte in ch.encode_utf8(&mut buf).bytes() {
        let _ = write!(out, "%{byte:02X}");
    }
}

fn ascii_host(server: &str) -> Option<String> {
    if server.is_ascii() {
        return Some(server.to_owned());
    }
    let (host, port) = server
        .rfind(':')
        .map_or((server, ""), |colon| server.split_at(colon));
    let parsed = url::Url::parse(&format!("http://{host}")).ok()?;
    Some(format!("{}{port}", parsed.host_str()?))
}

pub fn encode_pasted_url(text: &str) -> Option<String> {
    let pasted = text.trim_matches(is_js_space);
    let scheme = &pasted[..scheme_len(pasted)?];
    if pasted.contains(is_js_space) {
        return None;
    }
    let rest = &pasted[scheme.len()..];
    let head = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
    let at = head.rfind('@').map_or(0, |at| at + 1);
    let server = ascii_host(&head[at..]).filter(|server| !server.is_empty())?;
    let mut ends = UrlEnd::default();
    let mut url = scheme.to_owned();
    for ch in head[..at]
        .chars()
        .chain(server.chars())
        .chain(rest[head.len()..].chars())
    {
        if ends.at(ch) {
            percent_encode(ch, &mut url);
        } else {
            url.push(ch);
        }
    }
    Some(url)
}

struct Lowered {
    text: String,
    starts: Vec<(usize, usize)>,
    len: usize,
}

impl Lowered {
    fn new(original: &str) -> Lowered {
        let mut lowered = 0;
        let starts = original
            .char_indices()
            .map(|(at, ch)| {
                let start = (lowered, at);
                lowered += ch.to_lowercase().map(char::len_utf8).sum::<usize>();
                start
            })
            .collect();
        Lowered {
            text: original.to_lowercase(),
            starts,
            len: original.len(),
        }
    }

    fn lowered_at(&self, original: usize) -> usize {
        let i = self.starts.partition_point(|&(_, at)| at < original);
        self.starts
            .get(i)
            .map_or(self.text.len(), |&(lowered, _)| lowered)
    }

    fn original_floor(&self, lowered: usize) -> usize {
        let i = self.starts.partition_point(|&(at, _)| at <= lowered);
        self.starts
            .get(i.saturating_sub(1))
            .map_or(0, |&(_, at)| at)
    }

    fn original_ceil(&self, lowered: usize) -> usize {
        let i = self.starts.partition_point(|&(at, _)| at < lowered);
        self.starts.get(i).map_or(self.len, |&(_, at)| at)
    }

    fn find(&self, q: &str, from: usize) -> Option<(usize, usize)> {
        let start = self.lowered_at(from);
        let found = start + self.text.get(start..)?.find(q)?;
        Some((
            self.original_floor(found),
            self.original_ceil(found + q.len()),
        ))
    }
}

fn plain(title: &str, from: usize, to: usize, q: &str, parts: &mut Vec<Part>) {
    let text = &title[from..to];
    let mut push = |start: usize, end: usize, matched: bool| {
        parts.push(Part {
            text: text[start..end].to_owned(),
            from: from + start,
            to: from + end,
            matched,
            href: None,
        });
    };
    let mut rest = 0;
    if !q.is_empty() {
        let lowered = Lowered::new(text);
        while let Some((start, end)) = lowered.find(q, rest) {
            if start > rest {
                push(rest, start, false);
            }
            push(start, end, true);
            rest = end;
        }
    }
    if rest < text.len() {
        push(rest, text.len(), false);
    }
}

#[must_use]
pub fn tokens(title: &str, query: &str) -> Vec<Part> {
    let q = query.trim().to_lowercase();
    let mut parts = Vec::new();
    let mut at = 0;
    for link in find_links(title) {
        plain(title, at, link.start, &q, &mut parts);
        parts.push(Part {
            text: link_label(&link.url),
            from: link.start,
            to: link.end,
            matched: !q.is_empty() && link.url.to_lowercase().contains(&q),
            href: Some(link.url),
        });
        at = link.end;
    }
    plain(title, at, title.len(), &q, &mut parts);
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn urls(title: &str) -> Vec<String> {
        find_links(title).into_iter().map(|link| link.url).collect()
    }

    fn class(part: &Part) -> Option<&'static str> {
        match (part.href.is_some(), part.matched) {
            (false, false) => None,
            (false, true) => Some("match"),
            (true, false) => Some("link"),
            (true, true) => Some("link match"),
        }
    }

    fn assert_covers(title: &str, parts: &[Part]) {
        assert_eq!(parts.first().map(|part| part.from), Some(0));
        assert_eq!(parts.last().map(|part| part.to), Some(title.len()));
        for pair in parts.windows(2) {
            assert_eq!(pair[1].from, pair[0].to);
        }
        for part in parts.iter().filter(|part| part.href.is_none()) {
            assert_eq!(title[part.from..part.to], part.text);
        }
    }

    mod find_links {
        use super::*;

        #[test]
        fn ends_a_link_at_full_width_punctuation_and_typographic_quotes_so_text_written_right_after_a_url_is_not_opened_as_part_of_it()
         {
            assert_eq!(
                urls("PR: https://example.com/project/pull/296（two commits）"),
                ["https://example.com/project/pull/296"]
            );
            assert_eq!(
                urls("«preview: http://localhost:6420»: done"),
                ["http://localhost:6420"]
            );
            assert_eq!(
                urls("„https://example.com/pull/26“ (`Closes #25`)"),
                ["https://example.com/pull/26"]
            );
        }

        #[test]
        fn ends_a_link_at_the_first_chinese_japanese_or_korean_character_so_no_space_is_needed_after_a_url()
         {
            assert_eq!(
                find_links("詳細はhttps://example.com/を参照"),
                [Link {
                    start: "詳細は".len(),
                    end: "詳細はhttps://example.com/".len(),
                    url: "https://example.com/".to_owned()
                }]
            );
            assert_eq!(
                urls("详情见https://example.com/。"),
                ["https://example.com/"]
            );
            assert_eq!(
                urls("자세한 내용은 https://example.com/에서 확인"),
                ["https://example.com/"]
            );
        }

        #[test]
        fn keeps_trailing_punctuation_because_real_urls_end_with_it() {
            assert_eq!(
                urls("https://en.wikipedia.org/wiki/Washington,_D.C."),
                ["https://en.wikipedia.org/wiki/Washington,_D.C."]
            );
            assert_eq!(
                urls("https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82"),
                ["https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82"]
            );
        }

        #[test]
        fn keeps_brackets_that_pair_up_inside_the_url_but_not_a_closing_bracket_that_wraps_it() {
            assert_eq!(
                urls("(https://en.wikipedia.org/wiki/Mercury_(planet))"),
                ["https://en.wikipedia.org/wiki/Mercury_(planet)"]
            );
            assert_eq!(
                urls("[https://example.com/a[1]]"),
                ["https://example.com/a[1]"]
            );
        }

        #[test]
        fn ends_a_link_at_quotes_backticks_and_angle_brackets_which_cannot_appear_in_a_url() {
            assert_eq!(
                urls("\"https://example.com\" and <https://example.org>"),
                ["https://example.com", "https://example.org"]
            );
            assert_eq!(
                urls("Spec: `https://developer.mozilla.org/en-US/docs/Web/API/URL/`"),
                ["https://developer.mozilla.org/en-US/docs/Web/API/URL/"]
            );
        }

        #[test]
        fn treats_a_url_inside_another_urls_query_as_part_of_it() {
            assert_eq!(
                urls("https://a.example/login?next=https://b.example/"),
                ["https://a.example/login?next=https://b.example/"]
            );
        }

        #[test]
        fn ignores_a_scheme_with_no_host_which_cannot_be_opened() {
            assert_eq!(urls("https:// and https:///path"), Vec::<String>::new());
        }

        #[test]
        fn finds_a_scheme_in_any_letter_case() {
            assert_eq!(urls("HTTPS://Example.com"), ["HTTPS://Example.com"]);
        }
    }

    mod link_label {
        use super::*;

        #[test]
        fn shows_only_the_host_so_a_long_url_does_not_push_the_rest_of_the_title_out_of_the_row() {
            assert_eq!(
                link_label("https://www.example.com:8080/very/long/path?q=1#top"),
                "example.com:8080"
            );
            assert_eq!(link_label("https://www."), "www.");
        }

        #[test]
        fn shows_the_host_the_browser_will_open_so_text_before_an_at_or_a_backslash_cannot_disguise_where_a_link_goes()
         {
            assert_eq!(
                link_label("https://bank.example@evil.example/login"),
                "evil.example"
            );
            assert_eq!(
                link_label("https://user:pass@www.example.com:8080/"),
                "example.com:8080"
            );
            assert_eq!(
                link_label("https://evil.example\\@bank.example/"),
                "evil.example"
            );
        }

        #[test]
        fn shows_the_host_in_lower_case_as_the_browser_does_so_a_capital_i_cannot_pass_for_a_lower_case_l()
         {
            assert_eq!(link_label("https://paypaI.example/"), "paypai.example");
        }
    }

    mod link_target {
        use super::*;

        #[test]
        fn opens_the_host_the_chip_shows_even_where_macos_would_read_a_backslash_as_part_of_the_user_name()
         {
            for (url, opened) in [
                (
                    "https://evil.example\\@bank.example/",
                    "https://evil.example/@bank.example/",
                ),
                (
                    "https://bank.example\\@evil.example/login",
                    "https://bank.example/@evil.example/login",
                ),
                ("https://example.com\\guide", "https://example.com/guide"),
            ] {
                assert_eq!(link_target(url).as_deref(), Some(opened));
                assert_eq!(
                    url::Url::parse(opened).unwrap().host_str(),
                    Some(link_label(url).as_str())
                );
            }
        }
    }

    mod encode_pasted_url {
        use super::*;

        #[test]
        fn percent_encodes_a_pasted_url_with_letters_outside_ascii_so_it_stays_one_link_even_when_it_ends_with_one_such_as_maru()
         {
            let pasted = [
                (
                    "https://example.com/ありがとう。",
                    "https://example.com/%E3%81%82%E3%82%8A%E3%81%8C%E3%81%A8%E3%81%86%E3%80%82",
                ),
                (
                    "https://zh.wikipedia.org/wiki/北京",
                    "https://zh.wikipedia.org/wiki/%E5%8C%97%E4%BA%AC",
                ),
                (
                    "https://ko.wikipedia.org/wiki/서울",
                    "https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8",
                ),
                (
                    "https://de.wikipedia.org/wiki/Fuß",
                    "https://de.wikipedia.org/wiki/Fu%C3%9F",
                ),
            ];
            for (raw, encoded) in pasted {
                assert_eq!(encode_pasted_url(raw).as_deref(), Some(encoded));
                assert_eq!(urls(encoded), [encoded]);
            }
        }

        #[test]
        fn leaves_an_already_encoded_url_as_it_is_so_pasting_it_twice_does_not_double_encode_it() {
            assert_eq!(
                encode_pasted_url("https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8").as_deref(),
                Some("https://ko.wikipedia.org/wiki/%EC%84%9C%EC%9A%B8")
            );
        }

        #[test]
        fn converts_a_domain_outside_ascii_to_punycode_and_keeps_its_port() {
            assert_eq!(
                encode_pasted_url("https://日本語.jp:8080/パス").as_deref(),
                Some("https://xn--wgv71a119e.jp:8080/%E3%83%91%E3%82%B9")
            );
            assert_eq!(
                encode_pasted_url("https://münchen.de/straße").as_deref(),
                Some("https://xn--mnchen-3ya.de/stra%C3%9Fe")
            );
            assert_eq!(
                encode_pasted_url("https://한국.kr/").as_deref(),
                Some("https://xn--3e0b707e.kr/")
            );
        }

        #[test]
        fn drops_the_whitespace_a_copied_url_often_carries() {
            assert_eq!(
                encode_pasted_url("  https://example.com/北京\n").as_deref(),
                Some("https://example.com/%E5%8C%97%E4%BA%AC")
            );
        }

        #[test]
        fn encodes_characters_that_would_otherwise_end_the_link_early_so_the_whole_pasted_url_opens()
         {
            for pasted in [
                "https://example.com/a)b",
                "https://example.com/a))(b)",
                "https://example.com/\"x\"",
                "https://example.com/😀",
            ] {
                let url = encode_pasted_url(pasted).unwrap();
                assert_eq!(urls(&url), [url.as_str()]);
            }
            assert_eq!(
                encode_pasted_url("https://example.com/a))(b)").as_deref(),
                Some("https://example.com/a%29%29(b)")
            );
            assert_eq!(
                encode_pasted_url("https://example.com/😀").as_deref(),
                Some("https://example.com/%F0%9F%98%80")
            );
        }

        #[test]
        fn leaves_anything_but_a_single_url_alone_because_its_extent_cannot_be_known() {
            for pasted in [
                "München https://example.com/",
                "https://example.com/a b",
                "ftp://example.com/",
                "https://",
                "plain text",
            ] {
                assert_eq!(encode_pasted_url(pasted), None, "{pasted}");
            }
        }
    }

    mod tokens {
        use super::*;

        #[test]
        fn shows_a_url_as_its_host_but_opens_the_whole_url_fragment_included() {
            let parts = tokens("read https://example.com/page#section", "");
            let shown: Vec<_> = parts
                .iter()
                .map(|part| (part.text.as_str(), class(part), part.href.as_deref()))
                .collect();
            assert_eq!(
                shown,
                [
                    ("read ", None, None),
                    (
                        "example.com",
                        Some("link"),
                        Some("https://example.com/page#section")
                    ),
                ]
            );
        }

        #[test]
        fn highlights_the_link_when_the_filter_matches_only_its_hidden_url_so_the_reason_the_row_is_shown_is_visible()
         {
            let classes: Vec<_> = tokens("see https://ja.wikipedia.org/wiki/x", "wiki")
                .iter()
                .map(class)
                .collect();
            assert_eq!(classes, [None, Some("link match")]);
        }

        #[test]
        fn covers_the_title_in_order_without_gaps_so_a_click_on_any_part_maps_back_to_a_caret_position()
         {
            let title = "Buy milk at https://shop.example/milk then pay";
            assert_covers(title, &tokens(title, "a"));
        }

        #[test]
        fn maps_matches_back_to_the_original_text_even_when_lower_casing_changes_the_length_of_letters_before_them()
         {
            let title = "İİ ab İstanbul";
            let parts = tokens(title, "AB");
            assert_covers(title, &parts);
            let matched: Vec<_> = parts
                .iter()
                .filter(|part| part.matched)
                .map(|part| part.text.as_str())
                .collect();
            assert_eq!(matched, ["ab"]);
            let dotted: Vec<_> = tokens(title, "i")
                .iter()
                .filter(|part| part.matched)
                .map(|part| part.text.clone())
                .collect();
            assert_eq!(dotted, ["İ", "İ", "İ"]);
        }

        #[test]
        fn covers_a_title_in_japanese_by_byte_offsets_so_a_click_after_a_link_lands_on_the_right_character()
         {
            let title = "詳細はhttps://example.com/を参照";
            let parts = tokens(title, "参照");
            assert_covers(title, &parts);
            let shown: Vec<_> = parts
                .iter()
                .map(|part| (part.text.as_str(), class(part)))
                .collect();
            assert_eq!(
                shown,
                [
                    ("詳細は", None),
                    ("example.com", Some("link")),
                    ("を", None),
                    ("参照", Some("match"))
                ]
            );
        }
    }
}
