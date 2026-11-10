mod exchange;
pub(crate) mod jwt;
mod metadata;
mod transfer;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use url::Url;

use crate::error::Error;
use crate::http::Http;
use crate::store::{Kind, TokenStore};
use jwt::AccessToken;

pub use metadata::discover;

#[derive(Debug, Clone, PartialEq)]
pub struct AccessApp {
    pub hostname: String,
    pub aud: String,
    pub auth_domain: String,
}

pub(crate) struct Session {
    app: AccessApp,
    store: Arc<dyn TokenStore>,
    app_token: Mutex<String>,
    signed_out: Mutex<bool>,
}

#[derive(Debug, PartialEq)]
enum Plan {
    Use(String),
    Exchange(String),
    SignIn,
}

fn plan(app_token: Option<String>, org_token: Option<String>, app: &AccessApp, now: i64) -> Plan {
    if let Some(token) = app_token
        && AccessToken::parse(&token)
            .is_some_and(|parsed| parsed.is_live(now) && parsed.is_for(&app.aud))
    {
        return Plan::Use(token);
    }
    match org_token {
        Some(token)
            if AccessToken::parse(&token).is_some_and(|parsed| {
                parsed.is_live(now) && parsed.is_issued_by(&app.auth_domain)
            }) =>
        {
            Plan::Exchange(token)
        }
        _ => Plan::SignIn,
    }
}

impl Session {
    pub fn open(
        http: &Http,
        base: &Url,
        app: AccessApp,
        store: Arc<dyn TokenStore>,
        now: i64,
    ) -> Result<Self, Error> {
        let app_token = store.load(&app.hostname, Kind::App)?;
        let org_token = store.load(&app.hostname, Kind::Org)?;
        let token = match plan(app_token, org_token, &app, now) {
            Plan::Use(token) => token,
            Plan::Exchange(org_token) => renew(http, base, &app, store.as_ref(), &org_token, now)?,
            Plan::SignIn => return Err(Error::NotSignedIn),
        };
        Ok(Session {
            app,
            store,
            app_token: Mutex::new(token),
            signed_out: Mutex::new(false),
        })
    }

    pub fn token_for(&self, url: &Url) -> Option<String> {
        (url.host_str() == Some(self.app.hostname.as_str())).then(|| self.token())
    }

    pub fn email(&self) -> Option<String> {
        AccessToken::parse(&self.token())?.email
    }

    pub fn refresh(
        &self,
        http: &Http,
        base: &Url,
        rejected: Option<&str>,
        now: i64,
    ) -> Result<(), Error> {
        let signed_out = self
            .signed_out
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if *signed_out {
            return Err(Error::NotSignedIn);
        }
        if rejected != Some(self.token().as_str()) {
            return Ok(());
        }
        let org_token = self.store.load(&self.app.hostname, Kind::Org)?;
        match plan(None, org_token, &self.app, now) {
            Plan::Exchange(org_token) => {
                let token = renew(http, base, &self.app, self.store.as_ref(), &org_token, now)?;
                *self
                    .app_token
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = token;
                Ok(())
            }
            _ => Err(Error::NotSignedIn),
        }
    }

    pub fn sign_out(&self, forget: impl FnOnce() -> Result<(), Error>) -> Result<(), Error> {
        let mut signed_out = self
            .signed_out
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let forgotten = forget();
        *signed_out = forgotten.is_ok();
        forgotten
    }

    fn token(&self) -> String {
        self.app_token
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

fn renew(
    http: &Http,
    base: &Url,
    app: &AccessApp,
    store: &dyn TokenStore,
    org_token: &str,
    now: i64,
) -> Result<String, Error> {
    let token =
        exchange::exchange_org_token(http, base, app, org_token, now)?.ok_or(Error::NotSignedIn)?;
    store.save(&app.hostname, Kind::App, &token)?;
    Ok(token)
}

pub struct PendingLogin {
    pub url: Url,
    app: AccessApp,
    transfer: transfer::Pending,
}

#[must_use]
pub fn start_login(base: &Url, app: &AccessApp) -> PendingLogin {
    let (url, transfer) = transfer::start(base, &app.aud);
    PendingLogin {
        url,
        app: app.clone(),
        transfer,
    }
}

pub fn finish_login(
    http: &Http,
    pending: &PendingLogin,
    store: &dyn TokenStore,
    now: i64,
    cancelled: &AtomicBool,
) -> Result<String, Error> {
    let tokens = transfer::receive(http, &pending.transfer, cancelled)?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    let app = &pending.app;
    let parsed = AccessToken::parse(&tokens.app_token)
        .filter(|parsed| parsed.is_live(now) && parsed.is_for(&app.aud))
        .ok_or_else(|| {
            Error::Login("Access returned a token that is not valid for this application".into())
        })?;
    store.save(&app.hostname, Kind::App, &tokens.app_token)?;
    if tokens.org_token.is_empty() {
        store.delete(&app.hostname, Kind::Org)?;
    } else {
        store.save(&app.hostname, Kind::Org, &tokens.org_token)?;
    }
    Ok(parsed.email.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use mockito::{Server, ServerGuard};

    use super::*;
    use crate::store::MemoryStore;

    const NOW: i64 = 1_000;

    fn token(exp: i64, aud: &str) -> String {
        issued_by(exp, aud, "family.cloudflareaccess.com")
    }

    fn issued_by(exp: i64, aud: &str, team: &str) -> String {
        jwt::tests::token(&format!(
            r#"{{"exp":{exp},"aud":["{aud}"],"iss":"https://{team}"}}"#
        ))
    }

    fn app() -> AccessApp {
        AccessApp {
            hostname: "nestodo.example.com".into(),
            aud: "app".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        }
    }

    #[test]
    fn uses_a_saved_app_token_while_it_is_live() {
        let saved = token(NOW + 60, "app");
        assert_eq!(
            plan(Some(saved.clone()), None, &app(), NOW),
            Plan::Use(saved)
        );
    }

    #[test]
    fn exchanges_the_org_token_without_a_browser_once_the_app_token_expires() {
        let org = token(NOW + 3600, "org");
        assert_eq!(
            plan(Some(token(NOW - 1, "app")), Some(org.clone()), &app(), NOW),
            Plan::Exchange(org.clone())
        );
        assert_eq!(
            plan(None, Some(org.clone()), &app(), NOW),
            Plan::Exchange(org)
        );
    }

    #[test]
    fn does_not_use_an_app_token_issued_for_another_application() {
        assert_eq!(
            plan(Some(token(NOW + 60, "other")), None, &app(), NOW),
            Plan::SignIn
        );
    }

    #[test]
    fn asks_for_a_sign_in_when_both_tokens_are_expired_or_missing() {
        assert_eq!(
            plan(
                Some(token(NOW - 1, "app")),
                Some(token(NOW - 1, "org")),
                &app(),
                NOW
            ),
            Plan::SignIn
        );
        assert_eq!(plan(None, None, &app(), NOW), Plan::SignIn);
        assert_eq!(
            plan(Some("garbage".into()), Some("garbage".into()), &app(), NOW),
            Plan::SignIn
        );
    }

    #[test]
    fn keeps_the_org_token_which_opens_every_app_of_its_team_away_from_other_teams() {
        let org = issued_by(NOW + 3600, "org", "family.cloudflareaccess.com");
        let elsewhere = AccessApp {
            auth_domain: "other.cloudflareaccess.com".into(),
            ..app()
        };
        assert_eq!(plan(None, Some(org.clone()), &elsewhere, NOW), Plan::SignIn);
        assert_eq!(
            plan(None, Some(org.clone()), &app(), NOW),
            Plan::Exchange(org)
        );
        let unissued = jwt::tests::token(&format!(r#"{{"exp":{}}}"#, NOW + 3600));
        assert_eq!(plan(None, Some(unissued), &app(), NOW), Plan::SignIn);
    }

    #[test]
    fn sends_the_app_token_only_to_the_host_it_was_issued_for() {
        let store = Arc::new(MemoryStore::default());
        store
            .save("nestodo.example.com", Kind::App, &token(NOW + 60, "app"))
            .unwrap();
        let base = Url::parse("https://nestodo.example.com").unwrap();
        let session = Session::open(&Http::new(), &base, app(), store, NOW).unwrap();

        let api = Url::parse("https://nestodo.example.com/api/v1/todos").unwrap();
        assert!(session.token_for(&api).is_some());
        for other in [
            "https://evil.example/api/v1/todos",
            "https://nestodo.example.com.evil.example/api/v1/todos",
            "https://sub.nestodo.example.com/api/v1/todos",
            "https://family.cloudflareaccess.com/cdn-cgi/access/login",
            "https://login.cloudflareaccess.org/transfer/key",
        ] {
            assert_eq!(
                session.token_for(&Url::parse(other).unwrap()),
                None,
                "{other}"
            );
        }
    }

    #[test]
    fn does_not_reuse_a_saved_token_after_switching_to_another_host() {
        let store = Arc::new(MemoryStore::default());
        store
            .save("nestodo.example.com", Kind::App, &token(NOW + 60, "app"))
            .unwrap();
        store
            .save("nestodo.example.com", Kind::Org, &token(NOW + 3600, "org"))
            .unwrap();
        let other = AccessApp {
            hostname: "other.example.com".into(),
            ..app()
        };
        let base = Url::parse("https://other.example.com").unwrap();
        assert!(matches!(
            Session::open(&Http::new(), &base, other, store, NOW),
            Err(Error::NotSignedIn)
        ));
    }

    fn pending(server: &ServerGuard) -> PendingLogin {
        PendingLogin {
            url: Url::parse("https://nestodo.example.com/cdn-cgi/access/cli").unwrap(),
            app: app(),
            transfer: transfer::Pending {
                url: Url::parse(&format!("{}/transfer/key", server.url())).unwrap(),
                secret: transfer::tests::client(),
            },
        }
    }

    fn signed_in_tokens(org: &str) -> String {
        let app_token = jwt::tests::token(&format!(
            r#"{{"exp":{},"aud":"app","email":"kim@example.com"}}"#,
            NOW + 60
        ));
        format!(r#"{{"app_token":"{app_token}","org_token":"{org}"}}"#)
    }

    #[test]
    fn a_finished_sign_in_saves_both_tokens_so_the_app_stays_signed_in_after_a_restart() {
        let mut server = Server::new();
        let org = token(NOW + 3600, "org");
        let (body, service_key) =
            transfer::tests::sealed_for(&transfer::tests::client(), &signed_in_tokens(&org));
        server
            .mock("GET", "/transfer/key")
            .with_header("service-public-key", &service_key)
            .with_body(body)
            .create();
        let store = MemoryStore::default();

        let email = finish_login(
            &Http::new(),
            &pending(&server),
            &store,
            NOW,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(email, "kim@example.com");
        let saved = store.load("nestodo.example.com", Kind::App).unwrap();
        assert!(matches!(plan(saved, None, &app(), NOW), Plan::Use(_)));
        assert_eq!(
            store.load("nestodo.example.com", Kind::Org).unwrap(),
            Some(org)
        );
    }

    #[test]
    fn a_sign_in_without_an_org_token_forgets_the_old_one_so_it_cannot_sign_in_as_someone_else() {
        let mut server = Server::new();
        let (body, service_key) =
            transfer::tests::sealed_for(&transfer::tests::client(), &signed_in_tokens(""));
        server
            .mock("GET", "/transfer/key")
            .with_header("service-public-key", &service_key)
            .with_body(body)
            .create();
        let store = MemoryStore::default();
        store
            .save("nestodo.example.com", Kind::Org, &token(NOW + 3600, "org"))
            .unwrap();

        finish_login(
            &Http::new(),
            &pending(&server),
            &store,
            NOW,
            &AtomicBool::new(false),
        )
        .unwrap();

        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn a_sign_in_cancelled_while_its_tokens_were_on_the_way_saves_nothing() {
        let mut server = Server::new();
        let (body, service_key) = transfer::tests::sealed_for(
            &transfer::tests::client(),
            &signed_in_tokens(&token(NOW + 3600, "org")),
        );
        let cancelled = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&cancelled);
        server
            .mock("GET", "/transfer/key")
            .with_header("service-public-key", &service_key)
            .with_body_from_request(move |_| {
                cancel.store(true, Ordering::Relaxed);
                body.clone().into_bytes()
            })
            .create();
        let store = MemoryStore::default();

        let result = finish_login(&Http::new(), &pending(&server), &store, NOW, &cancelled);

        assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
        assert_eq!(store.load("nestodo.example.com", Kind::Org).unwrap(), None);
    }

    #[test]
    fn refuses_a_sign_in_whose_token_does_not_open_this_application() {
        let mut server = Server::new();
        let foreign = jwt::tests::token(&format!(r#"{{"exp":{},"aud":"other"}}"#, NOW + 60));
        let (body, service_key) = transfer::tests::sealed_for(
            &transfer::tests::client(),
            &format!(r#"{{"app_token":"{foreign}","org_token":""}}"#),
        );
        server
            .mock("GET", "/transfer/key")
            .with_header("service-public-key", &service_key)
            .with_body(body)
            .create();
        let store = MemoryStore::default();

        let result = finish_login(
            &Http::new(),
            &pending(&server),
            &store,
            NOW,
            &AtomicBool::new(false),
        );

        assert!(matches!(result, Err(Error::Login(_))), "{result:?}");
        assert_eq!(store.load("nestodo.example.com", Kind::App).unwrap(), None);
    }
}
