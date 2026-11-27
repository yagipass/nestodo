use std::fmt::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde_json::{Value, json};

use nestodo_desktop::api::StatsResponse;
use nestodo_desktop::error::Error;
use nestodo_desktop::icon::{ImageIcon, image_id};
use nestodo_desktop::profile::Profile;
use nestodo_desktop::thread::{CommentImage, Timeline, TimelineItem};
use nestodo_desktop::webhook::{Analytics, WebhookEdit, Webhooks};
use nestodo_desktop::wire::{Reply, Request, Todo};

use crate::ui::backend::Backend;

#[derive(Clone, Debug)]
struct Row {
    id: u64,
    parent: Option<u64>,
    position: usize,
    title: String,
    done: bool,
    collapsed: bool,
    icon: Option<String>,
}

#[derive(Clone, Debug)]
struct StoredImage {
    id: String,
    name: String,
    bytes: Vec<u8>,
}

impl StoredImage {
    fn listed(&self) -> ImageIcon {
        ImageIcon {
            id: self.id.clone(),
            name: self.name.clone(),
            url: format!("/api/v1/icons/{}/image", self.id),
        }
    }
}

const MAX_IMAGES: usize = 50;

pub fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        width,
        height,
        image::Rgb([90, 60, 200]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    bytes.into_inner()
}

pub struct TempFile(pub PathBuf);

impl TempFile {
    pub fn with(name: &str, bytes: &[u8]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let folder = std::env::temp_dir().join(format!(
            "nestodo-desktop-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(name);
        std::fs::write(&path, bytes).unwrap();
        Self(path)
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        if let Some(folder) = self.0.parent() {
            let _ = std::fs::remove_dir_all(folder);
        }
    }
}

#[derive(Clone, Debug)]
struct Comment {
    id: u64,
    todo: u64,
    body: String,
    via: &'static str,
    edited: bool,
    pictures: Vec<String>,
}

#[derive(Clone, Debug)]
struct StoredPicture {
    id: String,
    bytes: Vec<u8>,
    posted: bool,
}

fn picture_url(id: &str) -> String {
    format!("/api/v1/comment-images/{id}")
}

#[derive(Default)]
struct State {
    rows: Vec<Row>,
    trash: Vec<(u64, Vec<Row>)>,
    comments: Vec<Comment>,
    next_id: u64,
    next_comment: u64,
    failing: Option<u16>,
    failing_creates: Option<u16>,
    failing_timelines: bool,
    signed_out: bool,
    sent: Vec<Request>,
    timelines: Vec<u64>,
    images: Vec<StoredImage>,
    next_image: u64,
    image_calls: Vec<String>,
    image_lists: usize,
    image_downloads: usize,
    failing_image_lists: usize,
    pictures: Vec<StoredPicture>,
    next_picture: u64,
    picture_downloads: usize,
    refusing_pictures: Option<String>,
}

impl State {
    fn add_comment(&mut self, todo: u64, body: &str, via: &'static str) -> Comment {
        self.next_comment += 1;
        let comment = Comment {
            id: self.next_comment,
            todo,
            body: body.to_string(),
            via,
            edited: false,
            pictures: Vec::new(),
        };
        self.comments.push(comment.clone());
        comment
    }

    fn store_picture(&mut self, bytes: Vec<u8>) -> String {
        self.next_picture += 1;
        let id = format!("pic{}", self.next_picture);
        self.pictures.push(StoredPicture {
            id: id.clone(),
            bytes,
            posted: false,
        });
        id
    }

    fn take_pictures(&mut self, ids: &[String]) -> bool {
        let unique = ids
            .iter()
            .enumerate()
            .all(|(index, id)| !ids[..index].contains(id));
        let free = ids.iter().all(|id| {
            self.pictures
                .iter()
                .any(|picture| &picture.id == id && !picture.posted)
        });
        if !unique || !free {
            return false;
        }
        for picture in &mut self.pictures {
            if ids.contains(&picture.id) {
                picture.posted = true;
            }
        }
        true
    }

    fn comments_of(&self, todo: u64) -> impl Iterator<Item = &Comment> {
        self.comments
            .iter()
            .filter(move |comment| comment.todo == todo)
    }
}

fn at_of(comment: &Comment) -> String {
    format!("2026-11-16T10:{:02}:00.000Z", comment.id % 60)
}

fn comment_json(comment: &Comment) -> Value {
    let images: Vec<Value> = comment
        .pictures
        .iter()
        .map(|id| json!({ "id": id, "url": picture_url(id) }))
        .collect();
    json!({
        "id": comment.id,
        "body": comment.body,
        "via": comment.via,
        "at": at_of(comment),
        "editedAt": comment.edited.then_some("2026-11-16T11:00:00.000Z"),
        "images": images,
    })
}

#[derive(Default)]
pub struct FakeServer {
    state: Mutex<State>,
}

pub struct Seed<'a> {
    pub title: &'a str,
    pub parent: Option<u64>,
    pub comments: Vec<(&'a str, &'static str, usize)>,
    pub done: bool,
    pub collapsed: bool,
    pub icon: Option<&'a str>,
}

pub fn seed(title: &str) -> Seed<'_> {
    Seed {
        title,
        parent: None,
        comments: Vec::new(),
        done: false,
        collapsed: false,
        icon: None,
    }
}

impl<'a> Seed<'a> {
    pub fn icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn under(mut self, parent: u64) -> Self {
        self.parent = Some(parent);
        self
    }

    pub fn comment(mut self, body: &'static str) -> Self {
        self.comments.push((body, "app", 0));
        self
    }

    pub fn comment_with_pictures(mut self, body: &'static str, pictures: usize) -> Self {
        self.comments.push((body, "app", pictures));
        self
    }

    pub fn comment_via_mcp(mut self, body: &'static str) -> Self {
        self.comments.push((body, "mcp", 0));
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
                    done: seed.done,
                    collapsed: seed.collapsed,
                    icon: seed.icon.map(str::to_string),
                });
                for (body, via, pictures) in seed.comments {
                    let comment = state.add_comment(id, body, via).id;
                    let ids: Vec<String> = (0..pictures)
                        .map(|_| state.store_picture(png(12, 8)))
                        .collect();
                    state.take_pictures(&ids);
                    if let Some(comment) =
                        state.comments.iter_mut().find(|known| known.id == comment)
                    {
                        comment.pictures = ids;
                    }
                }
            }
        }
        server
    }

    pub fn with_images(self, names: &[&str]) -> Self {
        for name in names {
            self.store_image(name, png(8, 8));
        }
        self
    }

    pub fn store_image(&self, name: &str, bytes: Vec<u8>) -> String {
        let mut state = self.state.lock().unwrap();
        state.next_image += 1;
        let id = format!("img{}", state.next_image);
        state.images.push(StoredImage {
            id: id.clone(),
            name: name.to_string(),
            bytes,
        });
        id
    }

    pub fn images(&self) -> Vec<(String, String)> {
        let state = self.state.lock().unwrap();
        state
            .images
            .iter()
            .map(|image| (image.id.clone(), image.name.clone()))
            .collect()
    }

    pub fn image_bytes(&self, id: &str) -> Option<Vec<u8>> {
        let state = self.state.lock().unwrap();
        state
            .images
            .iter()
            .find(|image| image.id == id)
            .map(|image| image.bytes.clone())
    }

    pub fn picture_bytes(&self, id: &str) -> Option<Vec<u8>> {
        let state = self.state.lock().unwrap();
        state
            .pictures
            .iter()
            .find(|picture| picture.id == id)
            .map(|picture| picture.bytes.clone())
    }

    pub fn pictures_uploaded(&self) -> usize {
        self.state.lock().unwrap().pictures.len()
    }

    pub fn picture_downloads(&self) -> usize {
        self.state.lock().unwrap().picture_downloads
    }

    pub fn refuse_pictures(&self, message: &str) {
        self.state.lock().unwrap().refusing_pictures = Some(message.to_string());
    }

    pub fn image_calls(&self) -> Vec<String> {
        self.state.lock().unwrap().image_calls.clone()
    }

    pub fn image_lists(&self) -> usize {
        self.state.lock().unwrap().image_lists
    }

    pub fn image_downloads(&self) -> usize {
        self.state.lock().unwrap().image_downloads
    }

    pub fn fail_image_lists(&self, times: usize) {
        self.state.lock().unwrap().failing_image_lists = times;
    }

    pub fn fail_with(&self, status: u16) {
        self.state.lock().unwrap().failing = Some(status);
    }

    pub fn fail_creates_with(&self, status: u16) {
        self.state.lock().unwrap().failing_creates = Some(status);
    }

    pub fn fail_timelines(&self, failing: bool) {
        self.state.lock().unwrap().failing_timelines = failing;
    }

    pub fn timelines(&self) -> Vec<u64> {
        self.state.lock().unwrap().timelines.clone()
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
        Self::walk(&state, None, 0, &mut out);
        out.join("\n")
    }

    fn walk(state: &State, parent: Option<u64>, depth: usize, out: &mut Vec<String>) {
        let rows = &state.rows;
        let mut children: Vec<&Row> = rows.iter().filter(|row| row.parent == parent).collect();
        children.sort_by_key(|row| (row.position, row.id));
        for row in children {
            let mut line = format!("{}{}", "  ".repeat(depth), row.title);
            if let Some(icon) = &row.icon {
                let _ = write!(line, " [icon: {icon}]");
            }
            if row.done {
                line.push_str(" [done]");
            }
            if row.collapsed {
                line.push_str(" [collapsed]");
            }
            for comment in state.comments_of(row.id) {
                let body = comment.body.replace('\n', "\\n");
                let pictures = comment.pictures.join(", ");
                let shown = match (body.is_empty(), pictures.is_empty()) {
                    (_, true) => body,
                    (true, false) => format!("({pictures})"),
                    (false, false) => format!("{body} ({pictures})"),
                };
                let _ = write!(line, " [comment: {shown}]");
            }
            out.push(line);
            Self::walk(state, Some(row.id), depth + 1, out);
        }
    }

    fn ordered(state: &State) -> Vec<Todo> {
        fn visit(state: &State, parent: Option<u64>, out: &mut Vec<Todo>) {
            let mut children: Vec<&Row> = state
                .rows
                .iter()
                .filter(|row| row.parent == parent)
                .collect();
            children.sort_by_key(|row| (row.position, row.id));
            for row in children {
                out.push(Todo {
                    id: row.id,
                    parent_id: row.parent,
                    position: row.position as u64,
                    title: row.title.clone(),
                    completed_at: row.done.then(|| "2026-11-09 00:00:00".to_string()),
                    collapsed: row.collapsed,
                    comment_count: state.comments_of(row.id).count() as u64,
                    icon: row.icon.clone(),
                });
                visit(state, Some(row.id), out);
            }
        }
        let mut out = Vec::new();
        visit(state, None, &mut out);
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
        let segments: Vec<&str> = path
            .strip_prefix("/api/v1/")
            .unwrap_or_default()
            .split('/')
            .collect();
        let reply = |status: u16, body: Value| Reply {
            status,
            body: Some(body),
        };
        let not_found = || reply(404, json!({ "error": "not_found" }));
        let id = segments.get(1).and_then(|id| id.parse::<u64>().ok());
        let unknown_image = body
            .get("icon")
            .and_then(Value::as_str)
            .and_then(image_id)
            .is_some_and(|image| !state.images.iter().any(|known| known.id == image));
        if unknown_image {
            return reply(
                400,
                json!({ "error": "invalid_icon", "message": "Icon not found" }),
            );
        }
        match (request.method, segments.as_slice(), id) {
            ("POST", ["todos"], _) => {
                state.next_id += 1;
                let id = state.next_id;
                let parent = body["parentId"].as_u64();
                state.rows.push(Row {
                    id,
                    parent,
                    position: usize::MAX,
                    title: body["title"].as_str().unwrap_or_default().to_string(),
                    done: false,
                    collapsed: false,
                    icon: body["icon"].as_str().map(str::to_string),
                });
                let index = body["index"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok());
                Self::place(&mut state.rows, id, parent, index);
                Self::reopen(&mut state.rows, parent);
                if let Some(comment) = body["comment"].as_str() {
                    state.add_comment(id, comment, "app");
                }
                reply(201, json!({ "todo": { "id": id } }))
            }
            ("PATCH", ["todos", _], Some(id)) => {
                let Some(row) = state.rows.iter_mut().find(|row| row.id == id) else {
                    return not_found();
                };
                if let Some(title) = body["title"].as_str() {
                    row.title = title.to_string();
                }
                if let Some(collapsed) = body["collapsed"].as_bool() {
                    row.collapsed = collapsed;
                }
                if let Some(icon) = body.get("icon") {
                    row.icon = icon.as_str().map(str::to_string);
                }
                let parent = row.parent;
                let completed = body["completed"].as_bool();
                if completed == Some(false) {
                    row.done = false;
                }
                let mut moved = false;
                if body.get("parentId").is_some() || body.get("index").is_some() {
                    let target = match body.get("parentId") {
                        Some(Value::Null) => None,
                        Some(value) => value.as_u64(),
                        None => parent,
                    };
                    moved = target != parent;
                    let index = body["index"]
                        .as_u64()
                        .and_then(|index| usize::try_from(index).ok());
                    Self::place(&mut state.rows, id, target, index);
                }
                if completed == Some(true) {
                    Self::complete(&mut state.rows, id);
                } else if completed == Some(false) || moved {
                    Self::reopen_if_open_below(&mut state.rows, id);
                }
                reply(200, json!({ "todo": { "id": id } }))
            }
            ("DELETE", ["todos", _], Some(id)) => {
                let Some(target) = state.rows.iter().find(|row| row.id == id).cloned() else {
                    return not_found();
                };
                if !recursive && state.rows.iter().any(|row| row.parent == Some(id)) {
                    return reply(409, json!({ "error": "has_children" }));
                }
                let gone = Self::subtree(&state.rows, id);
                let (trashed, kept): (Vec<Row>, Vec<Row>) = std::mem::take(&mut state.rows)
                    .into_iter()
                    .partition(|row| gone.contains(&row.id));
                state.rows = kept;
                state.trash.push((id, trashed));
                Self::reorder(&mut state.rows, target.parent);
                Reply {
                    status: 204,
                    body: None,
                }
            }
            ("POST", ["todos", _, "restore"], Some(id)) => {
                let Some(at) = state.trash.iter().position(|(batch, _)| *batch == id) else {
                    return not_found();
                };
                let parent = body["parentId"].as_u64();
                if parent.is_some_and(|parent| !state.rows.iter().any(|row| row.id == parent)) {
                    return reply(400, json!({ "error": "invalid_placement" }));
                }
                let (_, rows) = state.trash.remove(at);
                for mut row in rows {
                    if row.id == id {
                        row.position = usize::MAX;
                    }
                    state.rows.push(row);
                }
                let index = body["index"]
                    .as_u64()
                    .and_then(|index| usize::try_from(index).ok());
                Self::place(&mut state.rows, id, parent, index);
                Self::reopen_if_open_below(&mut state.rows, id);
                reply(200, json!({ "todo": { "id": id } }))
            }
            ("POST", ["todos", _, "comments"], Some(id)) => {
                if !state.rows.iter().any(|row| row.id == id) {
                    return not_found();
                }
                let text = body["body"].as_str().unwrap_or_default();
                let pictures: Vec<String> = body["images"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|id| id.as_str().map(str::to_string))
                    .collect();
                if text.trim().is_empty() && pictures.is_empty() {
                    return reply(400, json!({ "error": "invalid_body" }));
                }
                if !state.take_pictures(&pictures) {
                    return reply(
                        400,
                        json!({ "error": "invalid_images", "message": "Image not found" }),
                    );
                }
                let comment = state.add_comment(id, text, "app").id;
                let Some(comment) = state.comments.iter_mut().find(|known| known.id == comment)
                else {
                    return not_found();
                };
                comment.pictures = pictures;
                reply(201, json!({ "comment": comment_json(comment) }))
            }
            ("PATCH", ["comments", _], Some(id)) => {
                let Some(comment) = state.comments.iter_mut().find(|comment| comment.id == id)
                else {
                    return not_found();
                };
                let text = body["body"].as_str().unwrap_or_default();
                if text.trim().is_empty() && comment.pictures.is_empty() {
                    return reply(400, json!({ "error": "invalid_body" }));
                }
                if comment.body != text {
                    comment.body = text.to_string();
                    comment.edited = true;
                }
                reply(200, json!({ "comment": comment_json(comment) }))
            }
            ("DELETE", ["comments", _], Some(id)) => {
                let Some(gone) = state.comments.iter().position(|comment| comment.id == id) else {
                    return not_found();
                };
                let gone = state.comments.remove(gone);
                state
                    .pictures
                    .retain(|picture| !gone.pictures.contains(&picture.id));
                Reply {
                    status: 204,
                    body: None,
                }
            }
            _ => reply(400, json!({ "error": "unsupported" })),
        }
    }

    fn reopen_if_open_below(rows: &mut [Row], id: u64) {
        let open = Self::subtree(rows, id)
            .iter()
            .any(|id| rows.iter().any(|row| row.id == *id && !row.done));
        if open {
            Self::reopen(rows, Some(id));
        }
    }
}

impl Backend for FakeServer {
    fn list(&self) -> Result<Vec<Todo>, Error> {
        let state = self.state.lock().unwrap();
        if state.signed_out {
            return Err(Error::NotSignedIn);
        }
        Ok(Self::ordered(&state))
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

    fn timeline(&self, id: u64) -> Result<Vec<TimelineItem>, Error> {
        let mut state = self.state.lock().unwrap();
        if state.signed_out {
            return Err(Error::NotSignedIn);
        }
        state.timelines.push(id);
        let Some(row) = state.rows.iter().find(|row| row.id == id) else {
            return Err(Error::Server {
                status: 404,
                message: "Todo not found".into(),
            });
        };
        if state.failing_timelines {
            return Err(Error::Server {
                status: 500,
                message: "failing".into(),
            });
        }
        let mut items =
            vec![json!({ "type": "event", "kind": "created", "at": "2026-11-16T09:00:00Z" })];
        items.extend(state.comments_of(id).map(|comment| {
            let mut item = comment_json(comment);
            item["type"] = json!("comment");
            item
        }));
        if row.done {
            items.push(
                json!({ "type": "event", "kind": "completed", "at": "2026-11-16T12:00:00Z" }),
            );
        }
        let timeline: Timeline = serde_json::from_value(json!({ "items": items }))
            .map_err(|error| Error::Invalid(error.to_string()))?;
        Ok(timeline.items)
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

    fn icons(&self) -> Result<Vec<ImageIcon>, Error> {
        let mut state = self.state.lock().unwrap();
        if state.signed_out {
            return Err(Error::NotSignedIn);
        }
        state.image_lists += 1;
        if state.failing_image_lists > 0 {
            state.failing_image_lists -= 1;
            return Err(Error::Server {
                status: 500,
                message: "failing".into(),
            });
        }
        Ok(state.images.iter().map(StoredImage::listed).collect())
    }

    fn add_icon(&self, name: &str, image: &[u8]) -> Result<ImageIcon, Error> {
        let mut state = self.state.lock().unwrap();
        state.image_calls.push(format!("POST {name}"));
        if state.images.len() >= MAX_IMAGES {
            return Err(Error::Refused(vec![(
                "too_many_icons".into(),
                "You can add up to 50 images".into(),
            )]));
        }
        state.next_image += 1;
        let stored = StoredImage {
            id: format!("img{}", state.next_image),
            name: name.to_string(),
            bytes: image.to_vec(),
        };
        state.images.push(stored.clone());
        Ok(stored.listed())
    }

    fn rename_icon(&self, id: &str, name: &str) -> Result<ImageIcon, Error> {
        let mut state = self.state.lock().unwrap();
        state.image_calls.push(format!("PATCH {id} {name}"));
        let Some(image) = state.images.iter_mut().find(|image| image.id == id) else {
            return Err(Error::Refused(vec![(
                "not_found".into(),
                "Icon not found".into(),
            )]));
        };
        image.name = name.to_string();
        Ok(image.listed())
    }

    fn delete_icon(&self, id: &str) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();
        state.image_calls.push(format!("DELETE {id}"));
        state.images.retain(|image| image.id != id);
        let value = format!("image:{id}");
        let State { rows, trash, .. } = &mut *state;
        for row in rows
            .iter_mut()
            .chain(trash.iter_mut().flat_map(|(_, rows)| rows.iter_mut()))
        {
            if row.icon.as_deref() == Some(value.as_str()) {
                row.icon = None;
            }
        }
        Ok(())
    }

    fn add_comment_image(&self, image: &[u8]) -> Result<CommentImage, Error> {
        let mut state = self.state.lock().unwrap();
        if let Some(message) = state.refusing_pictures.clone() {
            return Err(Error::Refused(vec![("image".into(), message)]));
        }
        let id = state.store_picture(image.to_vec());
        Ok(CommentImage {
            url: picture_url(&id),
            id,
        })
    }

    fn comment_image(&self, url: &str) -> Result<Vec<u8>, Error> {
        let mut state = self.state.lock().unwrap();
        state.picture_downloads += 1;
        state
            .pictures
            .iter()
            .find(|picture| picture_url(&picture.id) == url)
            .map(|picture| picture.bytes.clone())
            .ok_or(Error::Server {
                status: 404,
                message: "Image not found".into(),
            })
    }

    fn icon_image(&self, url: &str) -> Result<Vec<u8>, Error> {
        let mut state = self.state.lock().unwrap();
        state.image_downloads += 1;
        state
            .images
            .iter()
            .find(|image| image.listed().url == url)
            .map(|image| image.bytes.clone())
            .ok_or(Error::Server {
                status: 404,
                message: "Icon not found".into(),
            })
    }
}
