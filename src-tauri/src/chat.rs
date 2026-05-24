use crate::{pipes, screenpipe_api};
use serde_json::{json, Value};

struct Preset {
    provider: String,
    url: String,
    model: String,
    api_key: String,
}

/// Resolve the default model preset (or the only one) with its raw credentials.
fn default_preset() -> Result<Preset, String> {
    let list = pipes::models_list()?;
    let arr = list.as_array().cloned().unwrap_or_default();
    if arr.is_empty() {
        return Err("No AI preset configured — add one in the Settings tab.".into());
    }
    let pick = arr
        .iter()
        .find(|p| {
            ["default", "is_default", "defaultPreset"]
                .iter()
                .any(|k| p.get(*k).and_then(|v| v.as_bool()).unwrap_or(false))
        })
        .or_else(|| arr.first())
        .unwrap();
    let id = pick
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or("preset has no id")?;
    let show = pipes::models_show(id)?;
    let s = |k: &str| show.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    Ok(Preset {
        provider: s("provider"),
        url: s("url"),
        model: s("model"),
        api_key: s("apiKey"),
    })
}

fn endpoint(p: &Preset) -> String {
    let base = p.url.trim_end_matches('/');
    if p.provider == "native-ollama" {
        format!("{base}/v1/chat/completions")
    } else {
        format!("{base}/chat/completions")
    }
}

/// Pull recent relevant screen+audio activity to ground the answer.
async fn build_context(question: &str) -> String {
    let params = screenpipe_api::SearchParams {
        q: Some(question.to_string()),
        content_type: Some("all".into()),
        limit: Some(15),
        ..Default::default()
    };
    let Ok(res) = screenpipe_api::search(&params).await else {
        return "(could not search recordings)".into();
    };
    let mut out = String::new();
    if let Some(arr) = res.get("data").and_then(|d| d.as_array()) {
        for hit in arr.iter().take(15) {
            let c = hit.get("content").unwrap_or(hit);
            let text = c
                .get("text")
                .or_else(|| c.get("transcription"))
                .or_else(|| c.get("ocr_text"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let app = c.get("app_name").and_then(|v| v.as_str()).unwrap_or("");
            let ts = c.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");
            let snippet: String = text.chars().take(300).collect();
            out.push_str(&format!("[{ts} {app}] {snippet}\n"));
        }
    }
    if out.is_empty() {
        "(no relevant recordings found)".into()
    } else {
        out
    }
}

/// Answer a question about the user's recordings via their default model preset.
pub async fn chat(question: &str) -> Result<String, String> {
    let p = default_preset()?;
    if p.url.is_empty() || p.model.is_empty() {
        return Err("Default preset is missing a URL or model — check the Settings tab.".into());
    }
    if p.provider == "anthropic" {
        return Err("Chat via the Anthropic provider isn't wired yet — use a custom/openai/ollama preset (DeepSeek works).".into());
    }
    let context = build_context(question).await;
    let body = json!({
        "model": p.model,
        "messages": [
            {"role": "system", "content": format!("You are Meye, a local assistant that answers questions about the user's recorded screen and audio. Use this captured context when relevant:\n\n{context}")},
            {"role": "user", "content": question}
        ],
        "max_tokens": 1024
    });
    let mut req = reqwest::Client::new()
        .post(endpoint(&p))
        .timeout(std::time::Duration::from_secs(60))
        .json(&body);
    if !p.api_key.is_empty() {
        req = req.bearer_auth(&p.api_key);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    v.get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("unexpected model response: {v}"))
}
