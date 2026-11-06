use serde::Deserialize;
use ureq::http::{Method, StatusCode};
use url::Url;

use super::AccessApp;
use super::jwt::{self, KeySet};
use crate::error::Error;
use crate::http::{Call, Http};

const ACCESS_DOMAIN_SUFFIX: &str = ".cloudflareaccess.com";
const MAX_AGE_SECS: i64 = 24 * 60 * 60;
const ALLOWED_SKEW_SECS: i64 = 5 * 60;
const MAX_KEY_SET_BYTES: usize = 1 << 20;

#[derive(Deserialize)]
pub struct Claims {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    auth_domain: String,
    #[serde(default)]
    aud: String,
    #[serde(default)]
    iat: i64,
}

pub fn discover(http: &Http, base: &Url, now: i64) -> Result<Option<AccessApp>, Error> {
    let response =
        http.send(Call::new(Method::HEAD, base).header("cf-access-metadata-request", "true"))?;
    let Some(token) = response.header("cf-access-metadata") else {
        return Ok(None);
    };
    let unverified: Claims = jwt::unverified_payload(token)
        .ok_or_else(|| Error::Access("the metadata JWT is malformed".into()))?;
    let auth_domain = auth_domain_url(&unverified.auth_domain).map_err(Error::Access)?;
    let keys = fetch_keys(http, &auth_domain)?;
    let claims: Claims = jwt::verify_rs256(token, &keys).map_err(Error::Access)?;
    let host = base.host_str().unwrap_or_default();
    validate(&claims, host, now).map_err(Error::Access)?;
    Ok(Some(AccessApp {
        hostname: host.to_string(),
        aud: claims.aud,
        auth_domain: auth_domain.host_str().unwrap_or_default().to_string(),
    }))
}

fn fetch_keys(http: &Http, auth_domain: &Url) -> Result<KeySet, Error> {
    let url = auth_domain
        .join("/cdn-cgi/access/certs")
        .expect("a static path joins onto an https origin");
    let response = http.send(Call::new(Method::GET, &url))?;
    if response.status != StatusCode::OK {
        return Err(Error::Access(format!(
            "{url} returned {}",
            response.status.as_u16()
        )));
    }
    if response.body.len() > MAX_KEY_SET_BYTES {
        return Err(Error::Access(format!(
            "{url} returned more than {MAX_KEY_SET_BYTES} bytes"
        )));
    }
    serde_json::from_slice(&response.body)
        .map_err(|_| Error::Access(format!("{url} did not return a key set")))
}

fn auth_domain_url(auth_domain: &str) -> Result<Url, String> {
    let parsed = Url::parse(&format!("https://{auth_domain}"))
        .map_err(|_| format!("auth_domain {auth_domain:?} is not a host name"))?;
    let host = parsed.host_str().unwrap_or_default().to_ascii_lowercase();
    if !host.ends_with(ACCESS_DOMAIN_SUFFIX) {
        return Err(format!(
            "auth_domain {auth_domain:?} is not a team domain under {ACCESS_DOMAIN_SUFFIX}"
        ));
    }
    Ok(Url::parse(&format!("https://{host}")).expect("a validated host name forms a URL"))
}

fn validate(claims: &Claims, request_host: &str, now: i64) -> Result<(), String> {
    if !claims.hostname.eq_ignore_ascii_case(request_host) {
        return Err(format!(
            "the metadata is for {:?}, not {request_host:?}",
            claims.hostname
        ));
    }
    if claims.kind != "match" {
        return Err(format!(
            "the metadata type is {:?}, not \"match\"",
            claims.kind
        ));
    }
    if claims.aud.is_empty() {
        return Err("the metadata has no application audience".into());
    }
    if claims.iat <= 0 {
        return Err("the metadata has no issue time".into());
    }
    if claims.iat < now - MAX_AGE_SECS {
        return Err("the metadata is more than 24 hours old".into());
    }
    if claims.iat > now + ALLOWED_SKEW_SECS {
        return Err(
            "the metadata is more than 5 minutes in the future; check the system clock".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_792_000_000;

    fn claims() -> Claims {
        Claims {
            kind: "match".into(),
            hostname: "nestodo.example.com".into(),
            auth_domain: "family.cloudflareaccess.com".into(),
            aud: "0123abcd".into(),
            iat: NOW,
        }
    }

    #[test]
    fn accepts_fresh_metadata_for_the_requested_host() {
        assert!(validate(&claims(), "nestodo.example.com", NOW).is_ok());
        assert!(validate(&claims(), "NESTODO.example.com", NOW + MAX_AGE_SECS).is_ok());
        assert!(validate(&claims(), "nestodo.example.com", NOW - ALLOWED_SKEW_SECS).is_ok());
    }

    #[test]
    fn rejects_metadata_issued_for_another_host_so_its_audience_cannot_be_reused() {
        assert!(validate(&claims(), "evil.example.com", NOW).is_err());
    }

    #[test]
    fn rejects_metadata_that_does_not_describe_a_matched_application() {
        let mismatch = Claims {
            kind: "no_match".into(),
            ..claims()
        };
        let no_aud = Claims {
            aud: String::new(),
            ..claims()
        };
        assert!(validate(&mismatch, "nestodo.example.com", NOW).is_err());
        assert!(validate(&no_aud, "nestodo.example.com", NOW).is_err());
    }

    #[test]
    fn rejects_replayed_or_future_metadata() {
        assert!(validate(&claims(), "nestodo.example.com", NOW + MAX_AGE_SECS + 1).is_err());
        assert!(
            validate(
                &claims(),
                "nestodo.example.com",
                NOW - ALLOWED_SKEW_SECS - 1
            )
            .is_err()
        );
        let missing = Claims { iat: 0, ..claims() };
        assert!(validate(&missing, "nestodo.example.com", NOW).is_err());
    }

    #[test]
    fn only_fetches_signing_keys_from_a_cloudflare_access_team_domain() {
        assert_eq!(
            auth_domain_url("Family.CloudflareAccess.com")
                .unwrap()
                .as_str(),
            "https://family.cloudflareaccess.com/"
        );
        for spoofed in [
            "evil.example.com",
            "cloudflareaccess.com.evil.example",
            "evil.example/x.cloudflareaccess.com",
            "family.cloudflareaccess.com@evil.example",
        ] {
            assert!(auth_domain_url(spoofed).is_err(), "{spoofed}");
        }
    }

    #[test]
    fn the_signed_fixture_carries_claims_that_pass_validation() {
        let keys: KeySet = serde_json::from_str(jwt::tests::METADATA_JWKS).unwrap();
        let verified: Claims = jwt::verify_rs256(jwt::tests::METADATA_JWT, &keys).unwrap();
        assert!(validate(&verified, "nestodo.example.com", NOW).is_ok());
        assert_eq!(verified.auth_domain, "family.cloudflareaccess.com");
    }
}
