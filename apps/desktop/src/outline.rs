use std::collections::{HashMap, HashSet};

use unicode_segmentation::UnicodeSegmentation;

use crate::todos::{MAX_DEPTH, MAX_TITLE_LENGTH, utf16_len};
use crate::wire::Todo;

pub const ROOT: &str = "root";

pub type Key = String;

pub const END: usize = usize::MAX;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub key: Key,
    pub title: String,
    pub note: String,
    pub done: bool,
    pub collapsed: bool,
    pub children: Vec<Key>,
}

impl Item {
    fn new(key: &str, title: &str, note: &str) -> Item {
        Item {
            key: key.to_owned(),
            title: title.to_owned(),
            note: note.to_owned(),
            done: false,
            collapsed: false,
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outline {
    items: HashMap<Key, Item>,
}

impl Default for Outline {
    fn default() -> Self {
        Outline {
            items: HashMap::from([(ROOT.to_owned(), Item::new(ROOT, "", ""))]),
        }
    }
}

impl Outline {
    pub fn from_todos(todos: &[Todo], key_of: impl Fn(u64) -> Key) -> Outline {
        let mut outline = Outline::default();
        for todo in todos {
            let key = key_of(todo.id);
            let item = Item {
                done: todo.completed_at.is_some(),
                collapsed: todo.collapsed,
                ..Item::new(&key, &todo.title, &todo.note)
            };
            outline.items.insert(key, item);
        }
        for todo in todos {
            let parent = todo.parent_id.map_or_else(|| ROOT.to_owned(), &key_of);
            if let Some(item) = outline.items.get_mut(&parent) {
                item.children.push(key_of(todo.id));
            }
        }
        outline
    }

    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Item> {
        self.items.get(key)
    }

    #[must_use]
    pub fn item(&self, key: &str) -> &Item {
        self.get(key)
            .unwrap_or_else(|| panic!("{key} is not in the outline"))
    }

    #[must_use]
    pub fn contains(&self, key: &str) -> bool {
        self.items.contains_key(key)
    }

    fn item_mut(&mut self, key: &str) -> &mut Item {
        self.items
            .get_mut(key)
            .unwrap_or_else(|| panic!("{key} is not in the outline"))
    }

    fn index_in(&self, parent: &str, key: &str) -> usize {
        self.item(parent)
            .children
            .iter()
            .position(|child| child == key)
            .unwrap_or_else(|| panic!("{key} is not a child of {parent}"))
    }

    fn insert_child(&mut self, parent: &str, index: usize, key: &str) {
        let children = &mut self.item_mut(parent).children;
        children.insert(index.min(children.len()), key.to_owned());
    }

    fn detach(&mut self, parent: &str, key: &str) {
        let index = self.index_in(parent, key);
        self.item_mut(parent).children.remove(index);
    }
}

#[must_use]
pub fn key_for(id: u64) -> Key {
    format!("t{id}")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Op {
    Create {
        key: Key,
        parent: Key,
        index: usize,
        title: String,
        note: Option<String>,
    },
    Title {
        key: Key,
        title: String,
    },
    Note {
        key: Key,
        note: String,
    },
    Update {
        key: Key,
        completed: Option<bool>,
        collapsed: Option<bool>,
    },
    Move {
        key: Key,
        parent: Key,
        index: usize,
    },
    Delete {
        key: Key,
        removed: Vec<Key>,
    },
}

impl Op {
    #[must_use]
    pub fn key(&self) -> &Key {
        match self {
            Op::Create { key, .. }
            | Op::Title { key, .. }
            | Op::Note { key, .. }
            | Op::Update { key, .. }
            | Op::Move { key, .. }
            | Op::Delete { key, .. } => key,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Focus {
    pub key: Key,
    pub pos: usize,
    pub note: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub outline: Outline,
    pub ops: Vec<Op>,
    pub focus: Option<Focus>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub key: Key,
    pub depth: usize,
    pub under: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    pub key: Key,
    pub title: String,
    pub path: Vec<String>,
}

fn focus(key: &str, pos: usize) -> Focus {
    Focus {
        key: key.to_owned(),
        pos,
        note: false,
    }
}

#[must_use]
pub fn caret_in(text: &str, pos: usize) -> usize {
    text.grapheme_indices(true)
        .map(|(at, _)| at)
        .find(|&at| at >= pos)
        .unwrap_or(text.len())
}

#[must_use]
pub fn parent_of(outline: &Outline, key: &str) -> Option<Key> {
    outline
        .items
        .values()
        .find(|item| item.children.iter().any(|child| child == key))
        .map(|item| item.key.clone())
}

#[must_use]
pub fn subtree(outline: &Outline, key: &str) -> Vec<Key> {
    let mut keys = vec![key.to_owned()];
    for child in &outline.item(key).children {
        keys.extend(subtree(outline, child));
    }
    keys
}

fn depth_of(outline: &Outline, key: &str) -> usize {
    let mut depth = 0;
    let mut current = key.to_owned();
    while current != ROOT {
        depth += 1;
        current = parent_of(outline, &current)
            .unwrap_or_else(|| panic!("{current} is not under the root"));
    }
    depth
}

fn height_of(outline: &Outline, key: &str) -> usize {
    outline
        .item(key)
        .children
        .iter()
        .map(|child| height_of(outline, child))
        .max()
        .unwrap_or(0)
        + 1
}

fn mark(
    outline: &Outline,
    key: &str,
    q: &str,
    keep: &HashSet<Key>,
    hit: &mut HashSet<Key>,
) -> bool {
    let item = outline.item(key);
    let mut any = item.title.to_lowercase().contains(q)
        || item.note.to_lowercase().contains(q)
        || keep.contains(key);
    for child in &item.children {
        if mark(outline, child, q, keep, hit) {
            any = true;
        }
    }
    if any {
        hit.insert(key.to_owned());
    }
    any
}

struct RowWalk<'a> {
    outline: &'a Outline,
    show_done: bool,
    hit: Option<HashSet<Key>>,
    rows: Vec<Row>,
}

impl RowWalk<'_> {
    fn walk(&mut self, key: &str, depth: usize, under: bool) {
        let outline = self.outline;
        for child in &outline.item(key).children {
            let item = outline.item(child);
            if !self.show_done && item.done {
                continue;
            }
            if self.hit.as_ref().is_some_and(|hit| !hit.contains(child)) {
                continue;
            }
            self.rows.push(Row {
                key: child.clone(),
                depth,
                under,
            });
            if self.hit.is_some() || !item.collapsed {
                self.walk(child, depth + 1, under || item.done);
            }
        }
    }
}

#[must_use]
pub fn visible_rows(
    outline: &Outline,
    show_done: bool,
    query: &str,
    keep: &HashSet<Key>,
) -> Vec<Row> {
    let q = query.trim().to_lowercase();
    let hit = (!q.is_empty()).then(|| {
        let mut hit = HashSet::new();
        for child in &outline.item(ROOT).children {
            mark(outline, child, &q, keep, &mut hit);
        }
        hit
    });
    let mut walk = RowWalk {
        outline,
        show_done,
        hit,
        rows: Vec::new(),
    };
    walk.walk(ROOT, 0, false);
    walk.rows
}

fn shown_sibling(
    siblings: &[Key],
    from: usize,
    step: isize,
    shown: &HashSet<Key>,
) -> Option<usize> {
    let mut i = from.checked_add_signed(step)?;
    while let Some(sibling) = siblings.get(i) {
        if shown.contains(sibling) {
            return Some(i);
        }
        i = i.checked_add_signed(step)?;
    }
    None
}

#[must_use]
pub fn rename(outline: &Outline, key: &str, title: &str) -> Outline {
    let mut next = outline.clone();
    title.clone_into(&mut next.item_mut(key).title);
    next
}

#[must_use]
pub fn set_note(outline: &Outline, key: &str, note: &str) -> Outline {
    let mut next = outline.clone();
    note.clone_into(&mut next.item_mut(key).note);
    next
}

#[must_use]
pub fn split(
    outline: &Outline,
    key: &str,
    pos: usize,
    fresh: &str,
    shown: &HashSet<Key>,
) -> Change {
    let parent = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
    let item = outline.item(key);
    let pos = caret_in(&item.title, pos);
    let mut next = outline.clone();
    if pos == 0 && !item.title.is_empty() {
        let index = next.index_in(&parent, key);
        next.items
            .insert(fresh.to_owned(), Item::new(fresh, "", ""));
        next.insert_child(&parent, index, fresh);
        return Change {
            outline: next,
            ops: vec![Op::Create {
                key: fresh.to_owned(),
                parent,
                index,
                title: String::new(),
                note: None,
            }],
            focus: Some(focus(key, 0)),
        };
    }
    let (before, after) = item.title.split_at(pos);
    let into_children = after.is_empty()
        && !item.collapsed
        && item.children.iter().any(|child| shown.contains(child));
    let target = if into_children { key } else { parent.as_str() };
    before.clone_into(&mut next.item_mut(key).title);
    next.items
        .insert(fresh.to_owned(), Item::new(fresh, after, ""));
    let index = if into_children {
        0
    } else {
        next.index_in(&parent, key) + 1
    };
    next.insert_child(target, index, fresh);
    let mut ops = vec![Op::Create {
        key: fresh.to_owned(),
        parent: target.to_owned(),
        index,
        title: after.to_owned(),
        note: None,
    }];
    if !after.is_empty() {
        ops.push(Op::Title {
            key: key.to_owned(),
            title: before.to_owned(),
        });
    }
    Change {
        outline: next,
        ops,
        focus: Some(focus(fresh, 0)),
    }
}

#[must_use]
pub fn indent(outline: &Outline, key: &str, shown: &HashSet<Key>) -> Option<Change> {
    let parent = parent_of(outline, key)?;
    let siblings = &outline.item(&parent).children;
    let i = outline.index_in(&parent, key);
    let target = siblings[shown_sibling(siblings, i, -1, shown)?].clone();
    if depth_of(outline, &target) + height_of(outline, key) > MAX_DEPTH {
        return None;
    }
    let mut next = outline.clone();
    next.item_mut(&parent).children.remove(i);
    let index = next.item(&target).children.len();
    next.item_mut(&target).children.push(key.to_owned());
    let mut ops = vec![Op::Move {
        key: key.to_owned(),
        parent: target.clone(),
        index,
    }];
    if next.item(&target).collapsed {
        next.item_mut(&target).collapsed = false;
        ops.push(Op::Update {
            key: target,
            completed: None,
            collapsed: Some(false),
        });
    }
    Some(Change {
        outline: next,
        ops,
        focus: None,
    })
}

#[must_use]
pub fn outdent(outline: &Outline, key: &str) -> Option<Change> {
    let parent = parent_of(outline, key)?;
    if parent == ROOT {
        return None;
    }
    let grandparent = parent_of(outline, &parent)?;
    let mut next = outline.clone();
    next.detach(&parent, key);
    let index = next.index_in(&grandparent, &parent) + 1;
    next.insert_child(&grandparent, index, key);
    Some(Change {
        outline: next,
        ops: vec![Op::Move {
            key: key.to_owned(),
            parent: grandparent,
            index,
        }],
        focus: None,
    })
}

#[must_use]
pub fn move_by(outline: &Outline, key: &str, step: isize, shown: &HashSet<Key>) -> Option<Change> {
    let parent = parent_of(outline, key)?;
    let from = outline.index_in(&parent, key);
    let index = shown_sibling(&outline.item(&parent).children, from, step, shown)?;
    let mut next = outline.clone();
    let children = &mut next.item_mut(&parent).children;
    children.remove(from);
    children.insert(index, key.to_owned());
    Some(Change {
        outline: next,
        ops: vec![Op::Move {
            key: key.to_owned(),
            parent,
            index,
        }],
        focus: None,
    })
}

struct DestinationSearch<'a> {
    outline: &'a Outline,
    key: &'a str,
    show_done: bool,
    q: String,
    height: usize,
    found: Vec<Destination>,
}

impl DestinationSearch<'_> {
    fn walk(&mut self, parent: &str, path: &[String]) {
        let outline = self.outline;
        for child in &outline.item(parent).children {
            let Item { title, done, .. } = outline.item(child);
            if child == self.key
                || (!self.show_done && *done)
                || path.len() + 1 + self.height > MAX_DEPTH
            {
                continue;
            }
            if !title.trim().is_empty() && title.to_lowercase().contains(&self.q) {
                self.found.push(Destination {
                    key: child.clone(),
                    title: title.clone(),
                    path: path.to_vec(),
                });
            }
            let mut below = path.to_vec();
            below.push(title.clone());
            self.walk(child, &below);
        }
    }
}

#[must_use]
pub fn destinations(
    outline: &Outline,
    key: &str,
    show_done: bool,
    query: &str,
) -> Vec<Destination> {
    let mut search = DestinationSearch {
        outline,
        key,
        show_done,
        q: query.trim().to_lowercase(),
        height: height_of(outline, key),
        found: Vec::new(),
    };
    search.walk(ROOT, &[]);
    search.found
}

#[must_use]
pub fn move_to(outline: &Outline, key: &str, target: &str) -> Change {
    let parent = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
    let mut next = outline.clone();
    next.detach(&parent, key);
    let index = next.item(target).children.len();
    next.item_mut(target).children.push(key.to_owned());
    Change {
        outline: next,
        ops: vec![Op::Move {
            key: key.to_owned(),
            parent: target.to_owned(),
            index,
        }],
        focus: None,
    }
}

#[must_use]
pub fn toggle_done(outline: &Outline, key: &str) -> Change {
    let mut next = outline.clone();
    let item = next.item_mut(key);
    item.done = !item.done;
    let done = item.done;
    Change {
        outline: next,
        ops: vec![Op::Update {
            key: key.to_owned(),
            completed: Some(done),
            collapsed: None,
        }],
        focus: None,
    }
}

#[must_use]
pub fn set_collapsed(outline: &Outline, key: &str, collapsed: bool) -> Option<Change> {
    let item = outline.item(key);
    if item.children.is_empty() || item.collapsed == collapsed {
        return None;
    }
    let mut next = outline.clone();
    next.item_mut(key).collapsed = collapsed;
    Some(Change {
        outline: next,
        ops: vec![Op::Update {
            key: key.to_owned(),
            completed: None,
            collapsed: Some(collapsed),
        }],
        focus: None,
    })
}

fn delete_subtree(outline: &mut Outline, key: &str) -> Vec<Key> {
    let parent = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
    let removed = subtree(outline, key);
    outline.detach(&parent, key);
    for gone in &removed {
        outline.items.remove(gone);
    }
    removed
}

#[must_use]
pub fn remove(outline: &Outline, key: &str) -> Change {
    let mut next = outline.clone();
    let removed = delete_subtree(&mut next, key);
    Change {
        outline: next,
        ops: vec![Op::Delete {
            key: key.to_owned(),
            removed,
        }],
        focus: None,
    }
}

#[must_use]
pub fn merge_into_previous(outline: &Outline, rows: &[Row], key: &str) -> Option<Change> {
    let i = rows.iter().position(|row| row.key == key)?;
    let previous = rows.get(i.checked_sub(1)?)?;
    let item = outline.item(key);
    if !item.children.is_empty() || previous.depth > rows[i].depth {
        return None;
    }
    let target = outline.item(&previous.key);
    let title = format!("{}{}", target.title, item.title);
    if item.done || target.done || !item.note.is_empty() || utf16_len(&title) > MAX_TITLE_LENGTH {
        return None;
    }
    let removed = remove(outline, key);
    let mut ops = vec![Op::Title {
        key: previous.key.clone(),
        title: title.clone(),
    }];
    ops.extend(removed.ops);
    Some(Change {
        outline: rename(&removed.outline, &previous.key, &title),
        ops,
        focus: Some(focus(&previous.key, target.title.len())),
    })
}

#[must_use]
pub fn append(outline: &Outline, parent: &str, fresh: &str) -> Change {
    let mut next = outline.clone();
    let index = next.item(parent).children.len();
    next.items
        .insert(fresh.to_owned(), Item::new(fresh, "", ""));
    next.item_mut(parent).children.push(fresh.to_owned());
    Change {
        outline: next,
        ops: vec![Op::Create {
            key: fresh.to_owned(),
            parent: parent.to_owned(),
            index,
            title: String::new(),
            note: None,
        }],
        focus: Some(focus(fresh, 0)),
    }
}

fn apply_op(outline: &mut Outline, op: &Op) {
    match op {
        Op::Create {
            key,
            parent,
            index,
            title,
            note,
        } => {
            let note = note.as_deref().unwrap_or("");
            outline
                .items
                .insert(key.clone(), Item::new(key, title, note));
            outline.insert_child(parent, *index, key);
        }
        Op::Title { key, title } => outline.item_mut(key).title.clone_from(title),
        Op::Note { key, note } => outline.item_mut(key).note.clone_from(note),
        Op::Update {
            key,
            completed,
            collapsed,
        } => {
            let item = outline.item_mut(key);
            item.done = completed.unwrap_or(item.done);
            item.collapsed = collapsed.unwrap_or(item.collapsed);
        }
        Op::Move { key, parent, index } => {
            let from = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
            outline.detach(&from, key);
            outline.insert_child(parent, *index, key);
        }
        Op::Delete { key, .. } => {
            delete_subtree(outline, key);
        }
    }
}

#[must_use]
pub fn apply_ops(outline: &Outline, ops: &[Op]) -> Outline {
    let mut next = outline.clone();
    for op in ops {
        apply_op(&mut next, op);
    }
    next
}

fn recreate(outline: &Outline, key: &str) -> Vec<Op> {
    let parent = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
    let item = outline.item(key);
    let mut ops = vec![Op::Create {
        key: key.to_owned(),
        index: outline.index_in(&parent, key),
        parent,
        title: item.title.clone(),
        note: Some(item.note.clone()),
    }];
    if item.done || item.collapsed {
        ops.push(Op::Update {
            key: key.to_owned(),
            completed: item.done.then_some(true),
            collapsed: item.collapsed.then_some(true),
        });
    }
    ops
}

fn inverse(outline: &Outline, op: &Op) -> Vec<Op> {
    match op {
        Op::Create { key, .. } => vec![Op::Delete {
            key: key.clone(),
            removed: vec![key.clone()],
        }],
        Op::Title { key, .. } => vec![Op::Title {
            key: key.clone(),
            title: outline.item(key).title.clone(),
        }],
        Op::Note { key, .. } => vec![Op::Note {
            key: key.clone(),
            note: outline.item(key).note.clone(),
        }],
        Op::Update {
            key,
            completed,
            collapsed,
        } => {
            let item = outline.item(key);
            vec![Op::Update {
                key: key.clone(),
                completed: completed.map(|_| item.done),
                collapsed: collapsed.map(|_| item.collapsed),
            }]
        }
        Op::Move { key, .. } => {
            let parent = parent_of(outline, key).unwrap_or_else(|| panic!("{key} has no parent"));
            vec![Op::Move {
                key: key.clone(),
                index: outline.index_in(&parent, key),
                parent,
            }]
        }
        Op::Delete { removed, .. } => removed
            .iter()
            .flat_map(|key| recreate(outline, key))
            .collect(),
    }
}

#[must_use]
pub fn undo_for(outline: &Outline, ops: &[Op]) -> Vec<Op> {
    let mut current = outline.clone();
    let mut steps = Vec::with_capacity(ops.len());
    for op in ops {
        steps.push(inverse(&current, op));
        apply_op(&mut current, op);
    }
    steps.into_iter().rev().flatten().collect()
}

#[cfg(test)]
pub(crate) mod fixtures {
    use std::collections::HashSet;

    use super::{Key, Op, Outline, key_for, visible_rows};
    use crate::wire::Todo;

    pub(crate) fn todo(id: u64, title: &str, parent: Option<u64>) -> Todo {
        Todo {
            id,
            parent_id: parent,
            position: 0,
            title: title.to_owned(),
            note: String::new(),
            completed_at: None,
            collapsed: false,
        }
    }

    pub(crate) trait Mark {
        fn done(self) -> Self;
        fn folded(self) -> Self;
        fn noted(self, note: &str) -> Self;
    }

    impl Mark for Todo {
        fn done(self) -> Self {
            Todo {
                completed_at: Some("2026-10-23 09:00:00".to_owned()),
                ..self
            }
        }

        fn folded(self) -> Self {
            Todo {
                collapsed: true,
                ..self
            }
        }

        fn noted(self, note: &str) -> Self {
            Todo {
                note: note.to_owned(),
                ..self
            }
        }
    }

    pub(crate) fn outline_of(todos: &[Todo]) -> Outline {
        Outline::from_todos(todos, key_for)
    }

    pub(crate) fn shown(outline: &Outline, show_done: bool, query: &str) -> HashSet<Key> {
        visible_rows(outline, show_done, query, &HashSet::new())
            .into_iter()
            .map(|row| row.key)
            .collect()
    }

    pub(crate) fn create_op(key: &str, parent: &str, index: usize, title: &str) -> Op {
        Op::Create {
            key: key.to_owned(),
            parent: parent.to_owned(),
            index,
            title: title.to_owned(),
            note: None,
        }
    }

    pub(crate) fn title_op(key: &str, title: &str) -> Op {
        Op::Title {
            key: key.to_owned(),
            title: title.to_owned(),
        }
    }

    pub(crate) fn move_op(key: &str, parent: &str, index: usize) -> Op {
        Op::Move {
            key: key.to_owned(),
            parent: parent.to_owned(),
            index,
        }
    }

    pub(crate) fn update_op(key: &str, completed: Option<bool>, collapsed: Option<bool>) -> Op {
        Op::Update {
            key: key.to_owned(),
            completed,
            collapsed,
        }
    }

    pub(crate) fn delete_op(key: &str, removed: &[&str]) -> Op {
        Op::Delete {
            key: key.to_owned(),
            removed: removed.iter().map(|&key| key.to_owned()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde_json::{Value, json};

    use super::fixtures::*;
    use super::*;

    fn chain(depth: u64) -> Vec<Todo> {
        (0..depth)
            .map(|i| todo(i + 1, &format!("level {}", i + 1), (i > 0).then_some(i)))
            .collect()
    }

    fn titles(outline: &Outline, parent: &str) -> Value {
        Value::Array(
            outline
                .item(parent)
                .children
                .iter()
                .map(|key| {
                    let item = outline.item(key);
                    if item.children.is_empty() {
                        json!(item.title)
                    } else {
                        json!([item.title, titles(outline, key)])
                    }
                })
                .collect(),
        )
    }

    fn rows(outline: &Outline, show_done: bool, query: &str) -> Vec<Row> {
        visible_rows(outline, show_done, query, &HashSet::new())
    }

    fn row_keys(outline: &Outline, show_done: bool, query: &str) -> Vec<Key> {
        rows(outline, show_done, query)
            .into_iter()
            .map(|row| row.key)
            .collect()
    }

    fn sample() -> Outline {
        outline_of(&[
            todo(1, "work", None),
            todo(2, "release", Some(1)),
            todo(3, "write notes", Some(2)),
            todo(4, "expenses", Some(1)),
            todo(5, "home", None),
        ])
    }

    fn all(outline: &Outline) -> HashSet<Key> {
        shown(outline, true, "")
    }

    fn keys(destinations: Vec<Destination>) -> Vec<Key> {
        destinations.into_iter().map(|d| d.key).collect()
    }

    mod caret_in {
        use super::*;

        #[test]
        fn never_puts_the_caret_inside_an_emoji_so_enter_cannot_split_it_into_two_broken_halves() {
            for inside in 1..4 {
                assert_eq!(caret_in("\u{1F600} party", inside), 4);
            }
            assert_eq!(caret_in("ab", 1), 1);
        }

        #[test]
        fn never_puts_the_caret_between_the_code_points_of_one_visible_character_so_up_down_or_undo_then_enter_cannot_split_a_flag_or_a_voiced_kana()
         {
            assert_eq!(caret_in("\u{1F1EF}\u{1F1F5} trip", 4), 8);
            assert_eq!(caret_in("\u{1F44D}\u{1F3FD}", 4), 8);
            assert_eq!(caret_in("\u{1F469}\u{200D}\u{1F4BB}", 4), 11);
            assert_eq!(caret_in("\u{304B}\u{3099}", 3), 6);
        }

        #[test]
        fn keeps_the_caret_within_the_text_and_treats_a_negative_position_as_the_end() {
            assert_eq!(caret_in("ab", 5), 2);
            assert_eq!(caret_in("ab", END), 2);
        }
    }

    mod from_todos {
        use super::*;

        #[test]
        fn can_reuse_the_keys_of_rows_created_in_this_tab_so_reloading_keeps_their_inputs_and_focus_in_place()
         {
            let outline = Outline::from_todos(
                &[todo(1, "made here", None), todo(2, "child", Some(1))],
                |id| {
                    if id == 1 {
                        "new1".to_owned()
                    } else {
                        key_for(id)
                    }
                },
            );
            assert_eq!(outline.item(ROOT).children, ["new1"]);
            assert_eq!(outline.item("new1").children, ["t2"]);
        }
    }

    mod split {
        use super::*;

        #[test]
        fn keeps_the_head_on_the_original_row_and_creates_the_tail_first_so_a_failure_in_between_duplicates_text_instead_of_losing_it()
         {
            let sample = sample();
            let change = split(&sample, "t4", 3, "n1", &all(&sample));
            assert_eq!(
                titles(&change.outline, "t1"),
                json!([["release", ["write notes"]], "exp", "enses"])
            );
            assert_eq!(
                change.ops,
                [create_op("n1", "t1", 2, "enses"), title_op("t4", "exp")]
            );
            assert_eq!(change.focus, Some(focus("n1", 0)));
        }

        #[test]
        fn starts_the_first_child_when_enter_is_pressed_at_the_end_of_an_open_parent_like_workflowy()
         {
            let sample = sample();
            let change = split(&sample, "t2", "release".len(), "n1", &all(&sample));
            assert_eq!(titles(&change.outline, "t2"), json!(["", "write notes"]));
            assert_eq!(change.ops, [create_op("n1", "t2", 0, "")]);
        }

        #[test]
        fn opens_an_empty_row_above_when_enter_is_pressed_at_the_start_leaving_the_cursor_on_the_original_text()
         {
            let sample = sample();
            let change = split(&sample, "t5", 0, "n1", &all(&sample));
            assert_eq!(
                titles(&change.outline, ROOT),
                json!([
                    ["work", [["release", ["write notes"]], "expenses"]],
                    "",
                    "home"
                ])
            );
            assert_eq!(change.ops, [create_op("n1", ROOT, 1, "")]);
            assert_eq!(change.focus, Some(focus("t5", 0)));
        }

        #[test]
        fn adds_a_sibling_at_the_end_of_a_parent_whose_children_are_all_hidden_as_completed_so_enter_acts_as_it_does_on_a_row_without_children()
         {
            let outline = outline_of(&[
                todo(1, "trip", None),
                todo(2, "tickets", Some(1)).done(),
                todo(3, "home", None),
            ]);
            let change = split(
                &outline,
                "t1",
                "trip".len(),
                "n1",
                &shown(&outline, false, ""),
            );
            assert_eq!(change.ops, [create_op("n1", ROOT, 1, "")]);
            assert_eq!(
                rows(&change.outline, false, ""),
                [
                    Row {
                        key: "t1".into(),
                        depth: 0,
                        under: false
                    },
                    Row {
                        key: "n1".into(),
                        depth: 0,
                        under: false
                    },
                    Row {
                        key: "t3".into(),
                        depth: 0,
                        under: false
                    },
                ]
            );
        }

        #[test]
        fn adds_a_sibling_at_the_end_of_a_parent_whose_children_the_filter_hides_so_enter_follows_the_rows_on_screen_as_it_does_on_a_row_without_children()
         {
            let outline = outline_of(&[
                todo(1, "trip", None),
                todo(2, "tickets", Some(1)),
                todo(3, "home", None),
            ]);
            let change = split(
                &outline,
                "t1",
                "trip".len(),
                "n1",
                &shown(&outline, true, "trip"),
            );
            assert_eq!(change.ops, [create_op("n1", ROOT, 1, "")]);
        }

        #[test]
        fn starts_the_first_child_when_some_of_the_parents_children_are_on_screen_even_with_others_hidden_so_the_new_row_appears_right_under_it()
         {
            let outline = outline_of(&[
                todo(1, "trip", None),
                todo(2, "tickets", Some(1)).done(),
                todo(3, "passport", Some(1)),
                todo(4, "home", None),
            ]);
            let change = split(
                &outline,
                "t1",
                "trip".len(),
                "n1",
                &shown(&outline, false, ""),
            );
            assert_eq!(change.ops, [create_op("n1", "t1", 0, "")]);
        }

        #[test]
        fn leaves_the_note_with_the_head_of_the_title_since_the_note_was_written_about_the_row_it_is_under()
         {
            let outline = outline_of(&[
                todo(1, "book hotel near the station", None).noted("budget 20000 yen")
            ]);
            let change = split(&outline, "t1", "book hotel".len(), "n1", &all(&outline));
            assert_eq!(
                [
                    &change.outline.item("t1").note,
                    &change.outline.item("n1").note
                ],
                ["budget 20000 yen", ""]
            );
        }

        #[test]
        fn splits_at_the_end_of_a_multibyte_title_given_as_end_so_no_character_is_cut_in_half() {
            let outline = outline_of(&[todo(1, "牛乳を買う", None), todo(2, "home", None)]);
            let change = split(&outline, "t1", END, "n1", &all(&outline));
            assert_eq!(change.ops, [create_op("n1", ROOT, 1, "")]);
            let change = split(&outline, "t1", "牛乳".len() + 1, "n1", &all(&outline));
            assert_eq!(
                change.ops,
                [create_op("n1", ROOT, 1, "買う"), title_op("t1", "牛乳を")]
            );
        }
    }

    mod indent_and_outdent {
        use super::*;

        #[test]
        fn nests_a_row_as_the_last_child_of_the_row_above_and_opens_that_row_if_it_was_collapsed() {
            let outline = outline_of(&[
                todo(1, "a", None).folded(),
                todo(2, "a1", Some(1)),
                todo(3, "b", None),
            ]);
            let change = indent(&outline, "t3", &all(&outline)).unwrap();
            assert_eq!(titles(&change.outline, ROOT), json!([["a", ["a1", "b"]]]));
            assert!(!change.outline.item("t1").collapsed);
            assert_eq!(
                change.ops,
                [move_op("t3", "t1", 1), update_op("t1", None, Some(false))]
            );
        }

        #[test]
        fn skips_hidden_completed_rows_so_a_row_never_disappears_into_one_the_user_cannot_see() {
            let outline = outline_of(&[
                todo(1, "visible", None),
                todo(2, "hidden", None).done(),
                todo(3, "row", None),
            ]);
            let change = indent(&outline, "t3", &shown(&outline, false, "")).unwrap();
            assert_eq!(change.ops, [move_op("t3", "t1", 0)]);
        }

        #[test]
        fn does_nothing_when_the_only_rows_above_are_hidden_as_completed_rather_than_nesting_into_one_nobody_can_see()
         {
            let outline = outline_of(&[todo(1, "hidden", None).done(), todo(2, "row", None)]);
            assert_eq!(indent(&outline, "t2", &shown(&outline, false, "")), None);
        }

        #[test]
        fn skips_rows_the_filter_hides_so_a_row_is_nested_under_the_match_the_user_sees_above_it() {
            let outline = outline_of(&[
                todo(1, "buy milk", None),
                todo(2, "call mom", None),
                todo(3, "buy eggs", None),
            ]);
            let change = indent(&outline, "t3", &shown(&outline, true, "buy")).unwrap();
            assert_eq!(change.ops, [move_op("t3", "t1", 0)]);
        }

        #[test]
        fn does_nothing_for_the_first_row_in_its_list() {
            let sample = sample();
            assert_eq!(indent(&sample, "t1", &all(&sample)), None);
        }

        #[test]
        fn does_nothing_when_the_row_or_a_row_under_it_would_end_up_deeper_than_the_server_allows_rather_than_making_a_change_that_fails_to_save()
         {
            let parent = 62;
            let mut todos = chain(parent);
            todos.extend([
                todo(100, "a", Some(parent)),
                todo(101, "b", Some(parent)),
                todo(102, "under b", Some(101)),
                todo(103, "c", Some(parent)),
            ]);
            let outline = outline_of(&todos);
            assert_eq!(indent(&outline, "t101", &all(&outline)), None);
            assert_eq!(
                indent(&outline, "t103", &all(&outline)).unwrap().ops,
                [move_op("t103", "t101", 1)]
            );
        }

        #[test]
        fn places_an_outdented_row_right_after_its_old_parent_and_leaves_the_following_siblings_where_they_were()
         {
            let change = outdent(&sample(), "t2").unwrap();
            assert_eq!(
                titles(&change.outline, ROOT),
                json!([["work", ["expenses"]], ["release", ["write notes"]], "home"])
            );
            assert_eq!(change.ops, [move_op("t2", ROOT, 1)]);
        }

        #[test]
        fn does_nothing_for_a_top_level_row_which_has_no_parent_to_move_out_of() {
            assert_eq!(outdent(&sample(), "t1"), None);
        }
    }

    mod move_by {
        use super::*;

        #[test]
        fn sends_the_final_position_among_siblings_which_is_what_the_server_expects() {
            let sample = sample();
            let change = move_by(&sample, "t1", 1, &all(&sample)).unwrap();
            assert_eq!(
                titles(&change.outline, ROOT),
                json!(["home", ["work", [["release", ["write notes"]], "expenses"]]])
            );
            assert_eq!(change.ops, [move_op("t1", ROOT, 1)]);
        }

        #[test]
        fn stops_at_the_ends_of_the_list() {
            let sample = sample();
            assert_eq!(move_by(&sample, "t1", -1, &all(&sample)), None);
            assert_eq!(move_by(&sample, "t5", 1, &all(&sample)), None);
        }

        #[test]
        fn skips_hidden_completed_siblings_so_every_press_visibly_moves_the_row() {
            let outline = outline_of(&[
                todo(1, "A", None),
                todo(2, "Done", None).done(),
                todo(3, "B", None),
            ]);
            let down = move_by(&outline, "t1", 1, &shown(&outline, false, "")).unwrap();
            assert_eq!(row_keys(&down.outline, false, ""), ["t3", "t1"]);
            assert_eq!(down.ops, [move_op("t1", ROOT, 2)]);
            let up = move_by(&outline, "t3", -1, &shown(&outline, false, "")).unwrap();
            assert_eq!(row_keys(&up.outline, false, ""), ["t3", "t1"]);
            assert_eq!(up.ops, [move_op("t3", ROOT, 0)]);
        }

        #[test]
        fn does_nothing_when_the_only_siblings_that_way_are_hidden_as_completed_rather_than_making_a_move_nobody_can_see()
         {
            let outline = outline_of(&[todo(1, "A", None), todo(2, "Done", None).done()]);
            assert_eq!(
                move_by(&outline, "t1", 1, &shown(&outline, false, "")),
                None
            );
        }

        #[test]
        fn skips_siblings_the_filter_hides_so_the_row_moves_past_the_next_match_the_user_can_see() {
            let outline = outline_of(&[
                todo(1, "buy milk", None),
                todo(2, "call mom", None),
                todo(3, "buy eggs", None),
            ]);
            let down = move_by(&outline, "t1", 1, &shown(&outline, true, "buy")).unwrap();
            assert_eq!(row_keys(&down.outline, true, "buy"), ["t3", "t1"]);
            assert_eq!(down.ops, [move_op("t1", ROOT, 2)]);
        }
    }

    mod move_to {
        use super::*;

        #[test]
        fn puts_the_row_with_its_subtree_at_the_end_of_the_chosen_todos_children_in_a_single_move()
        {
            let change = move_to(&sample(), "t2", "t5");
            assert_eq!(
                titles(&change.outline, ROOT),
                json!([
                    ["work", ["expenses"]],
                    ["home", [["release", ["write notes"]]]]
                ])
            );
            assert_eq!(change.ops, [move_op("t2", "t5", 0)]);
        }

        #[test]
        fn sends_the_final_position_when_the_row_is_moved_to_the_end_of_its_own_parent_which_is_what_the_server_expects()
         {
            let change = move_to(&sample(), "t2", "t1");
            assert_eq!(
                titles(&change.outline, ROOT),
                json!([["work", ["expenses", ["release", ["write notes"]]]], "home"])
            );
            assert_eq!(change.ops, [move_op("t2", "t1", 1)]);
        }

        #[test]
        fn leaves_a_collapsed_todo_closed_so_the_rows_the_user_is_working_among_do_not_jump_around()
        {
            let outline = outline_of(&[
                todo(1, "a", None).folded(),
                todo(2, "a1", Some(1)),
                todo(3, "b", None),
            ]);
            let change = move_to(&outline, "t3", "t1");
            assert!(change.outline.item("t1").collapsed);
            assert_eq!(change.ops, [move_op("t3", "t1", 1)]);
        }
    }

    mod destinations {
        use super::*;

        fn to(key: &str, title: &str, path: &[&str]) -> Destination {
            Destination {
                key: key.to_owned(),
                title: title.to_owned(),
                path: path.iter().map(|&title| title.to_owned()).collect(),
            }
        }

        #[test]
        fn lists_every_other_todo_in_outline_order_with_the_titles_above_it_so_todos_with_the_same_name_can_be_told_apart()
         {
            assert_eq!(
                destinations(&sample(), "t5", true, ""),
                [
                    to("t1", "work", &[]),
                    to("t2", "release", &["work"]),
                    to("t3", "write notes", &["work", "release"]),
                    to("t4", "expenses", &["work"]),
                ]
            );
        }

        #[test]
        fn never_offers_the_row_itself_or_anything_under_it_since_the_server_refuses_to_put_a_todo_inside_itself()
         {
            assert_eq!(keys(destinations(&sample(), "t1", true, "")), ["t5"]);
        }

        #[test]
        fn finds_todos_by_title_regardless_of_case_even_inside_collapsed_ones_that_are_out_of_sight()
         {
            let outline = outline_of(&[
                todo(1, "work", None).folded(),
                todo(2, "Release", Some(1)),
                todo(3, "home", None),
            ]);
            assert_eq!(keys(destinations(&outline, "t3", true, " rel ")), ["t2"]);
        }

        #[test]
        fn leaves_out_completed_todos_and_everything_under_them_while_completed_todos_are_hidden() {
            let outline = outline_of(&[
                todo(1, "old", None).done(),
                todo(2, "inside old", Some(1)),
                todo(3, "open", None),
                todo(4, "row", None),
            ]);
            assert_eq!(keys(destinations(&outline, "t4", false, "")), ["t3"]);
            assert_eq!(
                keys(destinations(&outline, "t4", true, "")),
                ["t1", "t2", "t3"]
            );
        }

        #[test]
        fn leaves_out_todos_that_would_put_the_moved_row_or_a_row_under_it_deeper_than_the_server_allows()
         {
            let depth = 64;
            let mut todos = chain(depth);
            todos.extend([todo(100, "branch", None), todo(101, "leaf", Some(100))]);
            let outline = outline_of(&todos);
            let levels = |count: u64| (1..=count).map(key_for).collect::<Vec<_>>();
            assert_eq!(
                keys(destinations(&outline, "t100", true, "")),
                levels(depth - 2)
            );
            let mut expected = levels(depth - 1);
            expected.push("t100".to_owned());
            assert_eq!(keys(destinations(&outline, "t101", true, "")), expected);
        }

        #[test]
        fn leaves_out_todos_without_a_title_which_could_not_be_told_apart_in_the_list() {
            let outline = outline_of(&[
                todo(1, " ", None),
                todo(2, "under blank", Some(1)),
                todo(3, "row", None),
            ]);
            assert_eq!(
                destinations(&outline, "t3", true, ""),
                [to("t2", "under blank", &[" "])]
            );
        }
    }

    mod merge_into_previous {
        use super::*;

        fn merge(outline: &Outline, key: &str) -> Option<Change> {
            merge_into_previous(outline, &rows(outline, true, ""), key)
        }

        #[test]
        fn joins_a_row_onto_the_one_above_and_removes_it_leaving_the_cursor_at_the_seam() {
            let outline = outline_of(&[todo(1, "buy ", None), todo(2, "milk", None)]);
            let change = merge(&outline, "t2").unwrap();
            assert_eq!(titles(&change.outline, ROOT), json!(["buy milk"]));
            assert_eq!(
                change.ops,
                [title_op("t1", "buy milk"), delete_op("t2", &["t2"])]
            );
            assert_eq!(change.focus, Some(focus("t1", 4)));
        }

        #[test]
        fn puts_the_cursor_at_the_seam_after_a_multibyte_title_so_the_next_keystroke_lands_between_the_joined_texts()
         {
            let outline = outline_of(&[todo(1, "牛乳", None), todo(2, "を買う", None)]);
            let change = merge(&outline, "t2").unwrap();
            assert_eq!(change.focus, Some(focus("t1", "牛乳".len())));
        }

        #[test]
        fn refuses_when_the_row_above_sits_deeper_so_text_never_jumps_into_another_branch() {
            assert_eq!(merge(&sample(), "t5"), None);
        }

        #[test]
        fn refuses_when_the_row_has_children_so_backspace_never_deletes_a_subtree_by_accident() {
            assert_eq!(merge(&sample(), "t2"), None);
        }

        #[test]
        fn refuses_when_a_completed_row_is_involved_so_merging_never_silently_changes_what_is_done()
        {
            let into_done = outline_of(&[todo(1, "done", None).done(), todo(2, "open", None)]);
            assert_eq!(merge(&into_done, "t2"), None);
            let done_into_open = outline_of(&[todo(1, "open", None), todo(2, "done", None).done()]);
            assert_eq!(merge(&done_into_open, "t2"), None);
        }

        #[test]
        fn refuses_when_the_row_has_a_note_so_backspace_never_throws_away_what_was_written_under_it()
         {
            let outline = outline_of(&[
                todo(1, "buy ", None),
                todo(2, "milk", None).noted("the low-fat one"),
            ]);
            assert_eq!(merge(&outline, "t2"), None);
        }

        #[test]
        fn refuses_when_the_joined_title_would_be_longer_than_the_server_accepts() {
            let outline = outline_of(&[
                todo(1, &"x".repeat(1950), None),
                todo(2, &"y".repeat(51), None),
            ]);
            assert_eq!(merge(&outline, "t2"), None);
        }

        #[test]
        fn counts_the_joined_title_in_utf16_units_as_the_server_does_so_a_title_it_would_accept_is_not_refused()
         {
            let outline = outline_of(&[todo(1, &"あ".repeat(1999), None), todo(2, "い", None)]);
            assert!(merge(&outline, "t2").is_some());
        }

        #[test]
        fn refuses_an_emoji_title_that_fits_in_characters_but_not_in_the_utf16_units_the_server_counts()
         {
            let outline =
                outline_of(&[todo(1, &"\u{1F600}".repeat(1000), None), todo(2, "a", None)]);
            assert_eq!(merge(&outline, "t2"), None);
        }
    }

    mod remove {
        use super::*;

        #[test]
        fn deletes_the_whole_subtree_in_one_request_and_lists_every_removed_row_so_their_pending_edits_can_be_dropped()
         {
            let change = remove(&sample(), "t1");
            assert_eq!(titles(&change.outline, ROOT), json!(["home"]));
            assert_eq!(change.ops, [delete_op("t1", &["t1", "t2", "t3", "t4"])]);
        }
    }

    mod undo_for {
        use super::*;

        fn edits() -> Vec<(&'static str, Outline, Change)> {
            let sample = sample();
            let folded = outline_of(&[
                todo(1, "trip", None),
                todo(2, "packing", None).folded(),
                todo(3, "socks", Some(2)),
                todo(4, "passport", None),
            ]);
            let pair = outline_of(&[todo(1, "buy ", None), todo(2, "milk", None)]);
            let shown = all(&sample);
            vec![
                (
                    "split",
                    sample.clone(),
                    split(&sample, "t4", 3, "n1", &shown),
                ),
                (
                    "split into children",
                    sample.clone(),
                    split(&sample, "t2", "release".len(), "n1", &shown),
                ),
                (
                    "insert above",
                    sample.clone(),
                    split(&sample, "t5", 0, "n1", &shown),
                ),
                ("append", sample.clone(), append(&sample, ROOT, "n1")),
                (
                    "indent into a collapsed row",
                    folded.clone(),
                    indent(&folded, "t4", &all(&folded)).unwrap(),
                ),
                ("outdent", sample.clone(), outdent(&sample, "t3").unwrap()),
                (
                    "move up",
                    sample.clone(),
                    move_by(&sample, "t4", -1, &shown).unwrap(),
                ),
                (
                    "move to another branch",
                    sample.clone(),
                    move_to(&sample, "t5", "t2"),
                ),
                (
                    "move to the end of its own parent",
                    sample.clone(),
                    move_to(&sample, "t2", "t1"),
                ),
                ("complete", sample.clone(), toggle_done(&sample, "t3")),
                (
                    "collapse",
                    sample.clone(),
                    set_collapsed(&sample, "t1", true).unwrap(),
                ),
                ("delete a subtree", sample.clone(), remove(&sample, "t1")),
                (
                    "merge",
                    pair.clone(),
                    merge_into_previous(&pair, &rows(&pair, true, ""), "t2").unwrap(),
                ),
            ]
        }

        #[test]
        fn replays_each_edits_changes_into_the_outline_the_editor_shows_so_undo_starts_from_what_is_on_screen()
         {
            for (name, before, change) in edits() {
                assert_eq!(apply_ops(&before, &change.ops), change.outline, "{name}");
            }
        }

        #[test]
        fn takes_every_edit_back_to_exactly_the_outline_before_it_so_cmd_z_never_leaves_a_row_in_a_different_place()
         {
            for (name, before, change) in edits() {
                assert_eq!(
                    apply_ops(&change.outline, &undo_for(&before, &change.ops)),
                    before,
                    "{name}"
                );
            }
        }

        #[test]
        fn recreates_a_deleted_subtree_parents_first_under_the_same_keys_with_every_note_completion_and_fold_it_had()
         {
            let outline = outline_of(&[
                todo(1, "trip", None).folded(),
                todo(2, "packing", Some(1)).noted("check the weather"),
                todo(3, "socks", Some(2)).done(),
                todo(4, "home", None),
            ]);
            let recreate = |key: &str, parent: &str, title: &str, note: &str| Op::Create {
                key: key.to_owned(),
                parent: parent.to_owned(),
                index: 0,
                title: title.to_owned(),
                note: Some(note.to_owned()),
            };
            assert_eq!(
                undo_for(&outline, &remove(&outline, "t1").ops),
                [
                    recreate("t1", ROOT, "trip", ""),
                    update_op("t1", None, Some(true)),
                    recreate("t2", "t1", "packing", "check the weather"),
                    recreate("t3", "t2", "socks", ""),
                    update_op("t3", Some(true), None),
                ]
            );
        }
    }

    mod visible_rows {
        use super::*;

        fn outline() -> Outline {
            outline_of(&[
                todo(1, "project", None).done(),
                todo(2, "step", Some(1)),
                todo(3, "folded", None).folded(),
                todo(4, "inside fold", Some(3)),
            ])
        }

        #[test]
        fn greys_the_rows_under_a_completed_row_without_marking_them_done_themselves() {
            let under: Vec<(Key, bool)> = rows(&outline(), true, "")
                .into_iter()
                .map(|row| (row.key, row.under))
                .collect();
            assert_eq!(
                under,
                [
                    ("t1".to_owned(), false),
                    ("t2".to_owned(), true),
                    ("t3".to_owned(), false)
                ]
            );
        }

        #[test]
        fn hides_a_completed_row_together_with_its_subtree_when_completed_rows_are_hidden() {
            assert_eq!(row_keys(&outline(), false, ""), ["t3"]);
        }

        #[test]
        fn shows_filter_matches_with_their_parents_even_inside_collapsed_rows() {
            assert_eq!(row_keys(&outline(), true, "INSIDE"), ["t3", "t4"]);
        }

        #[test]
        fn finds_a_row_by_its_note_so_details_kept_out_of_the_title_can_still_be_filtered_for() {
            let noted = outline_of(&[
                todo(1, "dentist", None).noted("Only weekday mornings"),
                todo(2, "milk", None),
            ]);
            assert_eq!(row_keys(&noted, true, "weekday"), ["t1"]);
        }

        #[test]
        fn keeps_rows_created_during_a_filter_visible_even_though_their_text_does_not_match_yet() {
            let keep = HashSet::from(["t1".to_owned()]);
            let keys: Vec<Key> = visible_rows(&outline(), true, "inside", &keep)
                .into_iter()
                .map(|row| row.key)
                .collect();
            assert_eq!(keys, ["t1", "t3", "t4"]);
        }
    }
}
