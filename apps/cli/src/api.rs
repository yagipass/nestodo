use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use ureq::http::{Method, StatusCode};
use url::Url;

use crate::access::Session;
use crate::error::Error;
use crate::http::{Call, Http, Response};
use crate::todo::Todo;

const LOGIN_PATH: &str = "/cdn-cgi/access/login";

pub struct Api<'a> {
    http: &'a Http,
    base: &'a Url,
    session: Option<Session<'a>>,
    now: i64,
}

#[derive(Deserialize)]
struct One {
    todo: Todo,
}

#[derive(Deserialize)]
struct Many {
    todos: Vec<Todo>,
}

impl<'a> Api<'a> {
    pub fn new(http: &'a Http, base: &'a Url, session: Option<Session<'a>>, now: i64) -> Self {
        Self {
            http,
            base,
            session,
            now,
        }
    }

    pub fn list(&mut self) -> Result<Vec<Todo>, Error> {
        let response = self.call(Method::GET, "/api/v1/todos", None, None)?;
        Ok(parse::<Many>(&response)?.todos)
    }

    pub fn create(
        &mut self,
        title: &str,
        parent: Option<u64>,
        index: Option<usize>,
    ) -> Result<Todo, Error> {
        let mut body = json!({ "title": title });
        if let Some(parent) = parent {
            body["parentId"] = json!(parent);
        }
        if let Some(index) = index {
            body["index"] = json!(index);
        }
        let response = self.call(Method::POST, "/api/v1/todos", Some(body), None)?;
        Ok(parse::<One>(&response)?.todo)
    }

    pub fn edit(&mut self, id: u64, title: &str) -> Result<Todo, Error> {
        self.patch(id, json!({ "title": title }))
    }

    pub fn move_to(&mut self, id: u64, parent: Option<u64>, index: usize) -> Result<Todo, Error> {
        self.patch(id, json!({ "parentId": parent, "index": index }))
    }

    pub fn set_completed(&mut self, id: u64, completed: bool) -> Result<Todo, Error> {
        self.patch(id, json!({ "completed": completed }))
    }

    pub fn delete(&mut self, id: u64, recursive: bool) -> Result<(), Error> {
        let path = if recursive {
            format!("{}?recursive=true", item(id))
        } else {
            item(id)
        };
        self.call(Method::DELETE, &path, None, Some(id)).map(drop)
    }

    fn patch(&mut self, id: u64, body: Value) -> Result<Todo, Error> {
        let response = self.call(Method::PATCH, &item(id), Some(body), Some(id))?;
        Ok(parse::<One>(&response)?.todo)
    }

    fn call(
        &mut self,
        method: Method,
        path: &str,
        body: Option<Value>,
        id: Option<u64>,
    ) -> Result<Response, Error> {
        let url = self.base.join(path).expect("API paths are valid");
        let mut response = self.send(&method, &url, body.as_ref())?;
        if needs_sign_in(&response, &url) {
            let Some(session) = self.session.as_mut() else {
                return Err(Error::Unauthorized {
                    status: response.status.as_u16(),
                });
            };
            session.refresh(self.http, self.base, self.now)?;
            response = self.send(&method, &url, body.as_ref())?;
            if needs_sign_in(&response, &url) {
                return Err(Error::NotSignedIn);
            }
        }
        match (response.status, id) {
            (status, _) if status.is_success() => Ok(response),
            (StatusCode::NOT_FOUND, Some(id)) => Err(Error::NotFound { id }),
            (StatusCode::BAD_REQUEST, _) => Err(Error::Invalid(
                message(&response).unwrap_or_else(|| "the server rejected the input".into()),
            )),
            (status, _) => Err(Error::Server {
                status: status.as_u16(),
                message: message(&response)
                    .unwrap_or_else(|| status.canonical_reason().unwrap_or("").to_string()),
            }),
        }
    }

    fn send(&self, method: &Method, url: &Url, body: Option<&Value>) -> Result<Response, Error> {
        let mut call = Call::new(method.clone(), url).header("accept", "application/json");
        if let Some(token) = self
            .session
            .as_ref()
            .and_then(|session| session.token_for(url))
        {
            call = call.header("cf-access-token", token);
        }
        if let Some(body) = body {
            call = call.json(body);
        }
        self.http.send(call)
    }
}

fn item(id: u64) -> String {
    format!("/api/v1/todos/{id}")
}

fn needs_sign_in(response: &Response, url: &Url) -> bool {
    matches!(
        response.status,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) || response
        .location(url)
        .is_some_and(|location| location.path().starts_with(LOGIN_PATH))
}

fn parse<T: DeserializeOwned>(response: &Response) -> Result<T, Error> {
    serde_json::from_slice(&response.body).map_err(|_| Error::Server {
        status: response.status.as_u16(),
        message: "the response is not the expected JSON".into(),
    })
}

fn message(response: &Response) -> Option<String> {
    let body: Value = serde_json::from_slice(&response.body).ok()?;
    if let Some(message) = body.get("message").and_then(Value::as_str) {
        return Some(message.to_string());
    }
    let issues: Vec<&str> = body
        .get("issues")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|slot| {
            slot.get("issues")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
        .filter_map(|issue| issue.get("message").and_then(Value::as_str))
        .collect();
    if !issues.is_empty() {
        return Some(issues.join("; "));
    }
    body.get("error")
        .and_then(Value::as_str)
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard};

    use super::*;
    use crate::access::{AccessApp, jwt};
    use crate::store::{Kind, MemoryStore, TokenStore};

    const NOW: i64 = 1_000;
    const LOGIN: &str =
        "https://family.cloudflareaccess.com/cdn-cgi/access/login/127.0.0.1?kid=app";

    fn token(name: &str, exp: i64) -> String {
        jwt::tests::token(&format!(
            r#"{{"exp":{exp},"aud":"app","sub":"{name}","iss":"https://family.cloudflareaccess.com"}}"#
        ))
    }

    fn app() -> AccessApp {
        AccessApp {
            hostname: "127.0.0.1".into(),
            aud: "app".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        }
    }

    fn list(server: &ServerGuard, store: &MemoryStore) -> Result<Vec<Todo>, Error> {
        let http = Http::new(false);
        let base = Url::parse(&server.url()).unwrap();
        let session = Session::open(&http, &base, app(), store, NOW)?;
        Api::new(&http, &base, Some(session), NOW).list()
    }

    #[test]
    fn sends_the_saved_app_token_to_the_api() {
        let mut server = Server::new();
        let store = MemoryStore::default();
        store
            .save("127.0.0.1", Kind::App, &token("saved", NOW + 60))
            .unwrap();
        let api = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", token("saved", NOW + 60).as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();

        assert_eq!(list(&server, &store).unwrap(), []);
        api.assert();
    }

    #[test]
    fn swaps_the_org_token_for_a_new_app_token_when_access_asks_for_a_login() {
        let mut server = Server::new();
        let store = MemoryStore::default();
        let (stale, fresh) = (token("stale", NOW + 60), token("fresh", NOW + 60));
        store.save("127.0.0.1", Kind::App, &stale).unwrap();
        store
            .save("127.0.0.1", Kind::Org, &token("org", NOW + 3600))
            .unwrap();
        server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", stale.as_str())
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        let exchange = server
            .mock("HEAD", "/")
            .with_header(
                "set-cookie",
                &format!("CF_Authorization={fresh}; Path=/; Secure; HttpOnly"),
            )
            .create();
        let retried = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", fresh.as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();

        assert_eq!(list(&server, &store).unwrap(), []);
        exchange.assert();
        retried.assert();
        assert_eq!(store.load("127.0.0.1", Kind::App).unwrap(), Some(fresh));
    }

    #[test]
    fn asks_for_todo_login_when_access_still_refuses_after_one_exchange() {
        let mut server = Server::new();
        let store = MemoryStore::default();
        store
            .save("127.0.0.1", Kind::App, &token("app", NOW + 60))
            .unwrap();
        store
            .save("127.0.0.1", Kind::Org, &token("org", NOW + 3600))
            .unwrap();
        let api = server
            .mock("GET", "/api/v1/todos")
            .with_status(401)
            .expect(2)
            .create();
        let exchange = server
            .mock("HEAD", "/")
            .with_header(
                "set-cookie",
                &format!("CF_Authorization={}", token("new", NOW + 60)),
            )
            .expect(1)
            .create();

        assert!(matches!(list(&server, &store), Err(Error::NotSignedIn)));
        api.assert();
        exchange.assert();
    }

    #[test]
    fn does_not_exchange_again_when_the_token_it_just_exchanged_is_refused() {
        let mut server = Server::new();
        let store = MemoryStore::default();
        store
            .save("127.0.0.1", Kind::App, &token("expired", NOW - 1))
            .unwrap();
        store
            .save("127.0.0.1", Kind::Org, &token("org", NOW + 3600))
            .unwrap();
        let exchange = server
            .mock("HEAD", "/")
            .with_header(
                "set-cookie",
                &format!("CF_Authorization={}", token("new", NOW + 60)),
            )
            .expect(1)
            .create();
        server
            .mock("GET", "/api/v1/todos")
            .with_status(403)
            .create();

        assert!(matches!(list(&server, &store), Err(Error::NotSignedIn)));
        exchange.assert();
    }

    #[test]
    fn asks_for_todo_login_when_there_is_no_org_token_to_exchange() {
        let mut server = Server::new();
        let store = MemoryStore::default();
        store
            .save("127.0.0.1", Kind::App, &token("app", NOW + 60))
            .unwrap();
        let api = server
            .mock("GET", "/api/v1/todos")
            .with_status(302)
            .with_header("location", LOGIN)
            .expect(1)
            .create();

        assert!(matches!(list(&server, &store), Err(Error::NotSignedIn)));
        api.assert();
    }
}
