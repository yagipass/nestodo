use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebhookHeader {
    pub name: String,
    pub value: String,
    pub secret: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Webhook {
    pub url: Option<String>,
    #[serde(default)]
    pub has_token: bool,
    #[serde(default)]
    pub unreadable: bool,
    #[serde(default)]
    pub headers: Vec<WebhookHeader>,
    pub project: String,
    pub last_sent_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Webhooks {
    pub available: bool,
    pub webhook: Option<Webhook>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Analytics {
    #[serde(default)]
    pub available: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebhookEdit {
    pub url: String,
    pub url_secret: bool,
    pub token: Option<String>,
    pub headers: Vec<WebhookHeader>,
    pub project: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookForm {
    pub url: String,
    pub url_secret: bool,
    pub token: Option<String>,
    pub headers: Vec<WebhookHeader>,
    pub project: String,
    pub existing: Option<Webhook>,
}

impl WebhookForm {
    #[must_use]
    pub fn of(webhook: Option<&Webhook>) -> Self {
        Self {
            url: webhook
                .and_then(|webhook| webhook.url.clone())
                .unwrap_or_default(),
            url_secret: webhook.is_none_or(|webhook| webhook.url.is_none()),
            token: Some(String::new()),
            headers: webhook
                .map(|webhook| webhook.headers.clone())
                .unwrap_or_default(),
            project: webhook
                .map(|webhook| webhook.project.clone())
                .unwrap_or_default(),
            existing: webhook.cloned(),
        }
    }

    fn saved(&self) -> Option<&Webhook> {
        self.existing.as_ref().filter(|webhook| !webhook.unreadable)
    }

    #[must_use]
    pub fn replacing(&self) -> bool {
        self.saved().is_some()
    }

    #[must_use]
    pub fn keeps_hidden_url(&self) -> bool {
        self.saved().is_some_and(|saved| saved.url.is_none()) && self.url_secret
    }

    fn same_url(&self) -> bool {
        self.url
            == self
                .saved()
                .and_then(|saved| saved.url.as_deref())
                .unwrap_or_default()
    }

    #[must_use]
    pub fn token_kept(&self) -> bool {
        self.saved().is_some_and(|saved| saved.has_token)
            && self.same_url()
            && self.token.as_deref() == Some("")
    }

    #[must_use]
    pub fn value_kept(&self, header: &WebhookHeader) -> bool {
        let name = header.name.trim();
        header.secret
            && self.same_url()
            && self.saved().is_some_and(|saved| {
                saved
                    .headers
                    .iter()
                    .any(|kept| kept.secret && kept.name.eq_ignore_ascii_case(name))
            })
    }

    #[must_use]
    pub fn edit(&self) -> WebhookEdit {
        WebhookEdit {
            url: self.url.clone(),
            url_secret: self.url_secret,
            token: self.token.clone(),
            headers: self.headers.clone(),
            project: self.project.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved() -> Webhook {
        Webhook {
            url: None,
            has_token: true,
            unreadable: false,
            headers: vec![WebhookHeader {
                name: "X-Api-Key".into(),
                value: String::new(),
                secret: true,
            }],
            project: "Side projects".into(),
            last_sent_at: None,
            last_error: None,
        }
    }

    fn header(name: &str, secret: bool) -> WebhookHeader {
        WebhookHeader {
            name: name.into(),
            value: String::new(),
            secret,
        }
    }

    #[test]
    fn a_new_webhook_keeps_its_url_secret_unless_asked_since_a_url_can_itself_hold_a_secret() {
        let form = WebhookForm::of(None);
        assert!(form.url_secret);
        assert_eq!(
            form.edit(),
            WebhookEdit {
                url: String::new(),
                url_secret: true,
                token: Some(String::new()),
                headers: Vec::new(),
                project: String::new(),
            }
        );
    }

    #[test]
    fn the_saved_token_is_kept_only_while_the_url_stays_the_same_so_the_form_never_promises_to_send_it_elsewhere()
     {
        let saved = saved();
        let form = WebhookForm::of(Some(&saved));
        assert!(form.token_kept());
        assert!(
            !WebhookForm {
                url: "https://other.example/hook".into(),
                ..form.clone()
            }
            .token_kept()
        );
        assert!(
            !WebhookForm {
                token: None,
                ..form
            }
            .token_kept()
        );
    }

    #[test]
    fn a_secret_header_value_is_kept_only_for_the_same_name_and_url_whatever_the_letter_case() {
        let saved = saved();
        let form = WebhookForm::of(Some(&saved));
        assert!(form.value_kept(&header(" x-api-key ", true)));
        assert!(!form.value_kept(&header("X-Api-Version", true)));
        assert!(!form.value_kept(&header("X-Api-Key", false)));
        let moved = WebhookForm {
            url: "https://other.example/hook".into(),
            ..form
        };
        assert!(!moved.value_kept(&header("X-Api-Key", true)));
    }

    #[test]
    fn a_hidden_saved_url_may_be_left_blank_and_the_form_says_so_only_while_it_stays_secret() {
        let saved = saved();
        let form = WebhookForm::of(Some(&saved));
        assert!(form.keeps_hidden_url());
        assert!(
            !WebhookForm {
                url_secret: false,
                ..form
            }
            .keeps_hidden_url()
        );
        let shown = Webhook {
            url: Some("https://example.com/hook".into()),
            ..saved
        };
        assert!(!WebhookForm::of(Some(&shown)).keeps_hidden_url());
    }

    #[test]
    fn a_webhook_that_can_no_longer_be_read_keeps_nothing_so_every_value_is_asked_for_again() {
        let unreadable = Webhook {
            unreadable: true,
            ..saved()
        };
        let form = WebhookForm::of(Some(&unreadable));
        assert!(!form.replacing());
        assert!(!form.token_kept());
        assert!(!form.keeps_hidden_url());
        assert!(!form.value_kept(&header("X-Api-Key", true)));
    }

    #[test]
    fn an_older_server_that_does_not_say_whether_it_sends_usage_data_shows_no_switch() {
        let analytics: Analytics = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
        assert!(!analytics.available);
    }
}
