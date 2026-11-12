use std::fmt::Write;
use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};
use serde_json::{Value, json};
use ureq::http::Method;
use url::Url;

use crate::error::Error;
use crate::http::{Call, Http};

const CAPTURE: &str = "https://us.i.posthog.com/i/v0/e/";
const TIMEOUT: Duration = Duration::from_secs(5);

#[must_use]
pub fn capture_url() -> Url {
    Url::parse(CAPTURE).expect("the PostHog address is a URL")
}

#[must_use]
pub fn new_id() -> Option<String> {
    let mut bytes = [0u8; 16];
    SystemRandom::new().fill(&mut bytes).ok()?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = bytes
        .iter()
        .fold(String::with_capacity(32), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        });
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[must_use]
pub fn screen_event(token: &str, id: &str, screen: &str) -> Value {
    json!({
        "api_key": token,
        "event": "$screen",
        "distinct_id": id,
        "properties": {
            "$screen_name": screen,
            "$process_person_profile": false,
            "$lib": "nestodo-desktop",
            "$lib_version": env!("CARGO_PKG_VERSION"),
            "$app_version": env!("CARGO_PKG_VERSION"),
            "$os": "macOS",
            "$device_type": "Desktop",
        },
    })
}

pub fn send_screen(capture: &Url, token: &str, id: &str, screen: &str) -> Result<(), Error> {
    let response = Http::new().send(
        Call::new(Method::POST, capture)
            .json(&screen_event(token, id, screen))
            .timeout(TIMEOUT),
    )?;
    if response.status.is_success() {
        Ok(())
    } else {
        Err(Error::Server {
            status: response.status.as_u16(),
            message: String::from_utf8_lossy(&response.body).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use mockito::{Matcher, Server};

    use super::*;

    #[test]
    fn the_event_names_only_the_screen_and_the_app_so_no_todo_address_or_email_reaches_posthog() {
        let mut posthog = Server::new();
        let capture = Url::parse(&format!("{}/i/v0/e/", posthog.url())).unwrap();
        let sent = posthog
            .mock("POST", "/i/v0/e/")
            .match_header("content-type", "application/json")
            .match_body(Matcher::Json(json!({
                "api_key": "phc_test",
                "event": "$screen",
                "distinct_id": "0f8fad5b-d9cb-469f-a165-70867728950e",
                "properties": {
                    "$screen_name": "Stats",
                    "$process_person_profile": false,
                    "$lib": "nestodo-desktop",
                    "$lib_version": env!("CARGO_PKG_VERSION"),
                    "$app_version": env!("CARGO_PKG_VERSION"),
                    "$os": "macOS",
                    "$device_type": "Desktop",
                },
            })))
            .with_body("{\"status\":\"Ok\"}")
            .create();

        send_screen(
            &capture,
            "phc_test",
            "0f8fad5b-d9cb-469f-a165-70867728950e",
            "Stats",
        )
        .unwrap();

        sent.assert();
    }

    #[test]
    fn a_refused_event_is_reported_so_it_is_not_mistaken_for_one_that_arrived() {
        let mut posthog = Server::new();
        let capture = Url::parse(&format!("{}/i/v0/e/", posthog.url())).unwrap();
        posthog.mock("POST", "/i/v0/e/").with_status(401).create();

        assert!(send_screen(&capture, "phc_wrong", "id", "Todos").is_err());
    }

    #[test]
    fn each_install_gets_a_fresh_random_id_shaped_like_the_ids_posthog_makes() {
        let first = new_id().unwrap();
        let second = new_id().unwrap();

        assert_ne!(first, second);
        for id in [&first, &second] {
            let groups: Vec<&str> = id.split('-').collect();
            assert_eq!(
                groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
                [8, 4, 4, 4, 12]
            );
            assert!(id.chars().all(|c| c == '-' || c.is_ascii_hexdigit()));
            assert_eq!(&id[14..15], "4");
            assert!("89ab".contains(&id[19..20]));
        }
    }

    #[test]
    fn events_go_to_posthogs_us_cloud_like_the_web_app() {
        assert_eq!(capture_url().as_str(), "https://us.i.posthog.com/i/v0/e/");
    }
}
