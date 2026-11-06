use std::time::Duration;

use ureq::http::{HeaderMap, Method, Request, StatusCode};
use ureq::{Agent, AsSendBody};
use url::Url;

use crate::error::Error;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Http {
    agent: Agent,
    verbose: bool,
}

pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    pub fn cookie(&self, name: &str) -> Option<&str> {
        self.headers
            .get_all("set-cookie")
            .iter()
            .filter_map(|value| value.to_str().ok())
            .filter_map(|cookie| cookie.split(';').next()?.trim().split_once('='))
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }

    pub fn location(&self, from: &Url) -> Option<Url> {
        if !self.status.is_redirection() {
            return None;
        }
        from.join(self.header("location")?).ok()
    }
}

pub struct Call<'a> {
    method: Method,
    url: &'a Url,
    headers: Vec<(&'static str, String)>,
    json: Option<Vec<u8>>,
    timeout: Duration,
}

impl<'a> Call<'a> {
    pub fn new(method: Method, url: &'a Url) -> Self {
        Self {
            method,
            url,
            headers: Vec::new(),
            json: None,
            timeout: DEFAULT_TIMEOUT,
        }
    }

    pub fn header(mut self, name: &'static str, value: impl Into<String>) -> Self {
        self.headers.push((name, value.into()));
        self
    }

    pub fn json(mut self, body: &serde_json::Value) -> Self {
        self.json = Some(body.to_string().into_bytes());
        self
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}

impl Http {
    pub fn new(verbose: bool) -> Self {
        let agent = Agent::config_builder()
            .max_redirects(0)
            .http_status_as_error(false)
            .user_agent(concat!("nestodo-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .into();
        Self { agent, verbose }
    }

    pub fn send(&self, call: Call) -> Result<Response, Error> {
        if self.verbose {
            eprintln!("> {} {}", call.method, redact(call.url));
        }
        let mut builder = Request::builder()
            .method(call.method.clone())
            .uri(call.url.as_str());
        for (name, value) in &call.headers {
            builder = builder.header(*name, value);
        }
        let result = match call.json {
            Some(json) => self.run(
                builder
                    .header("content-type", "application/json")
                    .body(json),
                call.timeout,
            ),
            None => self.run(builder.body(()), call.timeout),
        };
        let response = result.map_err(|error| Error::Network {
            url: redact(call.url),
            reason: error.to_string(),
        })?;
        if self.verbose {
            eprintln!("< {}", response.status);
        }
        Ok(response)
    }

    fn run<S: AsSendBody>(
        &self,
        request: Result<Request<S>, ureq::http::Error>,
        timeout: Duration,
    ) -> Result<Response, ureq::Error> {
        let request = self
            .agent
            .configure_request(request?)
            .timeout_global(Some(timeout))
            .build();
        let mut response = self.agent.run(request)?;
        let body = response.body_mut().read_to_vec()?;
        let (parts, _) = response.into_parts();
        Ok(Response {
            status: parts.status,
            headers: parts.headers,
            body,
        })
    }
}

pub fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "[::1]")
}

pub fn redact(url: &Url) -> String {
    let mut shown = url.clone();
    shown.set_query(None);
    shown.set_fragment(None);
    let _ = shown.set_username("");
    let _ = shown.set_password(None);
    shown.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacted_urls_drop_the_query_where_access_puts_tokens() {
        let url = Url::parse(
            "https://nestodo.example.com/cdn-cgi/access/authorized?token=eyJ.secret.sig&redirect_url=%2F",
        )
        .unwrap();
        assert_eq!(
            redact(&url),
            "https://nestodo.example.com/cdn-cgi/access/authorized"
        );
    }
}
