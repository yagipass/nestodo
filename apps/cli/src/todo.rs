use std::collections::HashSet;

use chrono::{Local, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::outline::Outline;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: u64,
    pub parent_id: Option<u64>,
    pub position: usize,
    pub title: String,
    pub completed_at: Option<String>,
    pub collapsed: bool,
}

impl Todo {
    pub fn is_completed(&self) -> bool {
        self.completed_at.is_some()
    }
}

#[derive(Clone, Copy)]
pub struct Style {
    pub color: bool,
}

impl Style {
    fn paint(self, code: &str, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    pub fn dim(self, text: &str) -> String {
        self.paint("2", text)
    }
}

pub struct View {
    pub depth: Option<u32>,
    pub active: bool,
}

impl View {
    fn children<'a>(&self, outline: &'a Outline, parent: Option<u64>) -> Vec<&'a Todo> {
        let mut children = outline.children(parent);
        if self.active {
            children.retain(|todo| !todo.is_completed());
        }
        children
    }

    fn stops_at(&self, depth: u32) -> bool {
        self.depth.is_some_and(|max| depth >= max)
    }
}

struct Line<'a> {
    todo: &'a Todo,
    prefix: String,
    faded: bool,
    hidden: usize,
}

struct Walk<'a> {
    outline: &'a Outline,
    view: &'a View,
    lines: Vec<Line<'a>>,
    seen: HashSet<u64>,
}

impl<'a> Walk<'a> {
    fn branch(&mut self, parent: Option<u64>, lead: &str, depth: u32, faded: bool) {
        let children: Vec<&Todo> = self
            .view
            .children(self.outline, parent)
            .into_iter()
            .filter(|todo| self.seen.insert(todo.id))
            .collect();
        let stops = self.view.stops_at(depth);
        for (index, todo) in children.iter().copied().enumerate() {
            let last = index + 1 == children.len();
            self.lines.push(Line {
                todo,
                prefix: format!("{lead}{}", if last { "└── " } else { "├── " }),
                faded,
                hidden: if stops {
                    self.view.children(self.outline, Some(todo.id)).len()
                } else {
                    0
                },
            });
            if !stops {
                let lead = format!("{lead}{}", if last { "    " } else { "│   " });
                self.branch(
                    Some(todo.id),
                    &lead,
                    depth + 1,
                    faded || todo.is_completed(),
                );
            }
        }
    }
}

pub fn tree(outline: &Outline, start: Option<&Todo>, view: &View, style: Style) -> String {
    let mut walk = Walk {
        outline,
        view,
        lines: Vec::new(),
        seen: start.map(|todo| todo.id).into_iter().collect(),
    };
    walk.branch(start.map(|todo| todo.id), "", 1, false);
    let header = match start {
        Some(todo) => format!("{}  (#{})", outline.path(Some(todo.id)), todo.id),
        None => outline.path(None),
    };
    let mut out = vec![style.paint("1", &header)];
    if walk.lines.is_empty() {
        out.push(style.dim("No todos here yet."));
    }
    let width = walk
        .lines
        .iter()
        .map(|line| line.todo.id.to_string().len())
        .max()
        .unwrap_or(1);
    for line in &walk.lines {
        let mut row = render(outline, line.todo, width, &line.prefix, line.faded, style);
        if line.hidden > 0 {
            row.push_str(&style.dim(&format!("  ▸ +{}", line.hidden)));
        }
        out.push(row);
    }
    out.join("\n")
}

pub fn row(outline: &Outline, todo: &Todo, style: Style) -> String {
    render(outline, todo, 0, "", false, style)
}

fn render(
    outline: &Outline,
    todo: &Todo,
    width: usize,
    prefix: &str,
    faded: bool,
    style: Style,
) -> String {
    let faded = faded || todo.is_completed();
    let check = if todo.is_completed() { "[x]" } else { "[ ]" };
    let lead = format!("{:>width$} {check} ", todo.id);
    let children = outline.children(Some(todo.id));
    let title = printable(&todo.title);
    let mut line = if faded { style.dim(&lead) } else { lead };
    line.push_str(&style.dim(prefix));
    line.push_str(&if faded {
        style.dim(&title)
    } else if children.is_empty() {
        title
    } else {
        style.paint("1", &title)
    });
    if !children.is_empty() {
        let done = children.iter().filter(|child| child.is_completed()).count();
        line.push_str(&style.dim(&format!("  {done}/{}", children.len())));
    }
    line
}

#[derive(Serialize)]
struct Node<'a> {
    id: u64,
    title: &'a str,
    completed: bool,
    collapsed: bool,
    children: Vec<Node<'a>>,
}

pub fn json(outline: &Outline, start: Option<&Todo>, view: &View) -> String {
    let mut seen = start.map(|todo| todo.id).into_iter().collect();
    let text = match start {
        Some(todo) => serde_json::to_string_pretty(&node(outline, todo, view, 0, &mut seen)),
        None => serde_json::to_string_pretty(&nodes(outline, None, view, 1, &mut seen)),
    };
    let mut out = String::new();
    for c in text.expect("todos always serialize to JSON").chars() {
        if ('\u{7f}'..='\u{9f}').contains(&c) || is_bidi_control(c) {
            out.push_str(&format!("\\u{:04x}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

fn nodes<'a>(
    outline: &'a Outline,
    parent: Option<u64>,
    view: &View,
    depth: u32,
    seen: &mut HashSet<u64>,
) -> Vec<Node<'a>> {
    let children: Vec<&Todo> = view
        .children(outline, parent)
        .into_iter()
        .filter(|todo| seen.insert(todo.id))
        .collect();
    children
        .into_iter()
        .map(|todo| node(outline, todo, view, depth, seen))
        .collect()
}

fn node<'a>(
    outline: &'a Outline,
    todo: &'a Todo,
    view: &View,
    depth: u32,
    seen: &mut HashSet<u64>,
) -> Node<'a> {
    Node {
        id: todo.id,
        title: &todo.title,
        completed: todo.is_completed(),
        collapsed: todo.collapsed,
        children: if view.stops_at(depth) {
            Vec::new()
        } else {
            nodes(outline, Some(todo.id), view, depth + 1, seen)
        },
    }
}

pub fn detail(outline: &Outline, todo: &Todo) -> String {
    let children = outline.children(Some(todo.id));
    let open = children
        .iter()
        .filter(|child| !child.is_completed())
        .count();
    let children = match children.len() {
        0 => "none".to_string(),
        total => format!("{open} open / {total}"),
    };
    let completed = todo
        .completed_at
        .as_deref()
        .map_or("no".to_string(), local_time);
    format!(
        "ID:        {}\nTitle:     {}\nParent:    {}\nCompleted: {completed}\nChildren:  {children}",
        todo.id,
        printable(&todo.title),
        outline.path(todo.parent_id)
    )
}

pub fn printable(text: &str) -> String {
    text.replace("\r\n", "\n")
        .chars()
        .map(|c| match c {
            '\t' | '\r' | '\n' => ' ',
            c if c.is_control() || is_bidi_control(c) => '\u{FFFD}',
            c => c,
        })
        .collect()
}

fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{61c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

fn local_time(utc: &str) -> String {
    NaiveDateTime::parse_from_str(utc, "%Y-%m-%d %H:%M:%S").map_or_else(
        |_| printable(utc),
        |time| {
            time.and_utc()
                .with_timezone(&Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const PLAIN: Style = Style { color: false };
    const COLOR: Style = Style { color: true };
    const ALL: View = View {
        depth: None,
        active: false,
    };

    fn todo(id: u64, parent: Option<u64>, position: usize, title: &str, done: bool) -> Todo {
        Todo {
            id,
            parent_id: parent,
            position,
            title: title.into(),
            completed_at: done.then(|| "2026-10-20 09:00:00".into()),
            collapsed: false,
        }
    }

    fn sample() -> Outline {
        Outline::new(vec![
            todo(1, None, 0, "Work", false),
            todo(4, Some(1), 0, "v2 release", false),
            todo(7, Some(4), 0, "Show the CLI list as a tree", false),
            todo(10, Some(4), 1, "Draft the release notes", true),
            todo(5, Some(1), 1, "Prepare for the 1on1", false),
            todo(2, None, 1, "Chores", true),
            todo(11, Some(2), 0, "Milk", false),
            todo(18, None, 2, "Book the dentist", false),
        ])
    }

    #[test]
    fn draws_the_outline_as_a_tree_with_titles_last_so_wide_characters_cannot_misalign_it() {
        assert_eq!(
            tree(&sample(), None, &ALL, PLAIN),
            "Home\n \
             1 [ ] ├── Work  0/2\n \
             4 [ ] │   ├── v2 release  1/2\n \
             7 [ ] │   │   ├── Show the CLI list as a tree\n\
             10 [x] │   │   └── Draft the release notes\n \
             5 [ ] │   └── Prepare for the 1on1\n \
             2 [x] ├── Chores  0/1\n\
             11 [ ] │   └── Milk\n\
             18 [ ] └── Book the dentist"
        );
    }

    #[test]
    fn starts_at_the_given_todo_and_says_where_that_is() {
        let outline = sample();
        assert_eq!(
            tree(&outline, Some(outline.get(4).unwrap()), &ALL, PLAIN),
            "Home › Work › v2 release  (#4)\n \
             7 [ ] ├── Show the CLI list as a tree\n\
             10 [x] └── Draft the release notes"
        );
    }

    #[test]
    fn cuts_the_tree_at_the_depth_asked_for_and_says_how_much_is_hidden() {
        let view = View {
            depth: Some(1),
            active: false,
        };
        assert_eq!(
            tree(&sample(), None, &view, PLAIN),
            "Home\n \
             1 [ ] ├── Work  0/2  ▸ +2\n \
             2 [x] ├── Chores  0/1  ▸ +1\n\
             18 [ ] └── Book the dentist"
        );
    }

    #[test]
    fn active_hides_finished_todos_with_everything_under_them_but_still_counts_them() {
        let view = View {
            depth: None,
            active: true,
        };
        assert_eq!(
            tree(&sample(), None, &view, PLAIN),
            "Home\n \
             1 [ ] ├── Work  0/2\n \
             4 [ ] │   ├── v2 release  1/2\n \
             7 [ ] │   │   └── Show the CLI list as a tree\n \
             5 [ ] │   └── Prepare for the 1on1\n\
             18 [ ] └── Book the dentist"
        );
    }

    #[test]
    fn says_so_when_there_is_nothing_to_list() {
        assert_eq!(
            tree(&Outline::default(), None, &ALL, PLAIN),
            "Home\nNo todos here yet."
        );
        let outline = sample();
        assert_eq!(
            tree(&outline, Some(outline.get(11).unwrap()), &ALL, PLAIN),
            "Home › Chores › Milk  (#11)\nNo todos here yet."
        );
    }

    #[test]
    fn dims_open_todos_under_a_finished_one_because_finishing_the_parent_covers_them() {
        let colored = tree(&sample(), None, &ALL, COLOR);
        let line = |title: &str| colored.lines().find(|line| line.contains(title)).unwrap();
        assert!(line("Milk").starts_with("\x1b[2m11 [ ] \x1b[0m"));
        assert!(line("Milk").contains("\x1b[2mMilk\x1b[0m"));
        assert!(line("dentist").starts_with("18 [ ] "));
        assert!(!tree(&sample(), None, &ALL, PLAIN).contains('\x1b'));
    }

    #[test]
    fn json_nests_children_so_scripts_get_the_tree_without_rebuilding_it() {
        let outline = sample();
        let branch: Value =
            serde_json::from_str(&json(&outline, Some(outline.get(4).unwrap()), &ALL)).unwrap();
        assert_eq!(
            branch,
            json!({ "id": 4, "title": "v2 release", "completed": false, "collapsed": false, "children": [
                { "id": 7, "title": "Show the CLI list as a tree", "completed": false, "collapsed": false, "children": [] },
                { "id": 10, "title": "Draft the release notes", "completed": true, "collapsed": false, "children": [] },
            ]})
        );

        let view = View {
            depth: Some(1),
            active: true,
        };
        let home: Value = serde_json::from_str(&json(&outline, None, &view)).unwrap();
        assert_eq!(
            home,
            json!([
                { "id": 1, "title": "Work", "completed": false, "collapsed": false, "children": [] },
                { "id": 18, "title": "Book the dentist", "completed": false, "collapsed": false, "children": [] },
            ])
        );
    }

    #[test]
    fn replaces_control_characters_in_titles_so_a_stored_title_cannot_drive_the_terminal() {
        let stored = "Pay rent\u{1b}]52;c;aGk=\u{7}\r\u{1b}[2K\n\u{9b}2Kforged";
        let shown = "Pay rent\u{FFFD}]52;c;aGk=\u{FFFD} \u{FFFD}[2K \u{FFFD}2Kforged";
        let outline = Outline::new(vec![
            todo(1, None, 0, stored, false),
            todo(2, Some(1), 0, "child", false),
        ]);

        assert_eq!(
            tree(&outline, None, &ALL, PLAIN),
            format!("Home\n1 [ ] └── {shown}  0/1\n2 [ ]     └── child")
        );
        assert_eq!(
            tree(&outline, Some(outline.get(2).unwrap()), &ALL, PLAIN),
            format!("Home › {shown} › child  (#2)\nNo todos here yet.")
        );
        assert!(
            detail(&outline, outline.get(1).unwrap()).contains(&format!("\nTitle:     {shown}\n"))
        );
        assert!(
            detail(&outline, outline.get(2).unwrap())
                .contains(&format!("\nParent:    Home › {shown}\n"))
        );

        let exact: Value = serde_json::from_str(&json(&outline, None, &ALL)).unwrap();
        assert_eq!(exact[0]["title"], stored);
    }

    #[test]
    fn replaces_control_characters_in_finished_titles_too_as_list_shows_them_by_default() {
        let stored = "Pay rent\u{1b}]52;c;aGk=\u{7}";
        let shown = "Pay rent\u{FFFD}]52;c;aGk=\u{FFFD}";
        let outline = Outline::new(vec![
            todo(1, None, 0, stored, true),
            todo(2, Some(1), 0, stored, false),
        ]);

        assert_eq!(
            tree(&outline, None, &ALL, PLAIN),
            format!("Home\n1 [x] └── {shown}  0/1\n2 [ ]     └── {shown}")
        );
        assert_eq!(
            row(&outline, outline.get(1).unwrap(), PLAIN),
            format!("1 [x] {shown}  0/1")
        );
    }

    #[test]
    fn shows_tabs_and_old_line_breaks_in_titles_as_spaces_so_pasted_rows_do_not_look_corrupted() {
        let outline = Outline::new(vec![
            todo(1, None, 0, "Milk\t2L", false),
            todo(2, Some(1), 0, "buy\r\nmilk", false),
        ]);

        assert_eq!(
            tree(&outline, None, &ALL, PLAIN),
            "Home\n1 [ ] └── Milk 2L  0/1\n2 [ ]     └── buy milk"
        );
        assert!(
            detail(&outline, outline.get(2).unwrap())
                .contains("\nTitle:     buy milk\nParent:    Home › Milk 2L\n")
        );
    }

    #[test]
    fn json_escapes_del_and_c1_so_terminals_never_get_them_raw_but_scripts_get_the_exact_title() {
        let stored = "Pay rent\u{9b}2J\u{7f}\u{9d}8;;https://evil.example\u{9c}click\u{9d}8;;\u{9c}\u{80}\u{9f}";
        let outline = Outline::new(vec![todo(1, None, 0, stored, false)]);
        let home = json(&outline, None, &ALL);
        let one = json(&outline, Some(outline.get(1).unwrap()), &ALL);

        for text in [&home, &one] {
            assert!(
                !text.chars().any(|c| ('\u{7f}'..='\u{9f}').contains(&c)),
                "{text:?}"
            );
        }
        assert_eq!(
            serde_json::from_str::<Value>(&home).unwrap()[0]["title"],
            stored
        );
        assert_eq!(
            serde_json::from_str::<Value>(&one).unwrap()["title"],
            stored
        );
    }

    #[test]
    fn replaces_bidi_controls_in_titles_so_a_stored_title_cannot_reorder_what_the_terminal_shows() {
        const BIDI: [char; 12] = [
            '\u{61c}', '\u{200e}', '\u{200f}', '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}',
            '\u{202e}', '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ];
        let stored = format!("Pay \u{202e}lin\u{202c} {}", String::from_iter(BIDI));
        let shown = format!("Pay \u{FFFD}lin\u{FFFD} {}", "\u{FFFD}".repeat(BIDI.len()));
        let family = "👨\u{200d}👩\u{200d}👧 dinner";
        let outline = Outline::new(vec![
            todo(1, None, 0, &stored, false),
            todo(2, Some(1), 0, family, false),
        ]);

        assert_eq!(
            tree(&outline, None, &ALL, PLAIN),
            format!("Home\n1 [ ] └── {shown}  0/1\n2 [ ]     └── {family}")
        );
        assert!(
            detail(&outline, outline.get(2).unwrap())
                .contains(&format!("\nParent:    Home › {shown}\n"))
        );

        let text = json(&outline, None, &ALL);
        assert!(!text.contains(BIDI), "{text:?}");
        assert_eq!(
            serde_json::from_str::<Value>(&text).unwrap()[0]["title"],
            stored
        );
    }

    #[test]
    fn shows_a_completion_time_it_cannot_read_as_is_but_without_control_characters() {
        let mut done = todo(1, None, 0, "Milk", true);
        done.completed_at = Some("2026-10-20\u{1b}]52;c;aGk=\u{7}".into());
        let outline = Outline::new(vec![done]);

        assert!(
            detail(&outline, outline.get(1).unwrap())
                .contains("\nCompleted: 2026-10-20\u{FFFD}]52;c;aGk=\u{FFFD}\n")
        );
    }

    #[test]
    fn lists_a_todo_caught_in_a_parent_loop_once_instead_of_hanging() {
        let outline = Outline::new(vec![
            todo(2, Some(3), 0, "a", false),
            todo(3, Some(2), 0, "b", false),
        ]);
        let a = outline.get(2).unwrap();

        assert_eq!(
            tree(&outline, Some(a), &ALL, PLAIN),
            "Home › b › a  (#2)\n3 [ ] └── b  0/1"
        );
        assert_eq!(
            serde_json::from_str::<Value>(&json(&outline, Some(a), &ALL)).unwrap(),
            json!({ "id": 2, "title": "a", "completed": false, "collapsed": false, "children": [
                { "id": 3, "title": "b", "completed": false, "collapsed": false, "children": [] },
            ]})
        );
        assert!(detail(&outline, a).contains("\nParent:    Home › a › b\n"));
    }

    #[test]
    fn shows_where_a_todo_sits_and_how_much_is_left_under_it() {
        let outline = sample();
        assert_eq!(
            detail(&outline, outline.get(4).unwrap()),
            "ID:        4\nTitle:     v2 release\nParent:    Home › Work\nCompleted: no\nChildren:  1 open / 2"
        );
        assert!(detail(&outline, outline.get(18).unwrap()).ends_with("Children:  none"));
    }
}
