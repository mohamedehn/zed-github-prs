use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Serialize, PartialEq)]
pub struct PullSummary {
    pub number: u64,
    pub title: String,
    pub author: String,
    pub draft: bool,
    pub head: String,
    pub base: String,
    pub url: String,
}

impl PullSummary {
    pub fn from_json(pr: &Value) -> Self {
        let text = |v: &Value| v.as_str().unwrap_or_default().to_owned();
        Self {
            number: pr["number"].as_u64().unwrap_or_default(),
            title: text(&pr["title"]),
            author: text(&pr["user"]["login"]),
            draft: pr["draft"].as_bool().unwrap_or(false),
            head: text(&pr["head"]["ref"]),
            base: text(&pr["base"]["ref"]),
            url: text(&pr["html_url"]),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReviewComment {
    pub path: String,
    pub line: u32,
    #[serde(default = "default_side")]
    pub side: String,
    pub body: String,
}

fn default_side() -> String {
    "RIGHT".to_owned()
}
