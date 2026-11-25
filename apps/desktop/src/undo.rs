use std::collections::VecDeque;
use std::time::Duration;

use crate::outline::{Change, Focus, Key, Op, Outline, apply_ops, same_outline, undo_for};

const LIMIT: usize = 50;
const TYPING_PAUSE: Duration = Duration::from_millis(1000);

struct Typing {
    key: Key,
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

    pub fn type_text(&mut self, before: &Outline, after: &Outline, key: &str, composing: bool) {
        let now = (self.now)();
        let going = self
            .steps
            .back()
            .and_then(|step| step.typing.as_ref())
            .filter(|typing| {
                typing.key == key
                    && (typing.composing || now.saturating_sub(typing.at) <= TYPING_PAUSE)
            })
            .map(|typing| typing.original.clone());
        let continuing = going.is_some();
        let original = going.unwrap_or_else(|| before.item(key).title.clone());
        let focus = Some(Focus {
            key: key.to_owned(),
            pos: edit_end(&original, &after.item(key).title),
        });
        let typing = Some(Typing {
            key: key.to_owned(),
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
                ops: vec![Op::Title {
                    key: key.to_owned(),
                    title: original,
                }],
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

    pub fn clear(&mut self) {
        self.steps.clear();
    }

    pub fn end(&mut self) {
        if let Some(step) = self.steps.back_mut() {
            step.typing = None;
        }
    }

    pub fn undo(&mut self, current: &Outline) -> Option<Change> {
        let step = self.steps.pop_back()?;
        if !same_outline(current, &step.after) {
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
    use crate::outline::fixtures::{Mark, outline_of, shown, todo, update_op};
    use crate::outline::{
        Outline, ROOT, indent, remove, rename, set_comments, set_icon, split, toggle_done,
    };
    use crate::wire::Todo;

    fn row(id: u64, title: &str) -> Todo {
        todo(id, title, None)
    }

    fn start() -> Outline {
        outline_of(&[row(1, "milk"), row(2, "eggs")])
    }

    fn all(outline: &Outline) -> std::collections::HashSet<Key> {
        shown(outline, true)
    }

    fn at(key: &str, pos: usize) -> Focus {
        Focus {
            key: key.to_owned(),
            pos,
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
        history.type_text(outline, &next, key, composing);
        next
    }

    #[test]
    fn takes_edits_back_newest_first_and_returns_the_cursor_to_where_it_was_before_each_one() {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, Some(at("t2", 4)));
        let nested = indent(&added.outline, "n1", &all(&added.outline)).unwrap();
        history.record(&added.outline, &nested, Some(at("n1", 0)));

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
        assert_eq!(unnested.focus, Some(at("n1", 0)));

        let removed = history.undo(&unnested.outline).unwrap();
        assert_eq!(removed.outline, start);
        assert_eq!(
            removed.ops,
            [Op::Delete {
                key: "n1".into(),
                removed: vec!["n1".into()]
            }]
        );
        assert_eq!(removed.focus, Some(at("t2", 4)));

        assert_eq!(history.undo(&removed.outline), None);
    }

    #[test]
    fn takes_back_a_completion_with_every_todo_it_completed_under_it_while_a_later_reopen_from_the_check_leaves_those_done()
     {
        let start = outline_of(&[
            row(1, "groceries"),
            todo(2, "milk", Some(1)).done(),
            todo(3, "bread", Some(1)),
            row(4, "trip"),
        ]);
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let completed = toggle_done(&start, "t1");
        history.record(&start, &completed, None);
        let reopened = toggle_done(&completed.outline, "t1");
        history.record(&completed.outline, &reopened, None);
        assert!(!reopened.outline.item("t1").done);
        assert!(reopened.outline.item("t3").done);

        let completed_again = history.undo(&reopened.outline).unwrap();
        assert_eq!(completed_again.outline, completed.outline);
        assert_eq!(completed_again.ops, [update_op("t1", Some(true), None)]);

        let undone = history.undo(&completed_again.outline).unwrap();
        assert_eq!(undone.outline, start);
        assert_eq!(
            undone.ops,
            [
                update_op("t1", Some(false), None),
                update_op("t3", Some(false), None)
            ]
        );
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
    fn puts_the_cursor_where_the_undone_typing_was_in_the_row_it_was_typed_in() {
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let prefixed = type_title(&mut history, &start(), "t1", "oat milk", false);
        history.end();
        let ranged = type_title(&mut history, &prefixed, "t2", "free-range eggs", false);
        history.end();
        let trimmed = type_title(&mut history, &ranged, "t2", "free eggs", false);

        let untrimmed = history.undo(&trimmed).unwrap();
        assert_eq!(untrimmed.focus, Some(at("t2", "free-range".len())));
        let unranged = history.undo(&untrimmed.outline).unwrap();
        assert_eq!(unranged.focus, Some(at("t2", 0)));
        assert_eq!(
            history.undo(&unranged.outline).unwrap().focus,
            Some(at("t1", 0))
        );
    }

    #[test]
    fn keeps_every_step_when_a_comment_changes_a_rows_count_since_comments_are_not_taken_back_by_cmd_z()
     {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, None);
        let commented = set_comments(&added.outline, "t1", 1);

        let undone = history.undo(&commented).unwrap();

        assert_eq!(
            undone.ops,
            [Op::Delete {
                key: "n1".into(),
                removed: vec!["n1".into()]
            }]
        );
        assert_eq!(undone.outline.item("t1").comments, 1);
    }

    #[test]
    fn brings_a_deleted_row_back_with_its_comment_count_so_the_row_shows_its_log_again_at_once() {
        let start = outline_of(&[row(1, "milk"), row(2, "dentist").commented(3)]);
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let removed = remove(&start, "t2");
        history.record(&start, &removed, Some(at("t2", 0)));

        let restored = history.undo(&removed.outline).unwrap();

        assert_eq!(restored.outline, start);
        assert_eq!(restored.outline.item("t2").comments, 3);
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

        assert_eq!(history.undo(&thumbs_up).unwrap().focus, Some(at("t1", 0)));
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
            Some(at("t1", "Dance 🕺".len()))
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
    fn takes_back_setting_an_icon_in_one_step_leaving_the_row_without_one_as_before() {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let iconed = set_icon(&start, "t1", Some("\u{1F95B}")).unwrap();
        history.record(&start, &iconed, Some(at("t1", 4)));

        let undone = history.undo(&iconed.outline).unwrap();

        assert_eq!(undone.outline, start);
        assert_eq!(
            undone.ops,
            [Op::Icon {
                key: "t1".into(),
                icon: None
            }]
        );
        assert_eq!(undone.focus, Some(at("t1", 4)));
        assert_eq!(history.undo(&undone.outline), None);
    }

    #[test]
    fn forgets_every_step_once_an_icon_was_changed_elsewhere_so_undo_never_overwrites_the_icon_picked_on_another_device()
     {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let added = split(&start, "t2", 4, "n1", &all(&start));
        history.record(&start, &added, None);
        let iconed_on_phone = Outline::from_todos(
            &[
                row(1, "milk").iconed("\u{1F95B}"),
                row(2, "eggs"),
                row(3, ""),
            ],
            |id| {
                if id == 3 {
                    "n1".to_owned()
                } else {
                    format!("t{id}")
                }
            },
        );

        assert_eq!(history.undo(&iconed_on_phone), None);
    }

    #[test]
    fn forgets_every_step_once_cleared_so_an_undo_cannot_put_back_an_image_that_was_deleted_meanwhile()
     {
        let start = start();
        let mut history = UndoHistory::with_clock(|| Duration::ZERO);
        let pictured = set_icon(&start, "t1", Some("image:a1")).unwrap();
        history.record(&start, &pictured, None);
        let replaced = set_icon(&pictured.outline, "t1", Some("\u{1F95B}")).unwrap();
        history.record(&pictured.outline, &replaced, None);

        history.clear();

        assert_eq!(history.undo(&replaced.outline), None);
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
