use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ureq::http::{Method, StatusCode};
use url::Url;

use crate::access::{self, AccessApp, Session};
use crate::error::Error;
use crate::http::{Call, Http, Response, is_loopback};
use crate::stats::Day;
use crate::store::{Kind, TokenStore};
use crate::wire::{Reply, Request, Todo};

const LOGIN_PATH: &str = "/cdn-cgi/access/login";

pub struct Client {
    http: Http,
    base: Url,
    session: Option<Session>,
}

pub enum Connection {
    Ready(Box<Client>),
    SignedOut(AccessApp),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsResponse {
    pub time_zone: String,
    pub days: Vec<Day>,
}

#[derive(Deserialize)]
struct Many {
    todos: Vec<Todo>,
}

impl Client {
    pub fn connect(base: Url, store: Arc<dyn TokenStore>, now: i64) -> Result<Connection, Error> {
        let http = Http::new();
        match access::discover(&http, &base, now)? {
            None => Ok(Connection::Ready(Box::new(Client {
                http,
                base,
                session: None,
            }))),
            Some(app) => Self::resume(http, base, app, store, now),
        }
    }

    pub fn signed_in(
        base: Url,
        app: AccessApp,
        store: Arc<dyn TokenStore>,
        now: i64,
    ) -> Result<Client, Error> {
        let http = Http::new();
        let session = Session::open(&http, &base, app, store, now)?;
        Ok(Client {
            http,
            base,
            session: Some(session),
        })
    }

    fn resume(
        http: Http,
        base: Url,
        app: AccessApp,
        store: Arc<dyn TokenStore>,
        now: i64,
    ) -> Result<Connection, Error> {
        match Session::open(&http, &base, app.clone(), store, now) {
            Ok(session) => Ok(Connection::Ready(Box::new(Client {
                http,
                base,
                session: Some(session),
            }))),
            Err(Error::NotSignedIn) => Ok(Connection::SignedOut(app)),
            Err(error) => Err(error),
        }
    }

    pub fn email(&self) -> Option<String> {
        self.session.as_ref()?.email()
    }

    pub fn is_protected(&self) -> bool {
        self.session.is_some()
    }

    pub fn sign_out(&self, store: &dyn TokenStore) -> Result<(), Error> {
        let forget = || logout(store, self.base.host_str().unwrap_or_default());
        match &self.session {
            Some(session) => session.sign_out(forget),
            None => forget(),
        }
    }

    pub fn request(&self, request: &Request) -> Result<Reply, Error> {
        let method = Method::from_bytes(request.method.as_bytes())
            .map_err(|_| Error::Invalid(format!("{:?} is not an HTTP method", request.method)))?;
        let url = self.base.join(&request.path).map_err(|_| {
            Error::Invalid(format!("{:?} is not a path on the server", request.path))
        })?;
        let body = request.body.as_ref();
        let sent = self.token_for(&url);
        let mut response = self.send(&method, &url, body, sent.as_deref())?;
        if needs_sign_in(&response, &url) {
            let Some(session) = &self.session else {
                return Err(Error::Unauthorized {
                    status: response.status.as_u16(),
                });
            };
            session.refresh(&self.http, &self.base, sent.as_deref(), now())?;
            response = self.send(&method, &url, body, session.token_for(&url).as_deref())?;
            if needs_sign_in(&response, &url) {
                return Err(Error::NotSignedIn);
            }
        }
        Ok(Reply {
            status: response.status.as_u16(),
            body: serde_json::from_slice(&response.body).ok(),
        })
    }

    pub fn list(&self) -> Result<Vec<Todo>, Error> {
        Ok(self.get::<Many>("/api/v1/todos".into())?.todos)
    }

    pub fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error> {
        let time_zone: String =
            url::form_urlencoded::byte_serialize(time_zone.as_bytes()).collect();
        self.get(format!("/api/v1/stats?tz={time_zone}"))
    }

    fn get<T: DeserializeOwned>(&self, path: String) -> Result<T, Error> {
        let reply = self.request(&Request {
            method: "GET",
            path,
            body: None,
        })?;
        let status = reply.status;
        if !reply.is_success() {
            return Err(Error::Server {
                status,
                message: reply.body.as_ref().and_then(message).unwrap_or_else(|| {
                    StatusCode::from_u16(status)
                        .ok()
                        .and_then(|status| status.canonical_reason())
                        .unwrap_or_default()
                        .to_string()
                }),
            });
        }
        reply
            .body
            .and_then(|body| serde_json::from_value(body).ok())
            .ok_or_else(|| Error::Server {
                status,
                message: "the response is not the expected JSON".into(),
            })
    }

    fn token_for(&self, url: &Url) -> Option<String> {
        self.session.as_ref()?.token_for(url)
    }

    fn send(
        &self,
        method: &Method,
        url: &Url,
        body: Option<&Value>,
        token: Option<&str>,
    ) -> Result<Response, Error> {
        let mut call = Call::new(method.clone(), url).header("accept", "application/json");
        if let Some(token) = token {
            call = call.header("cf-access-token", token);
        }
        if let Some(body) = body {
            call = call.json(body);
        }
        self.http.send(call)
    }
}

pub fn logout(store: &dyn TokenStore, host: &str) -> Result<(), Error> {
    let app = store.delete(host, Kind::App);
    let org = store.delete(host, Kind::Org);
    app.and(org)
}

pub fn parse_base_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value).map_err(|error| format!("{value:?} is not a URL: {error}"))?;
    let origin_only = url.username().is_empty()
        && url.password().is_none()
        && url.path() == "/"
        && url.query().is_none()
        && url.fragment().is_none();
    if !origin_only || url.host_str().is_none() {
        return Err(format!(
            "{value:?} must be an origin such as https://nestodo.example.com"
        ));
    }
    match (url.scheme(), url.host_str()) {
        ("https", _) => Ok(url),
        ("http", Some(host)) if is_loopback(host) => Ok(url),
        ("http", _) => Err(format!(
            "{value:?} uses http://, which is only allowed for localhost"
        )),
        _ => Err(format!("{value:?} must start with https://")),
    }
}

#[must_use]
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

fn needs_sign_in(response: &Response, url: &Url) -> bool {
    matches!(
        response.status,
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) || response
        .location(url)
        .is_some_and(|location| location.path().starts_with(LOGIN_PATH))
}

fn message(body: &Value) -> Option<String> {
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
    use mockito::{Matcher, Server, ServerGuard};
    use serde_json::json;

    use super::*;
    use crate::access::jwt;
    use crate::store::MemoryStore;

    const LATER: i64 = 4_102_444_800;
    const EARLIER: i64 = 1_000;
    const LOGIN: &str =
        "https://family.cloudflareaccess.com/cdn-cgi/access/login/127.0.0.1?kid=app";

    fn token(name: &str, exp: i64) -> String {
        jwt::tests::token(&format!(
            r#"{{"exp":{exp},"aud":"app","sub":"{name}","email":"{name}@example.com","iss":"https://family.cloudflareaccess.com"}}"#
        ))
    }

    fn app() -> AccessApp {
        AccessApp {
            hostname: "127.0.0.1".into(),
            aud: "app".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        }
    }

    fn base(server: &ServerGuard) -> Url {
        Url::parse(&server.url()).unwrap()
    }

    fn signed_in(server: &ServerGuard, store: &Arc<MemoryStore>) -> Result<Client, Error> {
        Client::signed_in(base(server), app(), store.clone(), now())
    }

    fn store_with(tokens: &[(Kind, &str)]) -> Arc<MemoryStore> {
        let store = Arc::new(MemoryStore::default());
        for (kind, token) in tokens {
            store.save("127.0.0.1", *kind, token).unwrap();
        }
        store
    }

    fn get(path: &str) -> Request {
        Request {
            method: "GET",
            path: path.into(),
            body: None,
        }
    }

    fn exchange_giving(server: &mut ServerGuard, token: &str) -> mockito::Mock {
        server
            .mock("HEAD", "/")
            .with_header(
                "set-cookie",
                &format!("CF_Authorization={token}; Path=/; Secure; HttpOnly"),
            )
            .expect(1)
            .create()
    }

    #[test]
    fn sends_the_saved_app_token_to_the_api() {
        let mut server = Server::new();
        let saved = token("saved", LATER);
        let store = store_with(&[(Kind::App, &saved)]);
        let api = server
            .mock("GET", "/api/v1/todos")
            .match_header("accept", "application/json")
            .match_header("cf-access-token", saved.as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();

        assert_eq!(signed_in(&server, &store).unwrap().list().unwrap(), []);
        api.assert();
    }

    #[test]
    fn swaps_the_org_token_for_a_new_app_token_when_access_asks_for_a_login() {
        let mut server = Server::new();
        let (stale, fresh) = (token("stale", LATER), token("fresh", LATER));
        let store = store_with(&[(Kind::App, &stale), (Kind::Org, &token("org", LATER))]);
        server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", stale.as_str())
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        let exchange = exchange_giving(&mut server, &fresh);
        let retried = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", fresh.as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();

        assert_eq!(signed_in(&server, &store).unwrap().list().unwrap(), []);
        exchange.assert();
        retried.assert();
        assert_eq!(store.load("127.0.0.1", Kind::App).unwrap(), Some(fresh));
    }

    #[test]
    fn asks_to_sign_in_again_when_access_still_refuses_after_the_requests_one_exchange() {
        let mut server = Server::new();
        let store = store_with(&[
            (Kind::App, &token("app", LATER)),
            (Kind::Org, &token("org", LATER)),
        ]);
        let api = server
            .mock("GET", "/api/v1/todos")
            .with_status(401)
            .expect(2)
            .create();
        let exchange = exchange_giving(&mut server, &token("new", LATER));

        let client = signed_in(&server, &store).unwrap();
        assert!(matches!(client.list(), Err(Error::NotSignedIn)));
        api.assert();
        exchange.assert();
    }

    #[test]
    fn exchanges_again_on_a_later_request_because_the_app_stays_open_longer_than_an_app_token_lives()
     {
        let mut server = Server::new();
        let (first, second, third) = (
            token("first", LATER),
            token("second", LATER),
            token("third", LATER),
        );
        let store = store_with(&[(Kind::App, &first), (Kind::Org, &token("org", LATER))]);
        let refused_first = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", first.as_str())
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        let accepted_second = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", second.as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();
        let expired_second = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", second.as_str())
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        let accepted_third = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", third.as_str())
            .with_body(r#"{"todos":[]}"#)
            .create();
        let exchanges = [
            exchange_giving(&mut server, &second),
            exchange_giving(&mut server, &third),
        ];

        let client = signed_in(&server, &store).unwrap();
        assert_eq!(client.list().unwrap(), []);
        assert_eq!(client.list().unwrap(), []);

        for mock in [
            refused_first,
            accepted_second,
            expired_second,
            accepted_third,
        ] {
            mock.assert();
        }
        for exchange in exchanges {
            exchange.assert();
        }
        assert_eq!(store.load("127.0.0.1", Kind::App).unwrap(), Some(third));
    }

    #[test]
    fn stops_at_one_exchange_per_request_even_right_after_connecting_needed_one() {
        let mut server = Server::new();
        let store = store_with(&[
            (Kind::App, &token("expired", EARLIER)),
            (Kind::Org, &token("org", LATER)),
        ]);
        let exchange = server
            .mock("HEAD", "/")
            .with_header(
                "set-cookie",
                &format!("CF_Authorization={}", token("new", LATER)),
            )
            .expect(2)
            .create();
        let api = server
            .mock("GET", "/api/v1/todos")
            .with_status(403)
            .expect(2)
            .create();

        let client = signed_in(&server, &store).unwrap();
        assert!(matches!(client.list(), Err(Error::NotSignedIn)));
        exchange.assert();
        api.assert();
    }

    #[test]
    fn asks_to_sign_in_again_when_there_is_no_org_token_to_exchange() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let api = server
            .mock("GET", "/api/v1/todos")
            .with_status(302)
            .with_header("location", LOGIN)
            .expect(1)
            .create();

        let client = signed_in(&server, &store).unwrap();
        assert!(matches!(client.list(), Err(Error::NotSignedIn)));
        api.assert();
    }

    #[test]
    fn requests_refused_at_the_same_time_share_one_exchange_instead_of_each_renewing_the_login() {
        let mut server = Server::new();
        let (stale, fresh) = (token("stale", LATER), token("fresh", LATER));
        let store = store_with(&[(Kind::App, &stale), (Kind::Org, &token("org", LATER))]);
        server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", stale.as_str())
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        let exchange = exchange_giving(&mut server, &fresh);
        let accepted = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", fresh.as_str())
            .with_body(r#"{"todos":[]}"#)
            .expect(4)
            .create();

        let client = Arc::new(signed_in(&server, &store).unwrap());
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let client = Arc::clone(&client);
                std::thread::spawn(move || client.list())
            })
            .collect();
        for thread in threads {
            assert_eq!(thread.join().unwrap().unwrap(), []);
        }
        exchange.assert();
        accepted.assert();
    }

    fn slow_exchange_giving(
        server: &mut ServerGuard,
        stale: &str,
        fresh: &str,
        delay: std::time::Duration,
    ) -> std::sync::mpsc::Receiver<()> {
        server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", stale)
            .with_status(302)
            .with_header("location", LOGIN)
            .create();
        server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", fresh)
            .with_body(r#"{"todos":[]}"#)
            .create();
        let (started, renewing) = std::sync::mpsc::channel();
        let cookie = format!("CF_Authorization={fresh}; Path=/; Secure; HttpOnly");
        server
            .mock("HEAD", "/")
            .with_header_from_request("set-cookie", move |_| {
                started.send(()).ok();
                std::thread::sleep(delay);
                cookie.clone()
            })
            .create();
        renewing
    }

    #[test]
    fn reading_the_signed_in_email_does_not_wait_for_a_renewal_so_the_window_keeps_drawing_meanwhile()
     {
        let mut server = Server::new();
        let (stale, fresh) = (token("stale", LATER), token("fresh", LATER));
        let store = store_with(&[(Kind::App, &stale), (Kind::Org, &token("org", LATER))]);
        let renewing = slow_exchange_giving(
            &mut server,
            &stale,
            &fresh,
            std::time::Duration::from_secs(1),
        );
        let client = signed_in(&server, &store).unwrap();

        std::thread::scope(|scope| {
            scope.spawn(|| client.list());
            renewing.recv().unwrap();
            let started = std::time::Instant::now();
            assert_eq!(client.email().as_deref(), Some("stale@example.com"));
            assert!(started.elapsed() < std::time::Duration::from_millis(500));
        });
    }

    #[test]
    fn signing_out_during_a_renewal_waits_for_it_so_the_renewed_token_cannot_bring_the_login_back()
    {
        let mut server = Server::new();
        let (stale, fresh) = (token("stale", LATER), token("fresh", LATER));
        let store = store_with(&[(Kind::App, &stale), (Kind::Org, &token("org", LATER))]);
        let renewing = slow_exchange_giving(
            &mut server,
            &stale,
            &fresh,
            std::time::Duration::from_millis(300),
        );
        let client = signed_in(&server, &store).unwrap();

        std::thread::scope(|scope| {
            scope.spawn(|| client.list());
            renewing.recv().unwrap();
            client.sign_out(store.as_ref()).unwrap();
        });

        assert_eq!(store.load("127.0.0.1", Kind::App).unwrap(), None);
        assert_eq!(store.load("127.0.0.1", Kind::Org).unwrap(), None);
    }

    #[test]
    fn a_sign_out_the_keychain_refuses_keeps_the_window_signed_in_so_saves_can_still_renew_the_token()
     {
        let mut server = Server::new();
        let (stale, fresh) = (token("stale", LATER), token("fresh", LATER));
        let store = Arc::new(StuckToken(Kind::Org, MemoryStore::default()));
        store.save("127.0.0.1", Kind::App, &stale).unwrap();
        store
            .save("127.0.0.1", Kind::Org, &token("org", LATER))
            .unwrap();
        slow_exchange_giving(&mut server, &stale, &fresh, std::time::Duration::ZERO);
        let client = Client::signed_in(base(&server), app(), store.clone(), now()).unwrap();

        client.sign_out(store.as_ref()).unwrap_err();

        assert_eq!(client.list().unwrap(), []);
    }

    #[test]
    fn returns_error_statuses_as_replies_so_the_caller_decides_what_they_mean() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("DELETE", "/api/v1/todos/7")
            .with_status(404)
            .with_body(r#"{"message":"Todo not found"}"#)
            .create();
        server
            .mock("GET", "/api/v1/todos")
            .with_status(500)
            .with_body("upstream exploded")
            .create();
        let client = signed_in(&server, &store).unwrap();

        let missing = client
            .request(&Request {
                method: "DELETE",
                path: "/api/v1/todos/7".into(),
                body: None,
            })
            .unwrap();
        assert_eq!(
            missing,
            Reply {
                status: 404,
                body: Some(json!({"message": "Todo not found"}))
            }
        );
        assert_eq!(
            client.request(&get("/api/v1/todos")).unwrap(),
            Reply {
                status: 500,
                body: None
            }
        );
    }

    #[test]
    fn sends_the_method_path_and_json_body_the_caller_asked_for() {
        let mut server = Server::new();
        let saved = token("app", LATER);
        let store = store_with(&[(Kind::App, &saved)]);
        let patch = server
            .mock("PATCH", "/api/v1/todos/7")
            .match_header("accept", "application/json")
            .match_header("content-type", "application/json")
            .match_header("cf-access-token", saved.as_str())
            .match_body(Matcher::Json(json!({"completed": true})))
            .with_body(r#"{"todo":{"id":7}}"#)
            .create();

        let reply = signed_in(&server, &store)
            .unwrap()
            .request(&Request {
                method: "PATCH",
                path: "/api/v1/todos/7".into(),
                body: Some(json!({"completed": true})),
            })
            .unwrap();

        assert_eq!(
            reply,
            Reply {
                status: 200,
                body: Some(json!({"todo": {"id": 7}}))
            }
        );
        patch.assert();
    }

    #[test]
    fn reads_the_servers_camel_case_list_so_nesting_and_completion_are_not_lost() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("GET", "/api/v1/todos")
            .with_body(
                r#"{"todos":[
                    {"id":1,"parentId":null,"position":0,"title":"Groceries","note":"","completedAt":null,"collapsed":true,"createdAt":"2026-11-01T00:00:00.000Z"},
                    {"id":2,"parentId":1,"position":0,"title":"Milk","note":"2L","completedAt":"2026-11-02T09:00:00.000Z","collapsed":false,"createdAt":"2026-11-01T00:00:00.000Z"}
                ]}"#,
            )
            .create();
        let client = signed_in(&server, &store).unwrap();

        let todos = client.list().unwrap();

        assert_eq!(todos[0].parent_id, None);
        assert!(todos[0].collapsed);
        assert_eq!(todos[1].parent_id, Some(1));
        assert_eq!(todos[1].note, "2L");
        assert_eq!(
            todos[1].completed_at.as_deref(),
            Some("2026-11-02T09:00:00.000Z")
        );
    }

    #[test]
    fn list_reports_the_servers_own_message_when_it_fails() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("GET", "/api/v1/todos")
            .with_status(503)
            .with_body(r#"{"error":"unavailable","message":"The database is busy"}"#)
            .create();

        let result = signed_in(&server, &store).unwrap().list();

        assert!(
            matches!(&result, Err(Error::Server { status: 503, message }) if message == "The database is busy"),
            "{result:?}"
        );
    }

    #[test]
    fn stats_encodes_the_time_zone_so_a_plus_in_etc_names_is_not_read_as_a_space() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("GET", "/api/v1/stats")
            .match_query(Matcher::UrlEncoded("tz".into(), "Etc/GMT+5".into()))
            .with_body(
                r#"{"timeZone":"Etc/GMT+5","days":[{"date":"2026-11-09","added":2,"completed":1,"open":5}]}"#,
            )
            .create();

        let stats = signed_in(&server, &store)
            .unwrap()
            .stats("Etc/GMT+5")
            .unwrap();

        assert_eq!(
            stats,
            StatsResponse {
                time_zone: "Etc/GMT+5".into(),
                days: vec![Day {
                    date: "2026-11-09".into(),
                    added: 2,
                    completed: 1,
                    open: 5,
                }],
            }
        );
    }

    fn unprotected(server: &mut ServerGuard, store: Arc<MemoryStore>) -> Client {
        server
            .mock("HEAD", "/")
            .match_header("cf-access-metadata-request", "true")
            .create();
        match Client::connect(base(server), store, now()).unwrap() {
            Connection::Ready(client) => *client,
            Connection::SignedOut(app) => panic!("asked to sign in to {app:?}"),
        }
    }

    #[test]
    fn does_not_send_a_saved_token_to_a_server_that_does_not_use_access() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let client = unprotected(&mut server, store);
        let api = server
            .mock("GET", "/api/v1/todos")
            .match_header("cf-access-token", Matcher::Missing)
            .with_body(r#"{"todos":[]}"#)
            .create();

        assert!(!client.is_protected());
        assert_eq!(client.email(), None);
        assert_eq!(client.list().unwrap(), []);
        api.assert();
    }

    #[test]
    fn a_refusal_from_a_server_without_access_is_not_something_signing_in_can_fix() {
        let mut server = Server::new();
        let client = unprotected(&mut server, Arc::new(MemoryStore::default()));
        server
            .mock("GET", "/api/v1/todos")
            .with_status(401)
            .create();

        assert!(matches!(
            client.request(&get("/api/v1/todos")),
            Err(Error::Unauthorized { status: 401 })
        ));
    }

    #[test]
    fn reconnects_with_the_saved_login_without_opening_a_browser() {
        let store = store_with(&[(Kind::App, &token("kim", LATER))]);
        let base = Url::parse("http://127.0.0.1:9").unwrap();

        match Client::resume(Http::new(), base, app(), store, now()).unwrap() {
            Connection::Ready(client) => {
                assert!(client.is_protected());
                assert_eq!(client.email().as_deref(), Some("kim@example.com"));
            }
            Connection::SignedOut(_) => panic!("the saved login was not used"),
        }
    }

    #[test]
    fn asks_to_sign_in_to_the_discovered_app_when_no_saved_login_can_be_used() {
        let store = store_with(&[(Kind::App, &token("expired", EARLIER))]);
        let base = Url::parse("http://127.0.0.1:9").unwrap();

        match Client::resume(Http::new(), base, app(), store, now()).unwrap() {
            Connection::SignedOut(signed_out) => assert_eq!(signed_out, app()),
            Connection::Ready(_) => panic!("connected with an expired login"),
        }
    }

    #[test]
    fn accepts_https_and_local_http_origins() {
        for ok in [
            "https://nestodo.example.com",
            "https://nestodo.example.com/",
            "http://localhost:5173",
            "http://127.0.0.1:8787",
            "http://[::1]:3000",
        ] {
            assert!(parse_base_url(ok).is_ok(), "{ok}");
        }
    }

    #[test]
    fn refuses_plain_http_to_anything_but_localhost_so_tokens_never_travel_unencrypted() {
        for bad in [
            "http://nestodo.example.com",
            "http://192.168.1.10:5173",
            "http://localhost.evil.example",
        ] {
            assert!(parse_base_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn refuses_urls_that_are_not_plain_origins() {
        for bad in [
            "ftp://nestodo.example.com",
            "https://nestodo.example.com/api",
            "https://user@nestodo.example.com",
            "https://nestodo.example.com/?x=1",
            "nestodo.example.com",
        ] {
            assert!(parse_base_url(bad).is_err(), "{bad}");
        }
    }

    struct StuckToken(Kind, MemoryStore);

    impl TokenStore for StuckToken {
        fn load(&self, host: &str, kind: Kind) -> Result<Option<String>, Error> {
            self.1.load(host, kind)
        }

        fn save(&self, host: &str, kind: Kind, token: &str) -> Result<(), Error> {
            self.1.save(host, kind, token)
        }

        fn delete(&self, host: &str, kind: Kind) -> Result<(), Error> {
            if kind == self.0 {
                Err(Error::Storage("the keychain is locked".into()))
            } else {
                self.1.delete(host, kind)
            }
        }
    }

    #[test]
    fn sign_out_still_forgets_the_long_lived_org_token_when_the_app_token_cannot_be_deleted() {
        let store = StuckToken(Kind::App, MemoryStore::default());
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        let error = logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not access the saved login: the keychain is locked"
        );
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn sign_out_fails_while_the_long_lived_org_token_could_still_sign_the_next_launch_back_in() {
        let store = StuckToken(Kind::Org, MemoryStore::default());
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        let error = logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(
            error.to_string(),
            "could not access the saved login: the keychain is locked"
        );
    }

    #[test]
    fn sign_out_still_forgets_the_app_token_when_the_long_lived_org_token_cannot_be_deleted() {
        let store = StuckToken(Kind::Org, MemoryStore::default());
        store.save("nestodo.example.com", Kind::App, "app").unwrap();
        store.save("nestodo.example.com", Kind::Org, "org").unwrap();

        logout(&store, "nestodo.example.com").unwrap_err();

        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
    }
}
