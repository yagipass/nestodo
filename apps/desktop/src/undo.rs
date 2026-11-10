use std::collections::VecDeque;
use std::time::Duration;

use crate::outline::{Change, Focus, Item, Key, Op, Outline, apply_ops, undo_for};

const LIMIT: usize = 50;
const TYPING_PAUSE: Duration = Duration::from_millis(1000);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Note,
}

impl Field {
    fn text(self, item: &Item) -> &str {
        match self {
            Field::Title => &item.title,
            Field::Note => &item.note,
        }
    }

    fn restore(self, key: &str, text: String) -> Op {
        match self {
            Field::Title => Op::Title {
                key: key.to_owned(),
                title: text,
            },
            Field::Note => Op::Note {
                key: key.to_owned(),
                note: text,
            },
        }
    }
}

struct Typing {
    key: Key,
    field: Field,
    original: String,
    at: Duration,
    composing: bool,
}

struct Step {
    ops: Vec<Op>,
    focus: Option<Focus>,
    after: Outline,
    typing: Option<Typing>,
}

fn edit_end(before: &str, after: &str) -> usize {
    let (b, a) = (before.as_bytes(), after.as_bytes());
    let max = b.len().min(a.len());
    let mut prefix = 0;
    while prefix < max && b[prefix] == a[prefix] {
        prefix += 1;
    }
    while !before.is_char_boundary(prefix) || !after.is_char_boundary(prefix) {
        prefix -= 1;
    }
    let mut suffix = 0;
    while suffix < max - prefix && b[b.len() - 1 - suffix] == a[a.len() - 1 - suffix] {
        suffix += 1;
    }
    while !before.is_char_boundary(b.len() - suffix) || !after.is_char_boundary(a.len() - suffix) {
        suffix -= 1;
    }
    b.len() - suffix
}

pub struct UndoHistory {
    steps: VecDeque<Step>,
    now: Box<dyn Fn() -> Duration>,
}

impl UndoHistory {
    pub fn with_clock(clock: impl Fn() -> Duration + 'static) -> Self {
        UndoHistory {
            steps: VecDeque::new(),
            now: Box::new(clock),
        }
    }

    pub fn record(&mut self, before: &Outline, change: &Change, focus: Option<Focus>) {
        self.push(Step {
            ops: undo_for(before, &change.ops),
            focus,
            after: change.outline.clone(),
            typing: None,
        });
    }

    pub fn type_text(
        &mut self,
        before: &Outline,
        after: &Outline,
        key: &str,
        field: Field,
        composing: bool,
    ) {
        let now = (self.now)();
        let going = self
            .steps
            .back()
            .and_then(|step| step.typing.as_ref())
            .filter(|typing| {
                typing.key == key
                    && typing.field == field
                    && (typing.composing || now.saturating_sub(typing.at) <= TYPING_PAUSE)
            })
            .map(|typing| typing.original.clone());
        let continuing = going.is_some();
        let original = going.unwrap_or_else(|| field.text(before.item(key)).to_owned());
        let focus = Some(Focus {
            key: key.to_owned(),
            pos: edit_end(&original, field.text(after.item(key))),
            note: field == Field::Note,
        });
        let typing = Some(Typing {
            key: key.to_owned(),
            field,
            original: original.clone(),
            at: now,
            composing,
        });
        match self.steps.back_mut().filter(|_| continuing) {
            Some(step) => {
                step.after = after.clone();
                step.focus = focus;
                step.typing = typing;
            }
            None => self.push(Step {
                ops: vec![field.restore(key, original)],
                focus,
                after: after.clone(),
                typing,
            }),
        }
    }

    pub fn composed(&mut self) {
        let now = (self.now)();
        if let Some(typing) = self.steps.back_mut().and_then(|step| step.typing.as_mut()) {
            typing.composing = false;
            typing.at = now;
        }
    }

    pub fn end(&mut self) {
        if let Some(step) = self.steps.back_mut() {
            step.typing = None;
        }
    }

    pub fn undo(&mut self, current: &Outline) -> Option<Change> {
        let step = self.steps.pop_back()?;
        if *current != step.after {
            self.steps.clear();
            return None;
        }
        Some(Change {
            outline: apply_ops(current, &step.ops),
            ops: step.ops,
            focus: step.focus,
        })
    }

    fn push(&mut self, step: Step) {
        self.end();
        self.steps.push_back(step);
        if self.steps.len() > LIMIT {
            self.steps.pop_front();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use super::*;
    use crate::outline::fixtures::{outline_of, shown, todo};
    use crate::outline::{Outline, ROOT, indent, rename, set_note, split};
    use crate::wire::Todo;

    fn row(id: u64, title: &str) -> Todo {
        todo(id, title, None)
    }

    fn start() -> Outline {
        outline_of(&[row(1, "milk"), row(2, "eggs")])
    }

    fn all(outline: &Outline) -> std::collections::HashSet<Key> {
        shown(outline, true, "")
    }

    fn at(key: &str, pos: usize, note: bool) -> Focus {
        Focus {
            key: key.to_owned(),
            pos,
            note,
        }
    }

    fn fake_clock() -> (Rc<Cell<u64>>, UndoHistory) {
        let now = Rc::new(Cell::new(0));
        let clock = Rc::clone(&now);
        (
            now,
            UndoHistory::with_clock(move || Duration::from_millis(clock.get())),
        )
    }

    fn type_title(
        history: &mut UndoHistory,
        outline: &Outline,
        key: &str,
        title: &str,
        composing: bool,
    ) -> Outline {
        let next = rename(outline, key, title);
        history.type_text(outline, &next, key, Field::Title, composing);
        next
    }

    fn type_note(history: &mut UndoHistory, outline: &Outline, key: &str, note: &str) -> Outline {
        let next = set_note(outline, key, note);
        history.type_text(outline, &next, key, Field::Note, false);
        next
    }

    #[test]
    fn takes_edits_back_newest_first_and_returns_the_cursor_to_where_it_was_before_each_one() {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, Some(at("t2", 4, false)));
        let nested = indent(&added.outline, "n1", &all(&added.outline)).unwrap();
        history.record(&added.outline, &nested, Some(at("n1", 0, false)));

        let unnested = history.undo(&nested.outline).unwrap();
        assert_eq!(unnested.outline, added.outline);
        assert_eq!(
            unnested.ops,
            [Op::Move {
                key: "n1".into(),
                parent: ROOT.into(),
                index: 2
            }]
        );
        assert_eq!(unnested.focus, Some(at("n1", 0, false)));

        let removed = history.undo(&unnested.outline).unwrap();
        assert_eq!(removed.outline, start);
        assert_eq!(
            removed.ops,
            [Op::Delete {
                key: "n1".into(),
                removed: vec!["n1".into()]
            }]
        );
        assert_eq!(removed.focus, Some(at("t2", 4, false)));

        assert_eq!(history.undo(&removed.outline), None);
    }

    #[test]
    fn takes_back_a_burst_of_typing_in_one_step_and_starts_the_next_step_after_a_pause_of_more_than_a_second()
     {
        let start = start();
        let (now, mut history) = fake_clock();
        let mut outline = type_title(&mut history, &start, "t1", "milk!", false);
        now.set(1000);
        outline = type_title(&mut history, &outline, "t1", "milk!!", false);
        now.set(2000);
        outline = type_title(&mut history, &outline, "t1", "milk!!!", false);
        now.set(3001);
        outline = type_title(&mut history, &outline, "t1", "milk!!!?", false);

        let first = history.undo(&outline).unwrap();
        assert_eq!(
            first.ops,
            [Op::Title {
                key: "t1".into(),
                title: "milk!!!".into()
            }]
        );
        let second = history.undo(&first.outline).unwrap();
        assert_eq!(
            second.ops,
            [Op::Title {
                key: "t1".into(),
                title: "milk".into()
            }]
        );
        assert_eq!(second.outline, start);
    }

    #[test]
    fn keeps_an_ime_conversion_in_one_step_even_when_the_user_stops_to_pick_a_candidate_and_splits_only_on_a_pause_between_conversions()
     {
        let (now, mut history) = fake_clock();
        let mut outline = type_title(&mut history, &start(), "t2", "niunai", true);
        now.set(3000);
        outline = type_title(&mut history, &outline, "t2", "牛奶", true);
        history.composed();
        now.set(3500);
        outline = type_title(&mut history, &outline, "t2", "牛奶和", true);
        history.composed();
        now.set(6000);
        outline = type_title(&mut history, &outline, "t2", "牛奶和面包", true);
        history.composed();

        let first = history.undo(&outline).unwrap();
        assert_eq!(first.outline.item("t2").title, "牛奶和");
        assert_eq!(
            history
                .undo(&first.outline)
                .unwrap()
                .outline
                .item("t2")
                .title,
            "eggs"
        );
    }

    #[test]
    fn starts_a_new_step_for_typing_after_the_user_leaves_the_field_or_makes_another_edit_so_one_cmd_z_never_takes_back_more_than_that()
     {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let mut outline = type_title(&mut history, &start(), "t1", "milk!", false);
        history.end();
        outline = type_title(&mut history, &outline, "t1", "milk!!", false);
        let nested = indent(&outline, "t2", &all(&outline)).unwrap();
        history.record(&outline, &nested, None);
        outline = type_title(&mut history, &nested.outline, "t1", "milk!!!", false);

        let mut titles = Vec::new();
        while let Some(change) = history.undo(&outline) {
            titles.push(change.outline.item("t1").title.clone());
            outline = change.outline;
        }
        assert_eq!(titles, ["milk!!", "milk!!", "milk!", "milk"]);
    }

    #[test]
    fn starts_a_new_step_for_typing_right_after_an_undone_edit_so_the_next_cmd_z_takes_back_only_what_was_typed_since()
     {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let outline = type_title(&mut history, &start(), "t1", "milk!", false);
        let nested = indent(&outline, "t2", &all(&outline)).unwrap();
        history.record(&outline, &nested, None);
        let unnested = history.undo(&nested.outline).unwrap();

        let retyped = type_title(&mut history, &unnested.outline, "t1", "milk!!", false);

        assert_eq!(
            history.undo(&retyped).unwrap().outline.item("t1").title,
            "milk!"
        );
    }

    #[test]
    fn puts_the_cursor_where_the_undone_typing_was_in_the_note_when_the_note_was_typed_in() {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let prefixed = type_title(&mut history, &start(), "t1", "oat milk", false);
        history.end();
        let noted = type_note(&mut history, &prefixed, "t2", "free-range");
        history.end();
        let trimmed = type_note(&mut history, &noted, "t2", "free");

        let untrimmed = history.undo(&trimmed).unwrap();
        assert_eq!(untrimmed.focus, Some(at("t2", "free-range".len(), true)));
        let unnoted = history.undo(&untrimmed.outline).unwrap();
        assert_eq!(unnoted.focus, Some(at("t2", 0, true)));
        assert_eq!(
            history.undo(&unnoted.outline).unwrap().focus,
            Some(at("t1", 0, false))
        );
    }

    #[test]
    fn puts_the_cursor_back_where_an_emoji_was_typed_in_front_of_another_emoji_not_after_the_one_left_so_the_next_keystroke_lands_where_the_undone_one_was()
     {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let thumbs_up = type_title(
            &mut history,
            &outline_of(&[row(1, "😀 Dentist")]),
            "t1",
            "👍😀 Dentist",
            false,
        );

        assert_eq!(
            history.undo(&thumbs_up).unwrap().focus,
            Some(at("t1", 0, false))
        );
    }

    #[test]
    fn puts_the_cursor_after_an_emoji_whose_replacement_is_undone_never_between_its_two_halves_where_the_next_keystroke_would_break_it()
     {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let pleading = type_title(
            &mut history,
            &outline_of(&[row(1, "Dance 🕺")]),
            "t1",
            "Dance 🥺",
            false,
        );

        assert_eq!(
            history.undo(&pleading).unwrap().focus,
            Some(at("t1", "Dance 🕺".len(), false))
        );
    }

    #[test]
    fn keeps_every_step_when_reloading_brings_back_the_same_outline_so_switching_tabs_does_not_cost_the_user_their_undo()
     {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, None);
        let reloaded = Outline::from_todos(&[row(1, "milk"), row(2, "eggs"), row(3, "")], |id| {
            if id == 3 {
                "n1".to_owned()
            } else {
                format!("t{id}")
            }
        });

        assert_eq!(history.undo(&reloaded).unwrap().outline, start);
    }

    #[test]
    fn forgets_every_step_once_the_outline_was_changed_elsewhere_so_undo_never_overwrites_an_edit_made_on_another_device()
     {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, None);
        let nested = indent(&added.outline, "n1", &all(&added.outline)).unwrap();
        history.record(&added.outline, &nested, None);
        let renamed_on_phone = rename(&nested.outline, "t1", "oat milk");

        assert_eq!(history.undo(&renamed_on_phone), None);
        assert_eq!(history.undo(&added.outline), None);
    }

    #[test]
    fn keeps_only_the_latest_fifty_steps_so_a_long_session_does_not_hold_every_outline_it_ever_showed()
     {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let mut outline = start();
        for i in 0..=50 {
            outline = type_title(&mut history, &outline, "t1", &format!("milk {i}"), false);
            history.end();
        }
        let mut undone = 0;
        while let Some(change) = history.undo(&outline) {
            outline = change.outline;
            undone += 1;
        }
        assert_eq!(undone, 50);
        assert_eq!(outline.item("t1").title, "milk 0");
    }
}
