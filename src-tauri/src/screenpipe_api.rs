use crate::paths;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::OnceLock;

fn base() -> String {
    format!("http://127.0.0.1:{}", paths::PORT)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub content_type: Option<String>, // "ocr" | "audio" | "ui" | "all"
    pub app_name: Option<String>,
    pub window_name: Option<String>,
    pub start_time: Option<String>, // RFC3339
    pub end_time: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Pure: build the query-string key/value pairs for /search, omitting empty fields
/// and defaulting limit=50, offset=0. reqwest does the percent-encoding.
pub fn search_query_pairs(p: &SearchParams) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    if let Some(x) = &p.q { if !x.is_empty() { v.push(("q".into(), x.clone())); } }
    if let Some(x) = &p.content_type { v.push(("content_type".into(), x.clone())); }
    if let Some(x) = &p.app_name { if !x.is_empty() { v.push(("app_name".into(), x.clone())); } }
    if let Some(x) = &p.window_name { if !x.is_empty() { v.push(("window_name".into(), x.clone())); } }
    if let Some(x) = &p.start_time { if !x.is_empty() { v.push(("start_time".into(), x.clone())); } }
    if let Some(x) = &p.end_time { if !x.is_empty() { v.push(("end_time".into(), x.clone())); } }
    v.push(("limit".into(), p.limit.unwrap_or(50).to_string()));
    v.push(("offset".into(), p.offset.unwrap_or(0).to_string()));
    v
}

/// Pure: tag endpoint path for a content kind ("vision" | "audio") and row id.
pub fn tag_path(kind: &str, id: i64) -> String {
    format!("/tags/{kind}/{id}")
}

/// Shared HTTP client — reused across calls so we don't build a new connection
/// pool on every request (the health poll fires every 5s).
pub fn client() -> reqwest::Client {
    static C: OnceLock<reqwest::Client> = OnceLock::new();
    C.get_or_init(reqwest::Client::new).clone()
}

/// Fetch screenpipe's local API token by running `screenpipe auth token`, cached for the
/// process lifetime. Returns None if the binary isn't pinned yet or the call fails.
fn auth_token() -> Option<String> {
    static TOKEN: OnceLock<Option<String>> = OnceLock::new();
    TOKEN
        .get_or_init(|| {
            let out = std::process::Command::new(paths::recorder_binary())
                .args(["auth", "token"])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if t.is_empty() {
                None
            } else {
                Some(t)
            }
        })
        .clone()
}

/// Attach the bearer token to a request if we have one.
fn auth(rb: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    match auth_token() {
        Some(t) => rb.bearer_auth(t),
        None => rb,
    }
}

async fn get_json(path: &str, query: &[(String, String)]) -> Result<Value, String> {
    auth(client().get(format!("{}{}", base(), path)))
        .query(query)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn search(p: &SearchParams) -> Result<Value, String> {
    get_json("/search", &search_query_pairs(p)).await
}

pub async fn keyword(q: &str, limit: u32) -> Result<Value, String> {
    get_json("/search/keyword", &[("query".into(), q.into()), ("limit".into(), limit.to_string())]).await
}

pub async fn audio_devices() -> Result<Value, String> {
    get_json("/audio/list", &[]).await
}

pub async fn monitors() -> Result<Value, String> {
    get_json("/vision/list", &[]).await
}

pub async fn frame_ocr(id: i64) -> Result<Value, String> {
    get_json(&format!("/frames/{id}/ocr"), &[]).await
}

async fn post(path: &str, body: Value) -> Result<Value, String> {
    auth(client().post(format!("{}{}", base(), path)))
        .json(&body)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

pub async fn audio_start() -> Result<Value, String> {
    post("/audio/start", Value::Null).await
}

pub async fn audio_stop() -> Result<Value, String> {
    post("/audio/stop", Value::Null).await
}

pub async fn raw_sql(query: &str) -> Result<Value, String> {
    post("/raw_sql", serde_json::json!({ "query": query })).await
}

pub async fn add_tags(kind: &str, id: i64, tags: Vec<String>) -> Result<Value, String> {
    post(&tag_path(kind, id), serde_json::json!({ "tags": tags })).await
}

pub async fn remove_tags(kind: &str, id: i64, tags: Vec<String>) -> Result<Value, String> {
    auth(client().delete(format!("{}{}", base(), tag_path(kind, id))))
        .json(&serde_json::json!({ "tags": tags }))
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json::<Value>()
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_pairs_omit_empty_and_default_paging() {
        let p = SearchParams { q: Some("error".into()), content_type: Some("ocr".into()), ..Default::default() };
        let pairs = search_query_pairs(&p);
        assert!(pairs.contains(&("q".to_string(), "error".to_string())));
        assert!(pairs.contains(&("content_type".to_string(), "ocr".to_string())));
        assert!(pairs.contains(&("limit".to_string(), "50".to_string())));
        assert!(pairs.contains(&("offset".to_string(), "0".to_string())));
        assert!(!pairs.iter().any(|(k, _)| k == "app_name"));
    }

    #[test]
    fn empty_query_string_is_omitted() {
        let p = SearchParams { q: Some(String::new()), ..Default::default() };
        let pairs = search_query_pairs(&p);
        assert!(!pairs.iter().any(|(k, _)| k == "q"));
    }

    #[test]
    fn tag_path_builds() {
        assert_eq!(tag_path("vision", 42), "/tags/vision/42");
        assert_eq!(tag_path("audio", 7), "/tags/audio/7");
    }
}
