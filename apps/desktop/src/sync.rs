use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use crate::outline::{Key, Op, ROOT};
use crate::undo::Field;
use crate::wire::{Reply, Request};

pub const TYPING_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
struct Typed {
    field: Field,
    op: Op,
    due: Instant,
}

#[derive(Clone, Debug, Default)]
pub struct Sync {
    ids: HashMap<Key, u64>,
    keys: HashMap<u64, Key>,
    queue: VecDeque<Op>,
    typed: Vec<Typed>,
    running: bool,
    pub failed: bool,
    pub revision: u64,
}

impl Sync {
    pub fn new(ids: impl IntoIterator<Item = (Key, u64)>) -> Self {
        let mut sync = Sync::default();
        sync.track(ids);
        sync
    }

    #[must_use]
    pub fn pending(&self) -> bool {
        self.running || !self.queue.is_empty() || !self.typed.is_empty()
    }

    #[must_use]
    pub fn key_of(&self, id: u64) -> Option<Key> {
        self.keys.get(&id).cloned()
    }

    pub fn track(&mut self, ids: impl IntoIterator<Item = (Key, u64)>) {
        for (key, id) in ids {
            self.ids.insert(key.clone(), id);
            self.keys.insert(id, key);
        }
    }

    pub fn apply(&mut self, ops: &[Op]) {
        self.revision += 1;
        for op in ops {
            match op {
                Op::Delete { removed, .. } => {
                    for key in removed {
                        self.discard(Field::Title, key);
                        self.discard(Field::Note, key);
                    }
                }
                Op::Title { key, .. } => self.discard(Field::Title, key),
                Op::Note { key, .. } => self.discard(Field::Note, key),
                _ => self.flush(),
            }
            self.queue.push_back(op.clone());
        }
    }

    pub fn title(&mut self, key: &str, title: &str, now: Instant) {
        let op = Op::Title {
            key: key.to_owned(),
            title: title.to_owned(),
        };
        self.type_op(Field::Title, op, now);
    }

    pub fn note(&mut self, key: &str, note: &str, now: Instant) {
        let op = Op::Note {
            key: key.to_owned(),
            note: note.to_owned(),
        };
        self.type_op(Field::Note, op, now);
    }

    pub fn flush(&mut self) {
        self.queue
            .extend(self.typed.drain(..).map(|typed| typed.op));
    }

    pub fn flush_due(&mut self, now: Instant) {
        let (mut due, waiting): (Vec<Typed>, Vec<Typed>) = std::mem::take(&mut self.typed)
            .into_iter()
            .partition(|typed| typed.due <= now);
        due.sort_by_key(|typed| typed.due);
        self.typed = waiting;
        self.queue.extend(due.into_iter().map(|typed| typed.op));
    }

    #[must_use]
    pub fn next_due(&self) -> Option<Instant> {
        self.typed.iter().map(|typed| typed.due).min()
    }

    pub fn start(&mut self) -> Option<Request> {
        if self.running || self.failed {
            return None;
        }
        let request = self.request(self.queue.front()?);
        match request {
            Some(_) => self.running = true,
            None => self.failed = true,
        }
        request
    }

    pub fn finish(&mut self, reply: Result<Reply, ()>) {
        if !std::mem::take(&mut self.running) {
            return;
        }
        let Some(op) = self.queue.front() else {
            return;
        };
        let reply = reply.ok().filter(|reply| {
            reply.is_success() || (matches!(op, Op::Delete { .. }) && reply.status == 404)
        });
        let Some(reply) = reply else {
            self.failed = true;
            return;
        };
        if let Op::Create { key, .. } = op {
            let id = reply
                .body
                .as_ref()
                .and_then(|body| body.get("todo")?.get("id")?.as_u64());
            let Some(id) = id else {
                self.failed = true;
                return;
            };
            let key = key.clone();
            self.track([(key, id)]);
        }
        self.queue.pop_front();
    }

    fn type_op(&mut self, field: Field, op: Op, now: Instant) {
        self.revision += 1;
        self.discard(field, op.key());
        self.typed.push(Typed {
            field,
            op,
            due: now + TYPING_DELAY,
        });
    }

    fn take_typed(&mut self, field: Field, key: &str) -> Option<Op> {
        let at = self
            .typed
            .iter()
            .position(|typed| typed.field == field && typed.op.key() == key)?;
        Some(self.typed.remove(at).op)
    }

    fn discard(&mut self, field: Field, key: &str) {
        self.take_typed(field, key);
    }

    fn todo_path(&self, key: &str) -> Option<String> {
        Some(format!("/api/v1/todos/{}", self.ids.get(key)?))
    }

    fn parent_id(&self, key: &str) -> Option<Value> {
        if key == ROOT {
            return Some(Value::Null);
        }
        self.ids.get(key).map(|&id| Value::from(id))
    }

    fn request(&self, op: &Op) -> Option<Request> {
        let (method, path, body) = match op {
            Op::Create {
                parent,
                index,
                title,
                note,
                ..
            } => {
                let mut body = Map::new();
                body.insert("title".to_owned(), json!(title));
                if let Some(note) = note {
                    body.insert("note".to_owned(), json!(note));
                }
                body.insert("parentId".to_owned(), self.parent_id(parent)?);
                body.insert("index".to_owned(), json!(index));
                (
                    "POST",
                    "/api/v1/todos".to_owned(),
                    Some(Value::Object(body)),
                )
            }
            Op::Delete { key, removed } => {
                let path = self.todo_path(key)?;
                let path = if removed.len() > 1 {
                    format!("{path}?recursive=true")
                } else {
                    path
                };
                ("DELETE", path, None)
            }
            Op::Title { key, title } => (
                "PATCH",
                self.todo_path(key)?,
                Some(json!({ "title": title })),
            ),
            Op::Note { key, note } => {
                ("PATCH", self.todo_path(key)?, Some(json!({ "note": note })))
            }
            Op::Move { key, parent, index } => {
                let path = self.todo_path(key)?;
                let parent = self.parent_id(parent)?;
                (
                    "PATCH",
                    path,
                    Some(json!({ "parentId": parent, "index": index })),
                )
            }
            Op::Update {
                key,
                completed,
                collapsed,
            } => {
                let mut fields = Map::new();
                if let Some(completed) = completed {
                    fields.insert("completed".to_owned(), json!(completed));
                }
                if let Some(collapsed) = collapsed {
                    fields.insert("collapsed".to_owned(), json!(collapsed));
                }
                ("PATCH", self.todo_path(key)?, Some(Value::Object(fields)))
            }
        };
        Some(Request { method, path, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::outline::fixtures::{create_op, delete_op, move_op, title_op, update_op};

    struct Server {
        calls: Vec<Request>,
        next_id: u64,
        status: Box<dyn Fn(&Request) -> u16>,
    }

    impl Server {
        fn new(status: impl Fn(&Request) -> u16 + 'static) -> Self {
            Server {
                calls: Vec::new(),
                next_id: 100,
                status: Box::new(status),
            }
        }

        fn ok() -> Self {
            Server::new(|_| 200)
        }

        fn reply(&mut self, request: Request) -> Reply {
            let status = (self.status)(&request);
            let payload = if request.method == "POST" {
                self.next_id += 1;
                json!({ "todo": { "id": self.next_id - 1 } })
            } else {
                json!({})
            };
            self.calls.push(request);
            Reply {
                status,
                body: (status != 204).then_some(payload),
            }
        }

        fn paths(&self) -> Vec<&str> {
            self.calls.iter().map(|call| call.path.as_str()).collect()
        }

        fn bodies(&self) -> Vec<Option<Value>> {
            self.calls.iter().map(|call| call.body.clone()).collect()
        }
    }

    struct Harness {
        sync: Sync,
        server: Server,
        t0: Instant,
        elapsed: Duration,
        notified: usize,
    }

    impl Harness {
        fn new(ids: &[(&str, u64)], server: Server) -> Self {
            Harness {
                sync: Sync::new(ids.iter().map(|&(key, id)| (key.to_owned(), id))),
                server,
                t0: Instant::now(),
                elapsed: Duration::ZERO,
                notified: 0,
            }
        }

        fn pump(&mut self) {
            let failed = self.sync.failed;
            while let Some(request) = self.sync.start() {
                let reply = self.server.reply(request);
                self.sync.finish(Ok(reply));
            }
            if !failed && self.sync.failed {
                self.notified += 1;
            }
        }

        fn apply(&mut self, ops: &[Op]) {
            self.sync.apply(ops);
            self.pump();
        }

        fn title(&mut self, key: &str, title: &str) {
            self.sync.title(key, title, self.t0 + self.elapsed);
        }

        fn note(&mut self, key: &str, note: &str) {
            self.sync.note(key, note, self.t0 + self.elapsed);
        }

        fn fire(&mut self, due: Instant) {
            self.elapsed = due - self.t0;
            self.sync.flush_due(due);
            self.pump();
        }

        fn advance(&mut self, ms: u64) {
            let until = self.t0 + self.elapsed + Duration::from_millis(ms);
            while let Some(due) = self.sync.next_due().filter(|&due| due <= until) {
                self.fire(due);
            }
            self.elapsed = until - self.t0;
            self.pump();
        }

        fn run_all_timers(&mut self) {
            while let Some(due) = self.sync.next_due() {
                self.fire(due);
            }
            self.pump();
        }
    }

    fn request(method: &'static str, path: &str, body: Option<Value>) -> Request {
        Request {
            method,
            path: path.to_owned(),
            body,
        }
    }

    #[test]
    fn sends_later_edits_of_a_new_row_to_the_id_the_server_assigned_to_it() {
        let mut h = Harness::new(&[], Server::ok());

        h.apply(&[create_op("new1", ROOT, 0, "")]);
        h.apply(&[create_op("new2", "new1", 0, "child")]);
        h.apply(&[update_op("new1", Some(true), None)]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [
                request(
                    "POST",
                    "/api/v1/todos",
                    Some(json!({ "title": "", "parentId": null, "index": 0 }))
                ),
                request(
                    "POST",
                    "/api/v1/todos",
                    Some(json!({ "title": "child", "parentId": 100, "index": 0 }))
                ),
                request(
                    "PATCH",
                    "/api/v1/todos/100",
                    Some(json!({ "completed": true }))
                ),
            ]
        );
        assert_eq!(h.sync.key_of(101).as_deref(), Some("new2"));
    }

    #[test]
    fn brings_back_a_deleted_rows_note_in_the_request_that_recreates_it_so_undoing_a_delete_loses_nothing_written_under_it()
     {
        let mut h = Harness::new(&[], Server::ok());

        h.apply(&[Op::Create {
            key: "t1".into(),
            parent: ROOT.into(),
            index: 0,
            title: "dentist".into(),
            note: Some("weekday mornings".into()),
        }]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [request(
                "POST",
                "/api/v1/todos",
                Some(
                    json!({ "title": "dentist", "note": "weekday mornings", "parentId": null, "index": 0 })
                )
            )]
        );
    }

    #[test]
    fn sends_a_burst_of_typing_as_one_title_update_once_the_user_pauses() {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        for title in ["B", "Bu", "Buy", "Buy milk"] {
            h.title("t1", title);
            h.advance(200);
        }
        assert_eq!(h.server.calls, []);
        assert!(h.sync.pending());
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [request(
                "PATCH",
                "/api/v1/todos/1",
                Some(json!({ "title": "Buy milk" }))
            )]
        );
        assert!(!h.sync.pending());
    }

    #[test]
    fn saves_typing_half_a_second_after_the_last_keystroke_the_same_pause_as_the_android_app_so_other_devices_see_it_soon_without_a_request_per_keystroke()
     {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        h.title("t1", "Buy milk");
        h.advance(499);
        assert_eq!(h.server.calls, []);
        h.advance(1);

        assert_eq!(
            h.server.calls,
            [request(
                "PATCH",
                "/api/v1/todos/1",
                Some(json!({ "title": "Buy milk" }))
            )]
        );
    }

    #[test]
    fn sends_a_burst_of_note_typing_as_one_update_of_its_own_so_it_never_overwrites_the_title_being_typed_in_the_same_pause()
     {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        h.title("t1", "book hotel");
        for note in ["n", "ne", "near the station"] {
            h.note("t1", note);
            h.advance(200);
        }
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [
                request(
                    "PATCH",
                    "/api/v1/todos/1",
                    Some(json!({ "title": "book hotel" }))
                ),
                request(
                    "PATCH",
                    "/api/v1/todos/1",
                    Some(json!({ "note": "near the station" }))
                ),
            ]
        );
        assert!(!h.sync.pending());
    }

    #[test]
    fn sends_a_pending_note_before_a_structural_change_to_the_same_row_keeping_the_users_order() {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        h.note("t1", "near the station");
        h.apply(&[update_op("t1", Some(true), None)]);
        h.run_all_timers();

        assert_eq!(
            h.server.bodies(),
            [
                Some(json!({ "note": "near the station" })),
                Some(json!({ "completed": true }))
            ]
        );
    }

    #[test]
    fn sends_a_pending_title_before_a_structural_change_to_the_same_row_keeping_the_users_order() {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        h.title("t1", "renamed");
        h.apply(&[move_op("t1", ROOT, 3)]);
        h.run_all_timers();

        assert_eq!(
            h.server.bodies(),
            [
                Some(json!({ "title": "renamed" })),
                Some(json!({ "parentId": null, "index": 3 }))
            ]
        );
    }

    #[test]
    fn sends_the_merged_title_before_deleting_the_merged_row_so_a_failure_in_between_never_loses_the_text()
     {
        let server = Server::new(|call| if call.method == "PATCH" { 503 } else { 204 });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.title("t1", "buy ");
        h.apply(&[title_op("t1", "buy milk"), delete_op("t2", &["t2"])]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [request(
                "PATCH",
                "/api/v1/todos/1",
                Some(json!({ "title": "buy milk" }))
            )]
        );
        assert!(h.sync.failed);
    }

    #[test]
    fn drops_pending_titles_and_notes_of_deleted_rows_instead_of_sending_them_to_todos_that_no_longer_exist()
     {
        let server = Server::new(|call| if call.method == "DELETE" { 204 } else { 200 });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.title("t1", "typed into the row");
        h.note("t1", "typed under the title");
        h.title("t2", "typed under the parent");
        h.apply(&[delete_op("t1", &["t1", "t2"])]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [request("DELETE", "/api/v1/todos/1?recursive=true", None)]
        );
    }

    #[test]
    fn asks_for_the_subtree_only_when_the_row_had_children_here_so_a_row_that_looked_empty_never_takes_unseen_todos_with_it()
     {
        let mut h = Harness::new(&[("t1", 1), ("t2", 2), ("t3", 3)], Server::new(|_| 204));

        h.apply(&[delete_op("t1", &["t1", "t2"])]);
        h.apply(&[delete_op("t3", &["t3"])]);
        h.run_all_timers();

        assert_eq!(
            h.server.paths(),
            ["/api/v1/todos/1?recursive=true", "/api/v1/todos/3"]
        );
    }

    #[test]
    fn stops_after_a_failure_so_later_edits_are_not_applied_on_top_of_a_state_the_client_no_longer_matches()
     {
        let server = Server::new(|call| if call.path.ends_with("/1") { 500 } else { 200 });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.apply(&[update_op("t1", Some(true), None)]);
        h.run_all_timers();
        h.apply(&[update_op("t2", Some(true), None)]);
        h.run_all_timers();

        assert_eq!(h.server.paths(), ["/api/v1/todos/1"]);
        assert!(h.sync.failed);
        assert_eq!(h.notified, 1);
    }

    #[test]
    fn keeps_counting_a_change_that_failed_to_save_as_unsaved_so_closing_the_tab_still_warns_before_it_is_lost_even_when_the_failure_happened_out_of_sight()
     {
        let mut h = Harness::new(&[("t1", 1)], Server::new(|_| 500));

        h.apply(&[update_op("t1", Some(true), None)]);
        h.run_all_timers();

        assert!(h.sync.failed);
        assert!(h.sync.pending());
    }

    #[test]
    fn treats_deleting_a_todo_that_is_already_gone_as_done() {
        let mut h = Harness::new(&[("t1", 1)], Server::new(|_| 404));

        h.apply(&[delete_op("t1", &["t1"])]);
        h.run_all_timers();

        assert!(!h.sync.failed);
    }

    #[test]
    fn treats_a_delete_the_server_refuses_as_a_failure_so_editing_pauses_and_the_member_learns_the_delete_did_not_happen_instead_of_a_row_that_gained_todos_on_another_device_vanishing_unnoticed()
     {
        let server = Server::new(|call| if call.method == "DELETE" { 409 } else { 200 });
        let mut h = Harness::new(&[("t1", 1)], server);

        h.apply(&[delete_op("t1", &["t1"])]);
        h.run_all_timers();

        assert!(h.sync.failed);
        assert_eq!(h.notified, 1);
    }

    #[test]
    fn treats_a_change_to_a_todo_deleted_on_another_device_as_a_failure_so_editing_pauses_until_a_reload_shows_it_gone_instead_of_the_change_vanishing_unnoticed()
     {
        let mut h = Harness::new(&[("t1", 1)], Server::new(|_| 404));

        h.apply(&[update_op("t1", Some(true), None)]);
        h.run_all_timers();

        assert!(h.sync.failed);
        assert_eq!(h.notified, 1);
    }

    #[test]
    fn treats_a_network_error_as_a_failure_and_keeps_the_change_queued_so_it_still_counts_as_unsaved()
     {
        let mut sync = Sync::new([("t1".to_owned(), 1)]);

        sync.apply(&[update_op("t1", Some(true), None)]);
        assert!(sync.start().is_some());
        sync.finish(Err(()));

        assert!(sync.failed);
        assert!(sync.pending());
        assert_eq!(sync.start(), None);
    }

    #[test]
    fn treats_an_edit_to_a_row_without_a_server_id_as_a_failure_so_it_is_never_sent_to_another_todo()
     {
        let mut sync = Sync::new([]);

        sync.apply(&[update_op("t9", Some(true), None)]);

        assert_eq!(sync.start(), None);
        assert!(sync.failed);
        assert!(sync.pending());
    }

    #[test]
    fn sends_one_request_at_a_time_so_a_create_has_its_id_before_the_edits_that_need_it() {
        let mut sync = Sync::new([]);

        sync.apply(&[
            create_op("new1", ROOT, 0, ""),
            update_op("new1", Some(true), None),
        ]);
        assert!(sync.start().is_some());

        assert_eq!(sync.start(), None);
        assert!(!sync.failed);
    }
}
