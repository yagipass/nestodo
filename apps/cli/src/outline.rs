use std::collections::{HashMap, HashSet};

use crate::error::Error;
use crate::todo::{Todo, printable};

#[derive(Default)]
pub struct Outline {
    todos: HashMap<u64, Todo>,
    children: HashMap<Option<u64>, Vec<u64>>,
}

#[derive(Debug, PartialEq)]
pub struct Move {
    pub id: u64,
    pub parent: Option<u64>,
    pub index: usize,
}

#[derive(Debug, PartialEq)]
pub struct Removal {
    pub id: u64,
    pub under: usize,
}

impl Outline {
    pub fn new(mut todos: Vec<Todo>) -> Self {
        todos.sort_by_key(|todo| todo.position);
        let mut outline = Self::default();
        for todo in todos {
            outline
                .children
                .entry(todo.parent_id)
                .or_default()
                .push(todo.id);
            outline.todos.insert(todo.id, todo);
        }
        outline
    }

    pub fn get(&self, id: u64) -> Result<&Todo, Error> {
        self.todos.get(&id).ok_or(Error::NotFound { id })
    }

    pub fn children(&self, parent: Option<u64>) -> Vec<&Todo> {
        self.ids(parent)
            .iter()
            .filter_map(|id| self.todos.get(id))
            .collect()
    }

    pub fn path(&self, parent: Option<u64>) -> String {
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        let mut current = parent.and_then(|id| self.todos.get(&id));
        while let Some(todo) = current.filter(|todo| seen.insert(todo.id)) {
            names.push(todo.title.as_str());
            current = todo.parent_id.and_then(|id| self.todos.get(&id));
        }
        names.push("Home");
        names.reverse();
        printable(&names.join(" › "))
    }

    pub fn update(&mut self, todo: Todo) {
        self.todos.insert(todo.id, todo);
    }

    pub fn indent(&mut self, id: u64) -> Result<Move, Error> {
        let parent = self.get(id)?.parent_id;
        let siblings = self.ids(parent);
        match siblings.iter().position(|&sibling| sibling == id) {
            Some(index) if index > 0 => {
                let above = siblings[index - 1];
                self.place(id, Some(above), None)
            }
            _ => Err(Error::Invalid(format!(
                "{id} is the first todo under {}; there is nothing above to nest under",
                self.path(parent)
            ))),
        }
    }

    pub fn outdent(&mut self, id: u64) -> Result<Move, Error> {
        let Some(parent) = self.get(id)?.parent_id else {
            return Err(Error::Invalid(format!("{id} is already at the top level")));
        };
        let grandparent = self.get(parent)?.parent_id;
        self.place(id, grandparent, Some(parent))
    }

    pub fn plan_moves(
        &mut self,
        ids: &[u64],
        to: Option<u64>,
        after: Option<u64>,
    ) -> Result<Vec<Move>, Error> {
        let parent = match after {
            Some(after) => {
                let parent = self.get(after)?.parent_id;
                if let Some(to) = to
                    && parent != Some(to)
                {
                    self.get(to)?;
                    return Err(Error::Invalid(format!(
                        "{after} is not directly under {to}"
                    )));
                }
                parent
            }
            None => to,
        };
        let mut anchor = after;
        let mut moves: Vec<Move> = Vec::new();
        for &id in ids {
            if moves.iter().any(|planned| planned.id == id) {
                continue;
            }
            if Some(id) == after {
                return Err(Error::Invalid(format!("cannot move {id} after itself")));
            }
            moves.push(self.place(id, parent, anchor)?);
            if anchor.is_some() {
                anchor = Some(id);
            }
        }
        Ok(moves)
    }

    pub fn plan_removals(&mut self, ids: &[u64], recursive: bool) -> Result<Vec<Removal>, Error> {
        for &id in ids {
            self.get(id)?;
        }
        let mut removals = Vec::new();
        for &id in ids {
            if !self.todos.contains_key(&id) {
                continue;
            }
            let under = self.count_under(id);
            if under > 0 && !recursive {
                let (todos, them) = if under == 1 {
                    ("todo", "it")
                } else {
                    ("todos", "them")
                };
                return Err(Error::Invalid(format!(
                    "{id} has {under} {todos} under it; pass -r to delete {them} too"
                )));
            }
            self.remove(id);
            removals.push(Removal { id, under });
        }
        Ok(removals)
    }

    pub fn check_completions(&self, ids: &[u64], recursive: bool) -> Result<(), Error> {
        for &id in ids {
            self.get(id)?;
        }
        if recursive {
            return Ok(());
        }
        for &id in ids {
            let open = self
                .under(id)
                .into_iter()
                .filter(|under| !ids.contains(under) && !self.todos[under].is_completed())
                .count();
            if open > 0 {
                let (todos, them) = if open == 1 {
                    ("todo", "it")
                } else {
                    ("todos", "them")
                };
                return Err(Error::Invalid(format!(
                    "{id} has {open} open {todos} under it; pass -r to mark {them} done too"
                )));
            }
        }
        Ok(())
    }

    pub fn complete_under(&mut self, id: u64) {
        let Some(completed_at) = self
            .todos
            .get(&id)
            .and_then(|todo| todo.completed_at.clone())
        else {
            return;
        };
        for under in self.under(id) {
            let todo = self
                .todos
                .get_mut(&under)
                .expect("listed under a known todo");
            if !todo.is_completed() {
                todo.completed_at = Some(completed_at.clone());
            }
        }
    }

    fn ids(&self, parent: Option<u64>) -> &[u64] {
        self.children.get(&parent).map_or(&[], Vec::as_slice)
    }

    fn under(&self, id: u64) -> Vec<u64> {
        let mut seen = HashSet::from([id]);
        let mut pending = vec![id];
        let mut under = Vec::new();
        while let Some(parent) = pending.pop() {
            for &child in self.ids(Some(parent)) {
                if seen.insert(child) {
                    pending.push(child);
                    under.push(child);
                }
            }
        }
        under
    }

    fn count_under(&self, id: u64) -> usize {
        self.under(id).len()
    }

    fn is_within(&self, id: u64, ancestor: u64) -> bool {
        let mut seen = HashSet::new();
        let mut current = Some(id);
        while let Some(at) = current.filter(|&at| seen.insert(at)) {
            if at == ancestor {
                return true;
            }
            current = self.todos.get(&at).and_then(|todo| todo.parent_id);
        }
        false
    }

    fn place(&mut self, id: u64, parent: Option<u64>, after: Option<u64>) -> Result<Move, Error> {
        let old = self.get(id)?.parent_id;
        if let Some(parent) = parent {
            self.get(parent)?;
            if self.is_within(parent, id) {
                return Err(Error::Invalid(format!(
                    "cannot move {id} into its own subtree"
                )));
            }
        }
        self.children
            .entry(old)
            .or_default()
            .retain(|&sibling| sibling != id);
        let siblings = self.children.entry(parent).or_default();
        let index = after
            .and_then(|after| siblings.iter().position(|&sibling| sibling == after))
            .map_or(siblings.len(), |index| index + 1);
        siblings.insert(index, id);
        if let Some(todo) = self.todos.get_mut(&id) {
            todo.parent_id = parent;
        }
        Ok(Move { id, parent, index })
    }

    fn remove(&mut self, id: u64) {
        for child in self.children.remove(&Some(id)).unwrap_or_default() {
            self.remove(child);
        }
        if let Some(todo) = self.todos.remove(&id) {
            self.children
                .entry(todo.parent_id)
                .or_default()
                .retain(|&sibling| sibling != id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todo(id: u64, parent: Option<u64>, position: usize, title: &str) -> Todo {
        Todo {
            id,
            parent_id: parent,
            position,
            title: title.into(),
            completed_at: None,
            collapsed: false,
        }
    }

    fn sample() -> Outline {
        Outline::new(vec![
            todo(1, None, 0, "Work"),
            todo(4, Some(1), 0, "v2 release"),
            todo(7, Some(4), 0, "Show the CLI list as a tree"),
            todo(8, Some(4), 1, "Check the Android IME toolbar"),
            todo(9, Some(4), 2, "Build the web outline screen"),
            todo(10, Some(4), 3, "Draft the release notes"),
            todo(5, Some(1), 1, "Prepare for the 1on1"),
            todo(2, None, 1, "Chores"),
            todo(11, Some(2), 0, "Milk"),
            todo(18, None, 2, "Book the dentist"),
        ])
    }

    fn order(outline: &Outline, parent: Option<u64>) -> Vec<u64> {
        outline
            .children(parent)
            .iter()
            .map(|todo| todo.id)
            .collect()
    }

    fn step(id: u64, parent: Option<u64>, index: usize) -> Move {
        Move { id, parent, index }
    }

    #[test]
    fn keeps_siblings_in_position_order_even_if_the_server_order_changes() {
        let outline = Outline::new(vec![
            todo(1, None, 1, "b"),
            todo(3, Some(2), 0, "a1"),
            todo(2, None, 0, "a"),
        ]);
        assert_eq!(order(&outline, None), [2, 1]);
        assert_eq!(order(&outline, Some(2)), [3]);
    }

    #[test]
    fn paths_read_like_breadcrumbs_from_home() {
        let outline = sample();
        assert_eq!(outline.path(None), "Home");
        assert_eq!(outline.path(Some(4)), "Home › Work › v2 release");
    }

    #[test]
    fn moving_after_a_later_sibling_sends_the_index_it_ends_up_at_once_taken_out() {
        let mut outline = sample();
        assert_eq!(
            outline.plan_moves(&[7], None, Some(9)).unwrap(),
            [step(7, Some(4), 2)]
        );
        assert_eq!(order(&outline, Some(4)), [8, 9, 7, 10]);
    }

    #[test]
    fn moves_several_todos_after_the_anchor_in_the_order_they_were_given() {
        let mut outline = sample();
        assert_eq!(
            outline.plan_moves(&[10, 7], None, Some(8)).unwrap(),
            [step(10, Some(4), 2), step(7, Some(4), 2)]
        );
        assert_eq!(order(&outline, Some(4)), [8, 10, 7, 9]);
    }

    #[test]
    fn moving_into_a_todo_appends_so_the_server_puts_them_last_in_order() {
        let mut outline = sample();
        assert_eq!(
            outline.plan_moves(&[18, 5, 18], Some(2), None).unwrap(),
            [step(18, Some(2), 1), step(5, Some(2), 2)]
        );
        assert_eq!(order(&outline, Some(2)), [11, 18, 5]);
    }

    #[test]
    fn moving_into_the_todo_it_is_already_under_sends_the_end_index_counting_only_the_others() {
        let mut outline = sample();
        assert_eq!(
            outline.plan_moves(&[7, 5], Some(4), None).unwrap(),
            [step(7, Some(4), 3), step(5, Some(4), 4)]
        );
        assert_eq!(order(&outline, Some(4)), [8, 9, 10, 7, 5]);
    }

    #[test]
    fn moving_after_a_top_level_todo_moves_to_the_top_level() {
        let mut outline = sample();
        assert_eq!(
            outline.plan_moves(&[9], None, Some(2)).unwrap(),
            [step(9, None, 2)]
        );
        assert_eq!(order(&outline, None), [1, 2, 9, 18]);
    }

    #[test]
    fn refuses_moves_that_would_put_a_todo_inside_itself_before_anything_is_sent() {
        for (ids, to, after, message) in [
            (
                &[1][..],
                Some(7),
                None,
                "cannot move 1 into its own subtree",
            ),
            (&[4], None, Some(8), "cannot move 4 into its own subtree"),
            (&[9], None, Some(9), "cannot move 9 after itself"),
            (&[18], Some(1), Some(7), "7 is not directly under 1"),
        ] {
            let error = sample().plan_moves(ids, to, after).unwrap_err();
            assert_eq!(error.to_string(), message);
        }
        assert!(matches!(
            sample().plan_moves(&[42], Some(1), None),
            Err(Error::NotFound { id: 42 })
        ));
    }

    #[test]
    fn indenting_makes_a_todo_the_last_child_of_the_one_above() {
        let mut outline = sample();
        assert_eq!(outline.indent(9).unwrap(), step(9, Some(8), 0));
        assert_eq!(order(&outline, Some(4)), [7, 8, 10]);
        assert_eq!(order(&outline, Some(8)), [9]);

        let mut outline = sample();
        assert_eq!(outline.indent(5).unwrap(), step(5, Some(4), 4));
        assert_eq!(order(&outline, Some(1)), [4]);
        assert_eq!(order(&outline, Some(4)), [7, 8, 9, 10, 5]);
    }

    #[test]
    fn cannot_indent_the_first_todo_because_there_is_nothing_above_it() {
        assert_eq!(
            sample().indent(7).unwrap_err().to_string(),
            "7 is the first todo under Home › Work › v2 release; there is nothing above to nest under"
        );
        assert_eq!(
            sample().indent(1).unwrap_err().to_string(),
            "1 is the first todo under Home; there is nothing above to nest under"
        );
    }

    #[test]
    fn outdenting_puts_a_todo_right_after_its_old_parent() {
        let mut outline = sample();
        assert_eq!(outline.outdent(9).unwrap(), step(9, Some(1), 1));
        assert_eq!(order(&outline, Some(1)), [4, 9, 5]);

        let mut outline = sample();
        assert_eq!(outline.outdent(4).unwrap(), step(4, None, 1));
        assert_eq!(order(&outline, None), [1, 4, 2, 18]);

        assert_eq!(
            sample().outdent(18).unwrap_err().to_string(),
            "18 is already at the top level"
        );
    }

    #[test]
    fn refuses_to_delete_a_todo_with_todos_under_it_unless_recursive() {
        assert_eq!(
            sample().plan_removals(&[1], false).unwrap_err().to_string(),
            "1 has 6 todos under it; pass -r to delete them too"
        );
        assert_eq!(
            sample().plan_removals(&[2], false).unwrap_err().to_string(),
            "2 has 1 todo under it; pass -r to delete it too"
        );
        assert_eq!(
            sample().plan_removals(&[1], true).unwrap(),
            [Removal { id: 1, under: 6 }]
        );
    }

    #[test]
    fn refuses_to_complete_a_todo_with_open_todos_under_it_unless_recursive_since_the_server_completes_them_too()
     {
        assert_eq!(
            sample()
                .check_completions(&[4], false)
                .unwrap_err()
                .to_string(),
            "4 has 4 open todos under it; pass -r to mark them done too"
        );
        assert_eq!(
            sample()
                .check_completions(&[18, 2], false)
                .unwrap_err()
                .to_string(),
            "2 has 1 open todo under it; pass -r to mark it done too"
        );
        assert!(sample().check_completions(&[4], true).is_ok());
        assert!(matches!(
            sample().check_completions(&[4, 42], true),
            Err(Error::NotFound { id: 42 })
        ));
    }

    #[test]
    fn completes_without_r_when_every_open_todo_under_it_is_named_too() {
        assert!(sample().check_completions(&[2, 11], false).is_ok());
        assert!(sample().check_completions(&[11, 18], false).is_ok());
    }

    #[test]
    fn marks_the_open_todos_under_a_completed_todo_done_as_the_server_does_so_the_progress_shown_is_right()
     {
        let mut outline = sample();
        let mut work = outline.get(4).unwrap().clone();
        work.completed_at = Some("2026-11-13 09:00:00".into());
        outline.update(work);

        outline.complete_under(4);

        assert!(
            [7, 8, 9, 10]
                .iter()
                .all(|&id| outline.get(id).unwrap().is_completed())
        );
        assert!(!outline.get(5).unwrap().is_completed());
    }

    #[test]
    fn checks_the_guard_before_any_delete_so_a_refusal_deletes_nothing() {
        assert!(sample().plan_removals(&[18, 2], false).is_err());
        assert!(matches!(
            sample().plan_removals(&[18, 42], true),
            Err(Error::NotFound { id: 42 })
        ));
    }

    #[test]
    fn stops_at_a_parent_loop_that_racing_moves_can_leave_on_the_server_instead_of_hanging() {
        let looped = || {
            Outline::new(vec![
                todo(1, None, 0, "Work"),
                todo(2, Some(3), 0, "a"),
                todo(3, Some(2), 0, "b"),
            ])
        };

        assert_eq!(
            looped().plan_removals(&[2], true).unwrap(),
            [Removal { id: 2, under: 1 }]
        );
        assert_eq!(
            looped().plan_moves(&[1], Some(2), None).unwrap(),
            [step(1, Some(2), 1)]
        );
        assert_eq!(looped().path(Some(2)), "Home › b › a");
    }

    #[test]
    fn does_not_delete_again_what_an_earlier_recursive_delete_already_took() {
        assert_eq!(
            sample().plan_removals(&[4, 7], true).unwrap(),
            [Removal { id: 4, under: 4 }]
        );
        assert_eq!(
            sample().plan_removals(&[11, 2], false).unwrap(),
            [Removal { id: 11, under: 0 }, Removal { id: 2, under: 0 }],
            "deleting the children first leaves nothing under the parent"
        );
    }
}
