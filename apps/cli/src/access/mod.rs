mod exchange;
pub mod jwt;
mod metadata;
mod transfer;

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

pub struct Session<'a> {
    app: AccessApp,
    store: &'a dyn TokenStore,
    app_token: String,
    refreshed: bool,
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

impl<'a> Session<'a> {
    pub fn open(
        http: &Http,
        base: &Url,
        app: AccessApp,
        store: &'a dyn TokenStore,
        now: i64,
    ) -> Result<Self, Error> {
        let app_token = store.load(&app.hostname, Kind::App)?;
        let org_token = store.load(&app.hostname, Kind::Org)?;
        let mut session = Session {
            app,
            store,
            app_token: String::new(),
            refreshed: false,
        };
        match plan(app_token, org_token, &session.app, now) {
            Plan::Use(token) => session.app_token = token,
            Plan::Exchange(org_token) => session.exchange(http, base, &org_token, now)?,
            Plan::SignIn => return Err(Error::NotSignedIn),
        }
        Ok(session)
    }

    pub fn token_for(&self, url: &Url) -> Option<&str> {
        (url.host_str() == Some(self.app.hostname.as_str())).then_some(self.app_token.as_str())
    }

    pub fn refresh(&mut self, http: &Http, base: &Url, now: i64) -> Result<(), Error> {
        if self.refreshed {
            return Err(Error::NotSignedIn);
        }
        let org_token = self.store.load(&self.app.hostname, Kind::Org)?;
        match plan(None, org_token, &self.app, now) {
            Plan::Exchange(org_token) => self.exchange(http, base, &org_token, now),
            _ => Err(Error::NotSignedIn),
        }
    }

    fn exchange(
        &mut self,
        http: &Http,
        base: &Url,
        org_token: &str,
        now: i64,
    ) -> Result<(), Error> {
        self.refreshed = true;
        let token = exchange::exchange_org_token(http, base, &self.app, org_token, now)?
            .ok_or(Error::NotSignedIn)?;
        self.store.save(&self.app.hostname, Kind::App, &token)?;
        self.app_token = token;
        Ok(())
    }
}

pub fn login(
    http: &Http,
    base: &Url,
    app: &AccessApp,
    store: &dyn TokenStore,
    now: i64,
) -> Result<String, Error> {
    let tokens = transfer::browser_login(http, base, app)?;
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
    fn asks_for_a_login_when_both_tokens_are_expired_or_missing() {
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
        let store = MemoryStore::default();
        let app = AccessApp {
            hostname: "nestodo.example.com".into(),
            aud: "app".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        };
        store
            .save("nestodo.example.com", Kind::App, &token(NOW + 60, "app"))
            .unwrap();
        let base = Url::parse("https://nestodo.example.com").unwrap();
        let session = Session::open(&Http::new(false), &base, app, &store, NOW).unwrap();

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
        let store = MemoryStore::default();
        store
            .save("nestodo.example.com", Kind::App, &token(NOW + 60, "app"))
            .unwrap();
        store
            .save("nestodo.example.com", Kind::Org, &token(NOW + 3600, "org"))
            .unwrap();
        let other = AccessApp {
            hostname: "other.example.com".into(),
            aud: "app".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
        };
        let base = Url::parse("https://other.example.com").unwrap();
        assert!(matches!(
            Session::open(&Http::new(false), &base, other, &store, NOW),
            Err(Error::NotSignedIn)
        ));
    }
}
