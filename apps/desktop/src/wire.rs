use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Todo {
    pub id: u64,
    pub parent_id: Option<u64>,
    #[serde(default)]
    pub position: u64,
    pub title: String,
    #[serde(default)]
    pub note: String,
    pub completed_at: Option<String>,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    pub method: &'static str,
    pub path: String,
    pub body: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Reply {
    pub status: u16,
    pub body: Option<Value>,
}

impl Reply {
    #[must_use]
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}
