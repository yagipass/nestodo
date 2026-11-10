use std::time::Duration;

use ureq::http::Method;
use url::Url;

use super::AccessApp;
use super::jwt::AccessToken;
use crate::error::Error;
use crate::http::{Call, Http, Response, is_loopback};

const TOKEN_COOKIE: &str = "CF_Authorization";
const APP_SESSION_COOKIE: &str = "CF_AppSession";
const LOGIN_PATH: &str = "/cdn-cgi/access/login";
const AUTHORIZED_PATH: &str = "/cdn-cgi/access/authorized";
const MAX_HOPS: usize = 10;
const TIMEOUT: Duration = Duration::from_secs(7);

pub fn exchange_org_token(
    http: &Http,
    base: &Url,
    app: &AccessApp,
    org_token: &str,
    now: i64,
) -> Result<Option<String>, Error> {
    let mut flow = Exchange::new(app, org_token, now);
    let mut url = base.clone();
    for _ in 0..MAX_HOPS {
        let mut call = Call::new(Method::HEAD, &url).timeout(TIMEOUT);
        if let Some(cookie) = flow.cookie_for(&url) {
            call = call.header("cookie", cookie);
        }
        let response = http.send(call)?;
        match flow.step(&url, &response) {
            Step::Done(token) => return Ok(token),
            Step::Follow(next) => url = next,
        }
    }
    Ok(None)
}

enum Step {
    Follow(Url),
    Done(Option<String>),
}

struct Exchange<'a> {
    app: &'a AccessApp,
    org_token: &'a str,
    app_session: Option<String>,
    now: i64,
}

impl<'a> Exchange<'a> {
    fn new(app: &'a AccessApp, org_token: &'a str, now: i64) -> Self {
        Self {
            app,
            org_token,
            app_session: None,
            now,
        }
    }

    fn cookie_for(&self, url: &Url) -> Option<String> {
        if is_secure_on(url, &self.app.auth_domain) && url.path().starts_with(LOGIN_PATH) {
            return Some(format!("{TOKEN_COOKIE}={}", self.org_token));
        }
        if is_secure_on(url, &self.app.hostname) && url.path().starts_with(AUTHORIZED_PATH) {
            return Some(format!(
                "{APP_SESSION_COOKIE}={}",
                self.app_session.as_ref()?
            ));
        }
        None
    }

    fn step(&mut self, url: &Url, response: &Response) -> Step {
        if !is_secure_on(url, &self.app.hostname) {
            return response
                .location(url)
                .map_or(Step::Done(None), Step::Follow);
        }
        if let Some(session) = response.cookie(APP_SESSION_COOKIE) {
            self.app_session = Some(session.to_string());
        }
        if let Some(token) = response.cookie(TOKEN_COOKIE)
            && AccessToken::parse(token)
                .is_some_and(|parsed| parsed.is_live(self.now) && parsed.is_for(&self.app.aud))
        {
            return Step::Done(Some(token.to_string()));
        }
        if url.path().starts_with(AUTHORIZED_PATH) {
            return Step::Done(None);
        }
        response
            .location(url)
            .map_or(Step::Done(None), Step::Follow)
    }
}

fn is_secure_on(url: &Url, host: &str) -> bool {
    url.host_str() == Some(host)
        && (url.scheme() == "https" || (url.scheme() == "http" && is_loopback(host)))
}

#[cfg(test)]
mod tests {
    use ureq::http::{HeaderMap, HeaderValue, StatusCode};

    use super::*;
    use crate::access::jwt;

    const NOW: i64 = 1_000;

    fn app() -> AccessApp {
        AccessApp {
            hostname: "nestodo.example.com".into(),
            aud: "app-aud".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        }
    }

    fn url(s: &str) -> Url {
        Url::parse(s).unwrap()
    }

    fn response(status: u16, headers: &[(&'static str, &str)]) -> Response {
        let mut map = HeaderMap::new();
        for (name, value) in headers {
            map.append(*name, HeaderValue::from_str(value).unwrap());
        }
        Response {
            status: StatusCode::from_u16(status).unwrap(),
            headers: map,
            body: Vec::new(),
        }
    }

    fn app_token(exp: i64, aud: &str) -> String {
        jwt::tests::token(&format!(r#"{{"exp":{exp},"aud":"{aud}"}}"#))
    }

    #[test]
    fn sends_the_org_token_only_to_the_team_login_page() {
        let app = app();
        let flow = Exchange::new(&app, "org-secret", NOW);
        let login = url(
            "https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com?kid=app-aud",
        );
        assert_eq!(
            flow.cookie_for(&login).as_deref(),
            Some("CF_Authorization=org-secret")
        );

        for elsewhere in [
            "https://evil.example/cdn-cgi/access/login/nestodo.example.com",
            "http://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com",
            "https://nestodo.example.com/cdn-cgi/access/login",
            "https://family.cloudflareaccess.com/other",
        ] {
            assert_eq!(flow.cookie_for(&url(elsewhere)), None, "{elsewhere}");
        }
    }

    #[test]
    fn carries_the_app_session_from_the_app_to_its_authorized_endpoint_only() {
        let app = app();
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let start = url("https://nestodo.example.com/");
        let first = response(
            302,
            &[
                (
                    "set-cookie",
                    "CF_AppSession=session-1; Path=/; Secure; HttpOnly",
                ),
                (
                    "location",
                    "https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com",
                ),
            ],
        );
        assert!(matches!(flow.step(&start, &first), Step::Follow(_)));

        let authorized = url("https://nestodo.example.com/cdn-cgi/access/authorized?token=x");
        assert_eq!(
            flow.cookie_for(&authorized).as_deref(),
            Some("CF_AppSession=session-1")
        );
        for elsewhere in [
            "https://evil.example/cdn-cgi/access/authorized",
            "http://nestodo.example.com/cdn-cgi/access/authorized",
        ] {
            assert_eq!(flow.cookie_for(&url(elsewhere)), None, "{elsewhere}");
        }
    }

    #[test]
    fn ignores_an_app_session_cookie_set_by_another_host() {
        let app = app();
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let login =
            url("https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com");
        let redirect = response(
            302,
            &[
                ("set-cookie", "CF_AppSession=planted"),
                (
                    "location",
                    "https://nestodo.example.com/cdn-cgi/access/authorized",
                ),
            ],
        );
        assert!(matches!(flow.step(&login, &redirect), Step::Follow(_)));
        assert_eq!(
            flow.cookie_for(&url(
                "https://nestodo.example.com/cdn-cgi/access/authorized"
            )),
            None
        );
    }

    #[test]
    fn returns_the_app_token_that_the_authorized_endpoint_sets() {
        let app = app();
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let token = app_token(NOW + 60, "app-aud");
        let authorized = url("https://nestodo.example.com/cdn-cgi/access/authorized");
        let cookie = format!("CF_Authorization={token}; Path=/; Secure; HttpOnly");
        let done = response(302, &[("set-cookie", &cookie), ("location", "/")]);
        assert!(matches!(flow.step(&authorized, &done), Step::Done(Some(t)) if t == token));
    }

    #[test]
    fn stops_without_a_token_when_access_clears_or_withholds_it() {
        let app = app();
        let authorized = url("https://nestodo.example.com/cdn-cgi/access/authorized");
        for cookie in [
            "CF_Authorization=; Max-Age=0".to_string(),
            format!("CF_Authorization={}", app_token(NOW - 1, "app-aud")),
            format!("CF_Authorization={}", app_token(NOW + 60, "other-aud")),
        ] {
            let mut flow = Exchange::new(&app, "org-secret", NOW);
            let cleared = response(302, &[("set-cookie", &cookie), ("location", "/")]);
            assert!(
                matches!(flow.step(&authorized, &cleared), Step::Done(None)),
                "{cookie}"
            );
        }
    }

    #[test]
    fn takes_nothing_from_the_app_over_plain_http_where_anyone_on_the_path_could_plant_it() {
        let app = app();
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let token = app_token(NOW + 60, "app-aud");
        let plain = url("http://nestodo.example.com/cdn-cgi/access/authorized");
        let planted = response(
            302,
            &[
                ("set-cookie", &format!("CF_Authorization={token}")),
                ("set-cookie", "CF_AppSession=planted"),
                ("location", "https://nestodo.example.com/"),
            ],
        );
        assert!(matches!(flow.step(&plain, &planted), Step::Follow(_)));
        assert_eq!(
            flow.cookie_for(&url(
                "https://nestodo.example.com/cdn-cgi/access/authorized"
            )),
            None
        );
    }

    #[test]
    fn trusts_plain_http_on_this_computer_where_a_local_server_runs_without_tls() {
        let app = AccessApp {
            hostname: "127.0.0.1".into(),
            ..app()
        };
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let token = app_token(NOW + 60, "app-aud");
        let local = url("http://127.0.0.1:5173/");
        let done = response(200, &[("set-cookie", &format!("CF_Authorization={token}"))]);
        assert!(matches!(flow.step(&local, &done), Step::Done(Some(t)) if t == token));
    }

    #[test]
    fn does_not_take_a_token_cookie_from_the_team_domain_for_the_app() {
        let app = app();
        let mut flow = Exchange::new(&app, "org-secret", NOW);
        let login =
            url("https://family.cloudflareaccess.com/cdn-cgi/access/login/nestodo.example.com");
        let cookie = format!("CF_Authorization={}", app_token(NOW + 60, "app-aud"));
        let redirect = response(
            302,
            &[
                ("set-cookie", &cookie),
                (
                    "location",
                    "https://nestodo.example.com/cdn-cgi/access/authorized",
                ),
            ],
        );
        assert!(matches!(flow.step(&login, &redirect), Step::Follow(_)));
    }
}
