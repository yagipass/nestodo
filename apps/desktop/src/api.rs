use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use ureq::http::{Method, StatusCode};
use url::Url;

use crate::access::{self, AccessApp, Session};
use crate::error::Error;
use crate::http::{Call, Http, Response, is_loopback};
use crate::profile::Profile;
use crate::stats::Day;
use crate::store::{Kind, TokenStore};
use crate::webhook::{Analytics, WebhookEdit, Webhooks};
use crate::wire::{Reply, Request, Todo};

const LOGIN_PATH: &str = "/cdn-cgi/access/login";
const WEBHOOK_PATH: &str = "/api/v1/webhook";
const ANALYTICS_PATH: &str = "/api/v1/analytics";
const PROFILE_PATH: &str = "/api/v1/profile";
const AVATAR_PATH: &str = "/api/v1/profile/avatar";
const AVATARS_PATH: &str = "/api/v1/avatars/";

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

#[derive(Deserialize)]
struct Consent {
    enabled: bool,
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
        let response = self.respond(request)?;
        Ok(Reply {
            status: response.status.as_u16(),
            body: serde_json::from_slice(&response.body).ok(),
        })
    }

    fn respond(&self, request: &Request) -> Result<Response, Error> {
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
        Ok(response)
    }

    pub fn list(&self) -> Result<Vec<Todo>, Error> {
        Ok(self.get::<Many>("/api/v1/todos".into())?.todos)
    }

    pub fn stats(&self, time_zone: &str) -> Result<StatsResponse, Error> {
        let time_zone: String =
            url::form_urlencoded::byte_serialize(time_zone.as_bytes()).collect();
        self.get(format!("/api/v1/stats?tz={time_zone}"))
    }

    pub fn webhooks(&self) -> Result<Webhooks, Error> {
        self.get(WEBHOOK_PATH.into())
    }

    pub fn save_webhook(&self, edit: &WebhookEdit) -> Result<Webhooks, Error> {
        let body = serde_json::to_value(edit)
            .map_err(|error| Error::Invalid(format!("the webhook could not be sent: {error}")))?;
        self.put(WEBHOOK_PATH, body)
    }

    pub fn remove_webhook(&self) -> Result<(), Error> {
        self.delete(WEBHOOK_PATH)
    }

    pub fn profile(&self) -> Result<Profile, Error> {
        self.get(PROFILE_PATH.into())
    }

    pub fn save_name(&self, name: &str) -> Result<Profile, Error> {
        self.put(PROFILE_PATH, serde_json::json!({ "name": name }))
    }

    pub fn save_avatar(&self, image: &[u8]) -> Result<Profile, Error> {
        self.put(
            AVATAR_PATH,
            serde_json::json!({ "image": STANDARD.encode(image) }),
        )
    }

    pub fn remove_avatar(&self) -> Result<(), Error> {
        self.delete(AVATAR_PATH)
    }

    pub fn avatar(&self, url: &str) -> Result<Vec<u8>, Error> {
        let target = self.base.join(url).ok().filter(|target| {
            target.origin() == self.base.origin() && target.path().starts_with(AVATARS_PATH)
        });
        let Some(target) = target else {
            return Err(Error::Invalid(format!(
                "{url:?} is not an avatar on the server"
            )));
        };
        let response = self.respond(&Request {
            method: "GET",
            path: target.into(),
            body: None,
        })?;
        if response.status.is_success() {
            return Ok(response.body);
        }
        Err(failure(&Reply {
            status: response.status.as_u16(),
            body: serde_json::from_slice(&response.body).ok(),
        }))
    }

    pub fn analytics(&self) -> Result<Analytics, Error> {
        self.get(ANALYTICS_PATH.into())
    }

    pub fn set_analytics(&self, enabled: bool) -> Result<bool, Error> {
        let reply = self.request(&Request {
            method: "PUT",
            path: ANALYTICS_PATH.into(),
            body: Some(serde_json::json!({ "enabled": enabled })),
        })?;
        Ok(decoded::<Consent>(reply)?.enabled)
    }

    fn get<T: DeserializeOwned>(&self, path: String) -> Result<T, Error> {
        decoded(self.request(&Request {
            method: "GET",
            path,
            body: None,
        })?)
    }

    fn put<T: DeserializeOwned>(&self, path: &str, body: Value) -> Result<T, Error> {
        let reply = self.request(&Request {
            method: "PUT",
            path: path.into(),
            body: Some(body),
        })?;
        if reply.status == StatusCode::BAD_REQUEST.as_u16()
            && let Some(fields) = reply.body.as_ref().map(refused_fields)
            && !fields.is_empty()
        {
            return Err(Error::Refused(fields));
        }
        decoded(reply)
    }

    fn delete(&self, path: &str) -> Result<(), Error> {
        let reply = self.request(&Request {
            method: "DELETE",
            path: path.into(),
            body: None,
        })?;
        if reply.is_success() {
            Ok(())
        } else {
            Err(failure(&reply))
        }
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

fn failure(reply: &Reply) -> Error {
    let status = reply.status;
    Error::Server {
        status,
        message: reply.body.as_ref().and_then(message).unwrap_or_else(|| {
            StatusCode::from_u16(status)
                .ok()
                .and_then(|status| status.canonical_reason())
                .unwrap_or_default()
                .to_string()
        }),
    }
}

fn decoded<T: DeserializeOwned>(reply: Reply) -> Result<T, Error> {
    if !reply.is_success() {
        return Err(failure(&reply));
    }
    let status = reply.status;
    reply
        .body
        .and_then(|body| serde_json::from_value(body).ok())
        .ok_or_else(|| Error::Server {
            status,
            message: "the response is not the expected JSON".into(),
        })
}

fn issues(body: &Value) -> impl Iterator<Item = &Value> {
    body.get("issues")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|slot| {
            slot.get("issues")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
        })
}

fn refused_fields(body: &Value) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = Vec::new();
    for issue in issues(body) {
        let (Some(path), Some(message)) = (
            issue.get("path").and_then(Value::as_str),
            issue.get("message").and_then(Value::as_str),
        ) else {
            continue;
        };
        let field = path.split('.').next().unwrap_or_default();
        if !fields.iter().any(|(known, _)| known == field) {
            fields.push((field.to_string(), message.to_string()));
        }
    }
    fields
}

pub fn logout(store: &dyn TokenStore, host: &str) -> Result<(), Error> {
    let app = store.delete(host, Kind::App);
    let org = store.delete(host, Kind::Org);
    app.and(org)
}

pub fn base_url_of(input: &str) -> Result<Url, String> {
    let address = input.trim();
    let url = if has_scheme(address) {
        Url::parse(address)
    } else {
        Url::parse(&format!("https://{address}"))
    }
    .ok()
    .filter(|url| matches!(url.scheme(), "http" | "https") && url.has_host())
    .and_then(|url| Url::parse(&url.origin().ascii_serialization()).ok())
    .ok_or_else(|| "Enter an address such as nestodo.example.com.".to_string())?;
    match (url.scheme(), url.host_str()) {
        ("https", _) => Ok(url),
        ("http", Some(host)) if is_loopback(host) => Ok(url),
        _ => Err("Enter an address that starts with https://.".to_string()),
    }
}

fn has_scheme(address: &str) -> bool {
    address.split_once("://").is_some_and(|(scheme, _)| {
        let mut chars = scheme.chars();
        chars
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic())
            && chars.all(|char| char.is_ascii_alphanumeric() || matches!(char, '+' | '.' | '-'))
    })
}

#[must_use]
pub fn label(base: &Url) -> String {
    let shown = base.as_str().trim_end_matches('/');
    shown.strip_prefix("https://").unwrap_or(shown).to_string()
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
    let issues: Vec<&str> = issues(body)
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
    fn an_address_copied_from_the_browser_becomes_the_servers_root_so_the_api_and_the_sign_in_go_to_the_server_and_not_to_that_page()
     {
        for (typed, base) in [
            ("nestodo.example.com", "https://nestodo.example.com/"),
            (
                " Nestodo.example.com/today?from=https://example.com/ ",
                "https://nestodo.example.com/",
            ),
            (
                "https://user:pw@nestodo.example.com:8443/api?x=1#top",
                "https://nestodo.example.com:8443/",
            ),
            (
                "https://nestodo.example.com:443/",
                "https://nestodo.example.com/",
            ),
            ("localhost:5173", "https://localhost:5173/"),
            ("http://localhost:5173/today", "http://localhost:5173/"),
            ("http://127.0.0.1:8787", "http://127.0.0.1:8787/"),
            ("http://[::1]:3000", "http://[::1]:3000/"),
        ] {
            assert_eq!(base_url_of(typed).unwrap().as_str(), base, "{typed}");
        }
    }

    #[test]
    fn refuses_plain_http_to_anything_but_localhost_so_tokens_never_travel_unencrypted() {
        for bad in [
            "http://nestodo.example.com",
            "http://192.168.1.10:5173",
            "http://localhost.evil.example",
        ] {
            assert_eq!(
                base_url_of(bad).unwrap_err(),
                "Enter an address that starts with https://.",
                "{bad}"
            );
        }
    }

    #[test]
    fn text_that_is_not_a_web_address_asks_for_one_instead_of_trying_to_connect() {
        for bad in [
            "",
            "  ",
            "my server",
            "https://",
            "ftp://nestodo.example.com",
        ] {
            assert_eq!(
                base_url_of(bad).unwrap_err(),
                "Enter an address such as nestodo.example.com.",
                "{bad:?}"
            );
        }
    }

    #[test]
    fn names_an_https_server_by_its_host_and_spells_out_http_so_a_dev_server_stands_out() {
        let label_of = |typed: &str| label(&base_url_of(typed).unwrap());
        assert_eq!(label_of("nestodo.example.com"), "nestodo.example.com");
        assert_eq!(
            label_of("nestodo.example.com:8443"),
            "nestodo.example.com:8443"
        );
        assert_eq!(label_of("http://127.0.0.1:5173"), "http://127.0.0.1:5173");
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

    fn webhook_edit(url: &str, token: Option<&str>) -> crate::webhook::WebhookEdit {
        crate::webhook::WebhookEdit {
            url: url.into(),
            url_secret: true,
            token: token.map(str::to_string),
            headers: vec![crate::webhook::WebhookHeader {
                name: "X-Api-Key".into(),
                value: String::new(),
                secret: true,
            }],
            project: "Work".into(),
        }
    }

    #[test]
    fn removing_the_saved_token_sends_an_explicit_null_because_a_blank_token_keeps_the_saved_one() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let put = server
            .mock("PUT", "/api/v1/webhook")
            .match_body(Matcher::Json(json!({
                "url": "",
                "urlSecret": true,
                "token": null,
                "headers": [{"name": "X-Api-Key", "value": "", "secret": true}],
                "project": "Work"
            })))
            .with_body(r#"{"available":true,"webhook":{"url":null,"hasToken":false,"unreadable":false,"headers":[],"project":"Work","lastSentAt":null,"lastError":null}}"#)
            .create();

        let saved = signed_in(&server, &store)
            .unwrap()
            .save_webhook(&webhook_edit("", None))
            .unwrap();

        put.assert();
        assert!(saved.available);
        assert_eq!(saved.webhook.map(|webhook| webhook.has_token), Some(false));
    }

    #[test]
    fn a_refused_webhook_value_names_its_field_so_the_form_can_show_the_message_under_it() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("PUT", "/api/v1/webhook")
            .with_status(400)
            .with_body(
                r#"{"error":"Validation failed","issues":[{"slot":"body","issues":[
                    {"message":"Enter an http or https URL","path":"url"},
                    {"message":"A header name is required","path":"headers.0.name"},
                    {"message":"Another problem with the URL","path":"url"}
                ]}]}"#,
            )
            .create();

        let refused = signed_in(&server, &store)
            .unwrap()
            .save_webhook(&webhook_edit("ftp://example.com", Some("")));

        let Err(Error::Refused(fields)) = refused else {
            panic!("expected the save to be refused field by field, got {refused:?}");
        };
        assert_eq!(
            fields,
            [
                ("url".to_string(), "Enter an http or https URL".to_string()),
                (
                    "headers".to_string(),
                    "A header name is required".to_string()
                ),
            ]
        );
    }

    #[test]
    fn removing_the_webhook_deletes_it_on_the_server_and_reports_a_failure_instead_of_pretending() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let delete = server
            .mock("DELETE", "/api/v1/webhook")
            .with_status(204)
            .create();
        let client = signed_in(&server, &store).unwrap();

        client.remove_webhook().unwrap();
        delete.assert();

        server
            .mock("DELETE", "/api/v1/webhook")
            .with_status(503)
            .with_body(r#"{"message":"The database is busy"}"#)
            .create();
        assert!(matches!(
            client.remove_webhook(),
            Err(Error::Server { status: 503, .. })
        ));
    }

    #[test]
    fn turning_usage_data_on_sends_the_choice_and_returns_what_the_server_saved() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let put = server
            .mock("PUT", "/api/v1/analytics")
            .match_body(Matcher::Json(json!({"enabled": true})))
            .with_body(r#"{"enabled":true}"#)
            .create();

        assert!(
            signed_in(&server, &store)
                .unwrap()
                .set_analytics(true)
                .unwrap()
        );
        put.assert();
    }

    #[test]
    fn a_member_who_never_set_a_profile_reads_as_no_name_and_no_avatar_so_the_app_falls_back_to_the_email()
     {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("GET", "/api/v1/profile")
            .with_body(r#"{"email":"kim@example.com","name":null,"avatarUrl":null}"#)
            .create();

        let profile = signed_in(&server, &store).unwrap().profile().unwrap();

        assert_eq!(
            profile,
            Profile {
                email: "kim@example.com".into(),
                name: None,
                avatar_url: None,
            }
        );
    }

    #[test]
    fn a_refused_name_names_its_field_so_the_form_can_show_the_message_under_it() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let put = server
            .mock("PUT", "/api/v1/profile")
            .match_body(Matcher::Json(json!({"name": "Kim\u{202e}"})))
            .with_status(400)
            .with_body(
                r#"{"error":"Validation failed","issues":[{"slot":"body","issues":[
                    {"message":"The name cannot contain control characters","path":"name"}
                ]}]}"#,
            )
            .create();

        let refused = signed_in(&server, &store).unwrap().save_name("Kim\u{202e}");

        put.assert();
        let Err(Error::Refused(fields)) = refused else {
            panic!("expected the name to be refused by field, got {refused:?}");
        };
        assert_eq!(
            fields,
            [(
                "name".to_string(),
                "The name cannot contain control characters".to_string()
            )]
        );
    }

    #[test]
    fn a_saved_name_comes_back_as_the_server_stored_it_so_the_menu_shows_the_trimmed_name() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        server
            .mock("PUT", "/api/v1/profile")
            .match_body(Matcher::Json(json!({"name": "  Kim  "})))
            .with_body(r#"{"email":"kim@example.com","name":"Kim","avatarUrl":null}"#)
            .create();

        let saved = signed_in(&server, &store)
            .unwrap()
            .save_name("  Kim  ")
            .unwrap();

        assert_eq!(saved.name.as_deref(), Some("Kim"));
    }

    #[test]
    fn an_avatar_is_sent_as_base64_of_the_exact_bytes_so_the_server_can_check_what_it_stores() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let png = b"\x89PNG\r\n\x1a\nnot really pixels".to_vec();
        let put = server
            .mock("PUT", "/api/v1/profile/avatar")
            .match_body(Matcher::Json(json!({"image": STANDARD.encode(&png)})))
            .with_body(
                r#"{"email":"kim@example.com","name":null,"avatarUrl":"/api/v1/avatars/7f1c"}"#,
            )
            .create();

        let saved = signed_in(&server, &store)
            .unwrap()
            .save_avatar(&png)
            .unwrap();

        put.assert();
        assert_eq!(saved.avatar_url.as_deref(), Some("/api/v1/avatars/7f1c"));
    }

    #[test]
    fn an_avatar_downloads_as_raw_bytes_with_the_login_because_the_server_only_shows_it_to_its_owner()
     {
        let mut server = Server::new();
        let saved = token("app", LATER);
        let store = store_with(&[(Kind::App, &saved)]);
        let image = vec![0x89, b'P', b'N', b'G', 0, 1, 2, 255];
        let get = server
            .mock("GET", "/api/v1/avatars/7f1c")
            .match_header("cf-access-token", saved.as_str())
            .with_header("content-type", "image/png")
            .with_body(image.clone())
            .create();
        let client = signed_in(&server, &store).unwrap();

        assert_eq!(client.avatar("/api/v1/avatars/7f1c").unwrap(), image);
        get.assert();

        server
            .mock("GET", "/api/v1/avatars/gone")
            .with_status(404)
            .with_body(r#"{"error":"not_found","message":"Avatar not found"}"#)
            .create();
        assert!(matches!(
            client.avatar("/api/v1/avatars/gone"),
            Err(Error::Server { status: 404, .. })
        ));
    }

    #[test]
    fn an_avatar_on_another_port_is_not_fetched_because_the_login_would_go_along_to_whoever_listens_there()
     {
        let server = Server::new();
        let mut other = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let elsewhere = other
            .mock("GET", Matcher::Any)
            .with_body(vec![0x89, b'P', b'N', b'G'])
            .expect(0)
            .create();
        let client = signed_in(&server, &store).unwrap();

        assert!(matches!(
            client.avatar(&format!("{}/api/v1/avatars/7f1c", other.url())),
            Err(Error::Invalid(_))
        ));
        elsewhere.assert();
    }

    #[test]
    fn an_avatar_url_outside_the_avatars_is_not_fetched_so_a_profile_cannot_send_the_login_to_another_endpoint()
     {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let anything = server.mock("GET", Matcher::Any).expect(0).create();
        let client = signed_in(&server, &store).unwrap();

        assert!(matches!(
            client.avatar("/api/v1/avatars/../todos"),
            Err(Error::Invalid(_))
        ));
        anything.assert();
    }

    #[test]
    fn removing_the_avatar_deletes_it_on_the_server_and_reports_a_failure_instead_of_pretending() {
        let mut server = Server::new();
        let store = store_with(&[(Kind::App, &token("app", LATER))]);
        let delete = server
            .mock("DELETE", "/api/v1/profile/avatar")
            .with_status(204)
            .create();
        let client = signed_in(&server, &store).unwrap();

        client.remove_avatar().unwrap();
        delete.assert();

        server
            .mock("DELETE", "/api/v1/profile/avatar")
            .with_status(503)
            .with_body(r#"{"message":"The database is busy"}"#)
            .create();
        assert!(matches!(
            client.remove_avatar(),
            Err(Error::Server { status: 503, .. })
        ));
    }
}
