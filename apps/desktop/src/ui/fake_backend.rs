use std::fmt::Write;
use std::sync::Mutex;

use serde_json::{Value, json};

use nestodo_desktop::api::StatsResponse;
use nestodo_desktop::error::Error;
use nestodo_desktop::profile::Profile;
use nestodo_desktop::webhook::{Analytics, WebhookEdit, Webhooks};
use nestodo_desktop::wire::{Reply, Request, Todo};

use crate::ui::backend::Backend;

#[derive(Clone, Debug)]
struct Row {
    id: u64,
    parent: Option<u64>,
    position: usize,
    title: String,
    note: String,
    done: bool,
    collapsed: bool,
}

#[derive(Default)]
struct State {
    rows: Vec<Row>,
    next_id: u64,
    failing: Option<u16>,
    failing_creates: Option<u16>,
    signed_out: bool,
    sent: Vec<Request>,
}

#[derive(Default)]
pub struct FakeServer {
    state: Mutex<State>,
}

pub struct Seed<'a> {
    pub title: &'a str,
    pub parent: Option<u64>,
    pub note: &'a str,
    pub done: bool,
    pub collapsed: bool,
}

pub fn seed(title: &str) -> Seed<'_> {
    Seed {
        title,
        parent: None,
        note: "",
        done: false,
        collapsed: false,
    }
}

impl Seed<'_> {
    pub fn under(mut self, parent: u64) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn note(mut self, note: &'static str) -> Self {
        self.note = note;
        self
    }

    pub fn done(mut self) -> Self {
        self.done = true;
        self
    }

    pub fn collapsed(mut self) -> Self {
        self.collapsed = true;
        self
    }
}

impl FakeServer {
    pub fn with(seeds: Vec<Seed>) -> Self {
        let server = Self::default();
        {
            let mut state = server.state.lock().unwrap();
            for seed in seeds {
                state.next_id += 1;
                let id = state.next_id;
                let position = state
                    .rows
                    .iter()
                    .filter(|row| row.parent == seed.parent)
                    .count();
                state.rows.push(Row {
                    id,
                    parent: seed.parent,
                    position,
                    title: seed.title.to_string(),
                    note: seed.note.to_string(),
                    done: seed.done,
                    collapsed: seed.collapsed,
                });
            }
        }
        server
    }

    pub fn fail_with(&self, status: u16) {
        self.state.lock().unwrap().failing = Some(status);
    }

    pub fn fail_creates_with(&self, status: u16) {
        self.state.lock().unwrap().failing_creates = Some(status);
    }

    pub fn expire_sign_in(&self) {
        self.state.lock().unwrap().signed_out = true;
    }

    pub fn sent(&self) -> Vec<Request> {
        self.state.lock().unwrap().sent.clone()
    }

    pub fn tree(&self) -> String {
        let state = self.state.lock().unwrap();
        let mut out = Vec::new();
        Self::walk(&state.rows, None, 0, &mut out);
        out.join("\n")
    }

    fn walk(rows: &[Row], parent: Option<u64>, depth: usize, out: &mut Vec<String>) {
        let mut children: Vec<&Row> = rows.iter().filter(|row| row.parent == parent).collect();
        children.sort_by_key(|row| (row.position, row.id));
        for row in children {
            let mut line = format!("{}{}", "  ".repeat(depth), row.title);
            if row.done {
                line.push_str(" [done]");
            }
            if row.collapsed {
                line.push_str(" [collapsed]");
            }
            if !row.note.is_empty() {
                let _ = write!(line, " [note: {}]", row.note.replace('\n', "\\n"));
            }
            out.push(line);
            Self::walk(rows, Some(row.id), depth + 1, out);
        }
    }

    fn ordered(rows: &[Row]) -> Vec<Todo> {
        fn visit(rows: &[Row], parent: Option<u64>, out: &mut Vec<Todo>) {
            let mut children: Vec<&Row> = rows.iter().filter(|row| row.parent == parent).collect();
            children.sort_by_key(|row| (row.position, row.id));
            for row in children {
                out.push(Todo {
                    id: row.id,
                    parent_id: row.parent,
                    position: row.position as u64,
                    title: row.title.clone(),
                    note: row.note.clone(),
                    completed_at: row.done.then(|| "2026-11-09 00:00:00".to_string()),
                    collapsed: row.collapsed,
                });
                visit(rows, Some(row.id), out);
            }
        }
        let mut out = Vec::new();
        visit(rows, None, &mut out);
        out
    }

    fn reorder(rows: &mut [Row], parent: Option<u64>) {
        let mut siblings: Vec<(usize, u64)> = rows
            .iter()
            .filter(|row| row.parent == parent)
            .map(|row| (row.position, row.id))
            .collect();
        siblings.sort_unstable();
        for (position, (_, id)) in siblings.into_iter().enumerate() {
            if let Some(row) = rows.iter_mut().find(|row| row.id == id) {
                row.position = position;
            }
        }
    }

    fn place(rows: &mut [Row], id: u64, parent: Option<u64>, index: Option<usize>) {
        let old_parent = rows
            .iter()
            .find(|row| row.id == id)
            .and_then(|row| row.parent);
        let mut siblings: Vec<(usize, u64)> = rows
            .iter()
            .filter(|row| row.parent == parent && row.id != id)
            .map(|row| (row.position, row.id))
            .collect();
        siblings.sort_unstable();
        let mut order: Vec<u64> = siblings.into_iter().map(|(_, id)| id).collect();
        let index = index.unwrap_or(order.len()).min(order.len());
        order.insert(index, id);
        if let Some(row) = rows.iter_mut().find(|row| row.id == id) {
            row.parent = parent;
        }
        for (position, sibling) in order.into_iter().enumerate() {
            if let Some(row) = rows.iter_mut().find(|row| row.id == sibling) {
                row.position = position;
            }
        }
        if old_parent != parent {
            Self::reorder(rows, old_parent);
        }
    }

    fn subtree(rows: &[Row], id: u64) -> Vec<u64> {
        let mut ids = vec![id];
        let mut index = 0;
        while index < ids.len() {
            let parent = ids[index];
            ids.extend(
                rows.iter()
                    .filter(|row| row.parent == Some(parent))
                    .map(|row| row.id),
            );
            index += 1;
        }
        ids
    }

    fn reopen(rows: &mut [Row], mut id: Option<u64>) {
        while let Some(row) = id.and_then(|id| rows.iter_mut().find(|row| row.id == id)) {
            row.done = false;
            id = row.parent;
        }
    }

    fn complete(rows: &mut [Row], id: u64) {
        let ids = Self::subtree(rows, id);
        for row in rows.iter_mut().filter(|row| ids.contains(&row.id)) {
            row.done = true;
        }
    }

    fn handle(state: &mut State, request: &Request) -> Reply {
        let body = request.body.clone().unwrap_or(Value::Null);
        let path = request.path.split('?').next().unwrap_or_default();
        let recursive = request.path.ends_with("?recursive=true");
        let id: Option<u64> = path
            .strip_prefix("/api/v1/todos/")
            .and_then(|id| id.parse().ok());
        let reply = |status: u16, body: Value| Reply {
            status,
            body: Some(body),
        };
        match (request.method, id) {
            ("POST", None) => {
                state.next_id += 1;
                let id = state.next_id;
                let parent = body["parentId"].as_u64();
                state.rows.push(Row {
                    id,
                    parent,
                    position: usize::MAX,
                    title: body["title"].as_str().unwrap_or_default().to_string(),
                    note: body["note"].as_str().unwrap_or_default().to_string(),
                    done: false,
                    collapsed: false,
                });
                let index = body["index"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok());
                Self::place(&mut state.rows, id, parent, index);
                Self::reopen(&mut state.rows, parent);
                reply(201, json!({ "todo": { "id": id } }))
            }
            ("PATCH", Some(id)) => {
                let Some(row) = state.rows.iter_mut().find(|row| row.id == id) else {
                    return reply(404, json!({ "error": "not_found" }));
                };
                if let Some(title) = body["title"].as_str() {
                    row.title = title.to_string();
                }
                if let Some(note) = body["note"].as_str() {
                    row.note = note.to_string();
                }
                if let Some(collapsed) = body["collapsed"].as_bool() {
                    row.collapsed = collapsed;
                }
                let parent = row.parent;
                match body["completed"].as_bool() {
                    Some(true) => Self::complete(&mut state.rows, id),
                    Some(false) => Self::reopen(&mut state.rows, Some(id)),
                    None => {}
                }
                if body.get("parentId").is_some() || body.get("index").is_some() {
                    let parent = match body.get("parentId") {
                        Some(Value::Null) => None,
                        Some(value) => value.as_u64(),
                        None => parent,
                    };
                    let index = body["index"]
                        .as_u64()
                        .and_then(|index| usize::try_from(index).ok());
                    Self::place(&mut state.rows, id, parent, index);
                    let open = Self::subtree(&state.rows, id)
                        .iter()
                        .any(|id| state.rows.iter().any(|row| row.id == *id && !row.done));
                    if open {
                        Self::reopen(&mut state.rows, parent);
                    }
                }
                reply(200, json!({ "todo": { "id": id } }))
            }
            ("DELETE", Some(id)) => {
                let Some(target) = state.rows.iter().find(|row| row.id == id).cloned() else {
                    return reply(404, json!({ "error": "not_found" }));
                };
                if !recursive && state.rows.iter().any(|row| row.parent == Some(id)) {
                    return reply(409, json!({ "error": "has_children" }));
                }
                let gone = Self::subtree(&state.rows, id);
                state.rows.retain(|row| !gone.contains(&row.id));
                Self::reorder(&mut state.rows, target.parent);
                Reply {
                    status: 204,
                    body: None,
                }
            }
            _ => reply(400, json!({ "error": "unsupported" })),
        }
    }
}

impl Backend for FakeServer {
    fn list(&self) -> Result<Vec<Todo>, Error> {
        let state = self.state.lock().unwrap();
        if state.signed_out {
            return Err(Error::NotSignedIn);
        }
        Ok(Self::ordered(&state.rows))
    }

    fn request(&self, request: &Request) -> Result<Reply, Error> {
        let mut state = self.state.lock().unwrap();
        if state.signed_out {
            return Err(Error::NotSignedIn);
        }
        state.sent.push(request.clone());
        let creating = request.method == "POST";
        if let Some(status) = state.failing.or(state.failing_creates.filter(|_| creating)) {
            return Ok(Reply {
                status,
                body: Some(json!({ "error": "failing" })),
            });
        }
        Ok(Self::handle(&mut state, request))
    }

    fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error> {
        Ok(StatsResponse {
            time_zone: time_zone.to_string(),
            days: Vec::new(),
        })
    }

    fn webhooks(&self) -> Result<Webhooks, Error> {
        Ok(Webhooks {
            available: false,
            webhook: None,
        })
    }

    fn save_webhook(&self, _: &WebhookEdit) -> Result<Webhooks, Error> {
        Err(Error::Invalid("this server cannot store webhooks".into()))
    }

    fn remove_webhook(&self) -> Result<(), Error> {
        Ok(())
    }

    fn analytics(&self) -> Result<Analytics, Error> {
        Ok(Analytics {
            available: false,
            enabled: false,
            token: None,
        })
    }

    fn set_analytics(&self, enabled: bool) -> Result<bool, Error> {
        Ok(enabled)
    }

    fn profile(&self) -> Result<Profile, Error> {
        Ok(Profile {
            email: "kim@example.com".into(),
            name: None,
            avatar_url: None,
        })
    }

    fn save_name(&self, _: &str) -> Result<Profile, Error> {
        Err(Error::Invalid("this server cannot store a profile".into()))
    }

    fn save_avatar(&self, _: &[u8]) -> Result<Profile, Error> {
        Err(Error::Invalid("this server cannot store a profile".into()))
    }

    fn remove_avatar(&self) -> Result<(), Error> {
        Ok(())
    }

    fn avatar(&self, _: &str) -> Result<Vec<u8>, Error> {
        Err(Error::Invalid("this server has no avatars".into()))
    }
}
