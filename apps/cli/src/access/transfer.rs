use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use crypto_box::aead::{Aead, OsRng};
use crypto_box::{Nonce, PublicKey, SalsaBox, SecretKey};
use serde::Deserialize;
use ureq::http::{Method, StatusCode};
use url::Url;

use super::AccessApp;
use crate::error::Error;
use crate::http::{Call, Http};

const TRANSFER_URL: &str = "https://login.cloudflareaccess.org/transfer/";
const POLL_TIMEOUT: Duration = Duration::from_secs(60);
const POLL_ATTEMPTS: usize = 10;
const NONCE_LEN: usize = 24;

#[derive(Deserialize)]
pub struct Tokens {
    pub app_token: String,
    pub org_token: String,
}

pub fn browser_login(http: &Http, base: &Url, app: &AccessApp) -> Result<Tokens, Error> {
    let secret = SecretKey::generate(&mut OsRng);
    let public_key = URL_SAFE.encode(secret.public_key().as_bytes());
    let login = login_url(base, &app.aud, &public_key);

    if webbrowser::open(login.as_str()).is_ok() {
        eprintln!(
            "Sign in at this URL, which just opened in your browser:\n\n{login}\n\nIf nothing opened, copy it into a browser yourself."
        );
    } else {
        eprintln!("Sign in by copying this URL into a browser:\n\n{login}\n");
    }

    let transfer =
        Url::parse(&format!("{TRANSFER_URL}{public_key}")).expect("the transfer URL is valid");
    let (body, service_key) = poll(http, &transfer)?;
    decrypt(&secret, &body, &service_key)
}

fn login_url(base: &Url, aud: &str, public_key: &str) -> Url {
    let mut redirect = base.clone();
    redirect
        .query_pairs_mut()
        .clear()
        .append_pair("aud", aud)
        .append_pair("token", public_key);

    let mut login = base.clone();
    login.set_path("/cdn-cgi/access/cli");
    login
        .query_pairs_mut()
        .clear()
        .append_pair("aud", aud)
        .append_pair("edge_token_transfer", "true")
        .append_pair("redirect_url", redirect.as_str())
        .append_pair("send_org_token", "true")
        .append_pair("token", public_key);
    login
}

fn poll(http: &Http, transfer: &Url) -> Result<(Vec<u8>, String), Error> {
    for attempt in 0..POLL_ATTEMPTS {
        let response = http.send(Call::new(Method::GET, transfer).timeout(POLL_TIMEOUT))?;
        if response.status.is_server_error() {
            return Err(Error::Login(format!(
                "the Cloudflare login service returned {}",
                response.status.as_u16()
            )));
        }
        if response.status == StatusCode::OK && !response.body.is_empty() {
            let key = response
                .header("service-public-key")
                .unwrap_or_default()
                .to_string();
            return Ok((response.body, key));
        }
        if attempt == 0 {
            eprintln!("Waiting for you to sign in...");
        }
    }
    Err(Error::Login(
        "timed out waiting for the browser login".into(),
    ))
}

fn decrypt(secret: &SecretKey, body: &[u8], service_key: &str) -> Result<Tokens, Error> {
    let fail = |reason: &str| Error::Login(format!("could not read the login response: {reason}"));
    let sealed = STANDARD
        .decode(body.trim_ascii())
        .map_err(|_| fail("it is not base64"))?;
    if sealed.len() < NONCE_LEN {
        return Err(fail("it is too short"));
    }
    let service_key: [u8; 32] = URL_SAFE
        .decode(service_key)
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| fail("the service public key is missing or malformed"))?;
    let (nonce, ciphertext) = sealed.split_at(NONCE_LEN);
    let plaintext = SalsaBox::new(&PublicKey::from(service_key), secret)
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| fail("it was not encrypted for this login"))?;
    serde_json::from_slice(&plaintext).map_err(|_| fail("it does not contain tokens"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIENT_PUBLIC_KEY: &str = "e06Qm75__kTEZaIgA31gjuNYl9Me-XLwf3SJLLD3PxM=";
    const SERVICE_PUBLIC_KEY: &str = "D6poTtKIZ7l_Smot7l34zpdOdrcBjj8iocTPJnhXDyA=";
    const SEALED_BY_GO_NACL_BOX: &str = "MzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzidj3Q9nvJVpYeA5GREvgT00+r+nOtpkzy9d1AaPWpzWNOUW1mmO/ix6g4qL6MdB1hck3rK6cMyapJz88p+szI7mtZ9VmmzLWETpjrrA=";

    fn client() -> SecretKey {
        SecretKey::from([0x11; 32])
    }

    #[test]
    fn opens_a_response_sealed_by_the_same_nacl_box_that_cloudflared_uses() {
        let tokens = decrypt(
            &client(),
            SEALED_BY_GO_NACL_BOX.as_bytes(),
            SERVICE_PUBLIC_KEY,
        )
        .unwrap();
        assert_eq!(tokens.app_token, "app.token.value");
        assert_eq!(tokens.org_token, "org.token.value");
    }

    #[test]
    fn encodes_the_public_key_the_way_the_transfer_service_expects() {
        assert_eq!(
            URL_SAFE.encode(client().public_key().as_bytes()),
            CLIENT_PUBLIC_KEY
        );
    }

    #[test]
    fn refuses_a_response_that_was_not_sealed_for_this_login() {
        let stranger = SecretKey::from([0x44; 32]);
        assert!(
            decrypt(
                &stranger,
                SEALED_BY_GO_NACL_BOX.as_bytes(),
                SERVICE_PUBLIC_KEY
            )
            .is_err()
        );

        let other_service = URL_SAFE.encode(SecretKey::from([0x55; 32]).public_key().as_bytes());
        assert!(decrypt(&client(), SEALED_BY_GO_NACL_BOX.as_bytes(), &other_service).is_err());
    }

    #[test]
    fn refuses_malformed_responses() {
        assert!(decrypt(&client(), b"not base64!", SERVICE_PUBLIC_KEY).is_err());
        assert!(
            decrypt(
                &client(),
                STANDARD.encode([0u8; 23]).as_bytes(),
                SERVICE_PUBLIC_KEY
            )
            .is_err()
        );
        assert!(decrypt(&client(), SEALED_BY_GO_NACL_BOX.as_bytes(), "").is_err());
    }

    #[test]
    fn builds_the_same_login_url_as_cloudflared() {
        let base = Url::parse("https://nestodo.example.com").unwrap();
        let login = login_url(&base, "app-aud", CLIENT_PUBLIC_KEY);
        assert_eq!(
            login.as_str(),
            include_str!("testdata/login-url.txt").trim_ascii()
        );
    }
}
