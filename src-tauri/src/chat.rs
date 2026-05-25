use crate::{pipes, screenpipe_api};
use serde::Serialize;
use serde_json::{json, Value};

struct Preset {
    provider: String,
    url: String,
    model: String,
    api_key: String,
}

/// One recording the answer was grounded in.
#[derive(Serialize)]
pub struct Source {
    ts: String,
    app: String,
    text: String,
    frame_id: Option<i64>,
}

/// Chat answer plus the recordings used as context.
#[derive(Serialize)]
pub struct ChatReply {
    answer: String,
    sources: Vec<Source>,
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

/// Collect hits from a /search response into the prompt + sources, de-duplicating.
fn collect(
    res: &Value,
    prompt: &mut String,
    sources: &mut Vec<Source>,
    seen: &mut std::collections::HashSet<String>,
) {
    let Some(arr) = res.get("data").and_then(|d| d.as_array()) else {
        return;
    };
    for hit in arr {
        if sources.len() >= 30 {
            break;
        }
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
        let key: String = format!("{ts}|{}", text.chars().take(40).collect::<String>());
        if !seen.insert(key) {
            continue;
        }
        let frame_id = c
            .get("frame_id")
            .or_else(|| c.get("frameId"))
            .or_else(|| c.get("id"))
            .and_then(|v| v.as_i64());
        let snippet: String = text.chars().take(300).collect();
        prompt.push_str(&format!("[{ts} {app}] {snippet}\n"));
        sources.push(Source {
            ts: ts.to_string(),
            app: app.to_string(),
            text: snippet,
            frame_id,
        });
    }
}

/// Grounding context: most-recent activity (so "today"-style questions work) plus
/// keyword hits for the question. Returns (prompt context, sources for display).
async fn build_context(question: &str) -> (String, Vec<Source>) {
    let mut prompt = String::new();
    let mut sources: Vec<Source> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Keyword-relevant hits first (good for "find the X I saw" questions).
    if let Ok(res) = screenpipe_api::search(&screenpipe_api::SearchParams {
        q: Some(question.to_string()),
        content_type: Some("all".into()),
        limit: Some(12),
        ..Default::default()
    })
    .await
    {
        collect(&res, &mut prompt, &mut sources, &mut seen);
    }

    // Then a little most-recent activity (so "what did I do today" style questions still
    // ground), but kept modest so it doesn't dominate general questions.
    if let Ok(res) = screenpipe_api::search(&screenpipe_api::SearchParams {
        content_type: Some("all".into()),
        limit: Some(12),
        ..Default::default()
    })
    .await
    {
        collect(&res, &mut prompt, &mut sources, &mut seen);
    }

    // Empty => the caller sends no context at all (plain general-assistant chat).
    (prompt, sources)
}

/// Answer a question about the user's recordings via their default model preset.
pub async fn chat(question: &str) -> Result<ChatReply, String> {
    let p = default_preset()?;
    if p.url.is_empty() || p.model.is_empty() {
        return Err("Default preset is missing a URL or model — check the Settings tab.".into());
    }
    if p.provider == "anthropic" {
        return Err("Chat via the Anthropic provider isn't wired yet — use a custom/openai/ollama preset (DeepSeek works).".into());
    }
    let (context, mut sources) = build_context(question).await;
    let system = if context.trim().is_empty() {
        "You are Meye, a helpful, knowledgeable personal assistant. Answer naturally and \
         conversationally like a capable general-purpose AI."
            .to_string()
    } else {
        format!(
            "You are Meye, a helpful, knowledgeable personal assistant. Answer naturally and \
             conversationally like a capable general-purpose AI.\n\n\
             You ALSO have access to snippets of the user's recent screen/audio recordings, shown \
             below as OPTIONAL background. Use them only when the question is actually about what \
             the user did, saw, heard, or worked on — or when they're clearly relevant. For general \
             questions, ignore the recordings and just answer well. Never force an answer to be \
             about the user's activity.\n\n--- Recent recordings (optional context) ---\n{context}"
        )
    };
    // Don't advertise "sources" for a general chat where the recordings weren't the point.
    if context.trim().is_empty() {
        sources.clear();
    }
    let body = json!({
        "model": p.model,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": question}
        ],
        "max_tokens": 1024
    });
    let mut req = crate::screenpipe_api::client()
        .post(endpoint(&p))
        .timeout(std::time::Duration::from_secs(60))
        .json(&body);
    if !p.api_key.is_empty() {
        req = req.bearer_auth(&p.api_key);
    }
    let resp = req.send().await.map_err(|e| e.to_string())?;
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let answer = v
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|s| s.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("unexpected model response: {v}"))?;
    Ok(ChatReply { answer, sources })
}

#[cfg(test)]
mod probe {
    //! Live probes (ignored by default — hit real screenpipe + the DeepSeek API).
    //! Run with: cargo test --lib chat:: -- --ignored --nocapture
    use super::*;

    fn run(q: &str) -> ChatReply {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(chat(q)).expect("chat() failed")
    }

    #[test]
    #[ignore]
    fn generic_question_not_forced_to_activity() {
        let r = run("In one sentence, what is a monad in functional programming?");
        println!("\n[GENERIC] sources={}\n{}\n", r.sources.len(), r.answer);
    }

    #[test]
    #[ignore]
    fn activity_question_uses_recordings() {
        let r = run("In one short sentence, what app was I most recently using on my screen?");
        println!("\n[ACTIVITY] sources={}\n{}\n", r.sources.len(), r.answer);
    }
}
