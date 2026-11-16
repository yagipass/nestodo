use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::outline::{Key, Op, ROOT};
use crate::thread::Comment;
use crate::wire::{Reply, Request};

pub const TYPING_DELAY: Duration = Duration::from_millis(500);

#[derive(Clone, Debug)]
struct Typed {
    op: Op,
    due: Instant,
}

#[derive(Clone, Debug, PartialEq)]
enum Job {
    Op(Op),
    Comment { ticket: u64, key: Key, body: String },
    EditComment { id: u64, body: String },
    DeleteComment { id: u64 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Saved {
    Posted { ticket: u64, comment: Comment },
    Edited(Comment),
}

#[derive(Deserialize)]
struct One {
    comment: Comment,
}

#[derive(Clone, Debug, Default)]
pub struct Sync {
    ids: HashMap<Key, u64>,
    keys: HashMap<u64, Key>,
    queue: VecDeque<Job>,
    typed: Vec<Typed>,
    running: bool,
    tickets: u64,
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

    #[must_use]
    pub fn id_of(&self, key: &str) -> Option<u64> {
        self.ids.get(key).copied()
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
                        self.discard(key);
                    }
                }
                Op::Title { key, .. } => self.discard(key),
                _ => {}
            }
            self.flush();
            self.queue.push_back(Job::Op(op.clone()));
        }
    }

    pub fn title(&mut self, key: &str, title: &str, now: Instant) {
        self.revision += 1;
        self.discard(key);
        self.typed.push(Typed {
            op: Op::Title {
                key: key.to_owned(),
                title: title.to_owned(),
            },
            due: now + TYPING_DELAY,
        });
    }

    pub fn comment(&mut self, key: &str, body: &str) -> u64 {
        self.tickets += 1;
        let ticket = self.tickets;
        self.enqueue(Job::Comment {
            ticket,
            key: key.to_owned(),
            body: body.to_owned(),
        });
        ticket
    }

    pub fn edit_comment(&mut self, id: u64, body: &str) {
        self.enqueue(Job::EditComment {
            id,
            body: body.to_owned(),
        });
    }

    pub fn delete_comment(&mut self, id: u64) {
        self.enqueue(Job::DeleteComment { id });
    }

    fn enqueue(&mut self, job: Job) {
        self.revision += 1;
        self.flush();
        self.queue.push_back(job);
    }

    pub fn flush(&mut self) {
        self.queue
            .extend(self.typed.drain(..).map(|typed| Job::Op(typed.op)));
    }

    pub fn flush_due(&mut self, now: Instant) {
        let (mut due, waiting): (Vec<Typed>, Vec<Typed>) = std::mem::take(&mut self.typed)
            .into_iter()
            .partition(|typed| typed.due <= now);
        due.sort_by_key(|typed| typed.due);
        self.typed = waiting;
        self.queue
            .extend(due.into_iter().map(|typed| Job::Op(typed.op)));
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

    pub fn finish(&mut self, reply: Result<Reply, ()>) -> Option<Saved> {
        if !std::mem::take(&mut self.running) {
            return None;
        }
        let job = self.queue.front()?;
        let gone_is_done = matches!(job, Job::Op(Op::Delete { .. }) | Job::DeleteComment { .. });
        let reply = reply
            .ok()
            .filter(|reply| reply.is_success() || (gone_is_done && reply.status == 404));
        let Some(reply) = reply else {
            self.failed = true;
            return None;
        };
        let saved = match job {
            Job::Op(Op::Create { key, .. }) => {
                let id = reply
                    .body
                    .as_ref()
                    .and_then(|body| body.get("todo")?.get("id")?.as_u64());
                let Some(id) = id else {
                    self.failed = true;
                    return None;
                };
                let key = key.clone();
                self.track([(key, id)]);
                None
            }
            Job::Comment { ticket, .. } => {
                let ticket = *ticket;
                let Some(comment) = Self::comment_in(reply) else {
                    self.failed = true;
                    return None;
                };
                Some(Saved::Posted { ticket, comment })
            }
            Job::EditComment { .. } => {
                let Some(comment) = Self::comment_in(reply) else {
                    self.failed = true;
                    return None;
                };
                Some(Saved::Edited(comment))
            }
            Job::Op(_) | Job::DeleteComment { .. } => None,
        };
        self.queue.pop_front();
        saved
    }

    fn comment_in(reply: Reply) -> Option<Comment> {
        serde_json::from_value::<One>(reply.body?)
            .ok()
            .map(|one| one.comment)
    }

    fn discard(&mut self, key: &str) {
        self.typed.retain(|typed| typed.op.key() != key);
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

    fn request(&self, job: &Job) -> Option<Request> {
        let (method, path, body) = match job {
            Job::Op(op) => return self.op_request(op),
            Job::Comment { key, body, .. } => (
                "POST",
                format!("{}/comments", self.todo_path(key)?),
                Some(json!({ "body": body })),
            ),
            Job::EditComment { id, body } => (
                "PATCH",
                format!("/api/v1/comments/{id}"),
                Some(json!({ "body": body })),
            ),
            Job::DeleteComment { id } => ("DELETE", format!("/api/v1/comments/{id}"), None),
        };
        Some(Request { method, path, body })
    }

    fn op_request(&self, op: &Op) -> Option<Request> {
        let (method, path, body) = match op {
            Op::Create {
                parent,
                index,
                title,
                ..
            } => {
                let mut body = Map::new();
                body.insert("title".to_owned(), json!(title));
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
            Op::Restore {
                key, parent, index, ..
            } => (
                "POST",
                format!("{}/restore", self.todo_path(key)?),
                Some(json!({ "parentId": self.parent_id(parent)?, "index": index })),
            ),
            Op::Title { key, title } => (
                "PATCH",
                self.todo_path(key)?,
                Some(json!({ "title": title })),
            ),
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
            let commenting = request.path.ends_with("/comments")
                || request.path.starts_with("/api/v1/comments/");
            let payload = if commenting {
                self.next_id += 1;
                json!({ "comment": {
                    "id": self.next_id - 1,
                    "body": request.body.as_ref().and_then(|body| body.get("body")).cloned(),
                    "via": "app",
                    "at": "2026-11-16T01:00:00.000Z",
                    "editedAt": null
                } })
            } else if request.method == "POST" {
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
        saved: Vec<Saved>,
    }

    impl Harness {
        fn new(ids: &[(&str, u64)], server: Server) -> Self {
            Harness {
                sync: Sync::new(ids.iter().map(|&(key, id)| (key.to_owned(), id))),
                server,
                t0: Instant::now(),
                elapsed: Duration::ZERO,
                notified: 0,
                saved: Vec::new(),
            }
        }

        fn pump(&mut self) {
            let failed = self.sync.failed;
            while let Some(request) = self.sync.start() {
                let reply = self.server.reply(request);
                self.saved.extend(self.sync.finish(Ok(reply)));
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
    fn asks_the_server_to_bring_a_deleted_row_back_under_its_old_id_so_undoing_a_delete_keeps_the_comments_written_on_it()
     {
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], Server::new(|_| 200));
        let outline = crate::outline::fixtures::outline_of(&[
            crate::outline::fixtures::todo(1, "health", None),
            crate::outline::fixtures::todo(2, "dentist", Some(1)),
        ]);

        h.apply(&[delete_op("t2", &["t2"])]);
        h.apply(&[Op::Restore {
            key: "t2".into(),
            parent: "t1".into(),
            index: 0,
            items: vec![outline.item("t2").clone()],
        }]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [
                request("DELETE", "/api/v1/todos/2", None),
                request(
                    "POST",
                    "/api/v1/todos/2/restore",
                    Some(json!({ "parentId": 1, "index": 0 }))
                ),
            ]
        );
        assert!(!h.sync.failed);
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
    fn sends_a_comment_on_a_row_still_being_created_once_the_server_has_given_it_an_id_so_a_comment_written_right_after_enter_is_kept()
     {
        let mut h = Harness::new(&[], Server::ok());

        h.sync.apply(&[create_op("new1", ROOT, 0, "book hotel")]);
        let ticket = h.sync.comment("new1", "near the station");
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [
                request(
                    "POST",
                    "/api/v1/todos",
                    Some(json!({ "title": "book hotel", "parentId": null, "index": 0 }))
                ),
                request(
                    "POST",
                    "/api/v1/todos/100/comments",
                    Some(json!({ "body": "near the station" }))
                ),
            ]
        );
        assert!(matches!(
            h.saved.as_slice(),
            [Saved::Posted { ticket: posted, comment }]
                if *posted == ticket && comment.body == "near the station"
        ));
    }

    #[test]
    fn edits_and_deletes_comments_by_their_own_ids_in_the_order_they_were_made() {
        let server = Server::new(|call| if call.method == "DELETE" { 204 } else { 200 });
        let mut h = Harness::new(&[("t1", 1)], server);

        h.sync.edit_comment(7, "no answer, try Friday");
        h.sync.delete_comment(8);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [
                request(
                    "PATCH",
                    "/api/v1/comments/7",
                    Some(json!({ "body": "no answer, try Friday" }))
                ),
                request("DELETE", "/api/v1/comments/8", None),
            ]
        );
        assert!(matches!(
            h.saved.as_slice(),
            [Saved::Edited(comment)] if comment.body == "no answer, try Friday"
        ));
        assert!(!h.sync.pending());
    }

    #[test]
    fn stops_at_a_comment_the_server_refused_and_sends_nothing_after_it_so_editing_pauses_like_any_failed_save()
     {
        let server = Server::new(|call| {
            if call.path == "/api/v1/todos/1/comments" {
                500
            } else {
                200
            }
        });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.sync.comment("t1", "first");
        h.sync.comment("t2", "second");
        h.run_all_timers();

        assert_eq!(h.server.paths(), ["/api/v1/todos/1/comments"]);
        assert_eq!(h.saved, []);
        assert!(h.sync.failed);
        assert!(h.sync.pending());
        assert_eq!(h.notified, 1);
    }

    #[test]
    fn treats_deleting_a_comment_that_is_already_gone_as_done() {
        let mut h = Harness::new(&[], Server::new(|_| 404));

        h.sync.delete_comment(8);
        h.run_all_timers();

        assert!(!h.sync.failed);
        assert!(!h.sync.pending());
    }

    #[test]
    fn treats_a_comment_reply_without_the_saved_comment_as_a_failure_so_the_pane_never_shows_a_comment_the_server_did_not_confirm()
     {
        let mut sync = Sync::new([("t1".to_owned(), 1)]);

        sync.comment("t1", "near the station");
        assert!(sync.start().is_some());
        let saved = sync.finish(Ok(Reply {
            status: 201,
            body: Some(json!({})),
        }));

        assert_eq!(saved, None);
        assert!(sync.failed);
    }

    #[test]
    fn sends_typing_from_before_a_comment_first_keeping_the_users_order() {
        let mut h = Harness::new(&[("t1", 1)], Server::ok());

        h.title("t1", "book hotel");
        h.sync.comment("t1", "near the station");
        h.run_all_timers();

        assert_eq!(
            h.server.bodies(),
            [
                Some(json!({ "title": "book hotel" })),
                Some(json!({ "body": "near the station" }))
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
    fn drops_pending_titles_of_deleted_rows_instead_of_sending_them_to_todos_that_no_longer_exist()
    {
        let server = Server::new(|call| if call.method == "DELETE" { 204 } else { 200 });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.title("t1", "typed into the row");
        h.title("t2", "typed under the parent");
        h.apply(&[delete_op("t1", &["t1", "t2"])]);
        h.run_all_timers();

        assert_eq!(
            h.server.calls,
            [request("DELETE", "/api/v1/todos/1?recursive=true", None)]
        );
    }

    #[test]
    fn sends_typing_on_another_row_before_a_delete_so_a_refused_delete_cannot_hold_it_back() {
        let server = Server::new(|call| if call.method == "DELETE" { 500 } else { 200 });
        let mut h = Harness::new(&[("t1", 1), ("t2", 2)], server);

        h.title("t2", "typed just before");
        h.apply(&[delete_op("t1", &["t1"])]);
        h.run_all_timers();

        assert_eq!(h.server.paths(), ["/api/v1/todos/2", "/api/v1/todos/1"]);
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
