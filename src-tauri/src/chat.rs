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

/// Run one recording search and fold its hits into the shared sources/seen, returning the
/// snippet text to hand back to the model as a tool result.
async fn run_search(
    query: &str,
    sources: &mut Vec<Source>,
    seen: &mut std::collections::HashSet<String>,
) -> String {
    let mut snippets = String::new();
    if let Ok(res) = screenpipe_api::search(&screenpipe_api::SearchParams {
        q: Some(query.to_string()),
        content_type: Some("all".into()),
        limit: Some(15),
        ..Default::default()
    })
    .await
    {
        collect(&res, &mut snippets, sources, seen);
    }
    snippets
}

/// The tool the model may call to look at the user's recordings. By exposing this as an
/// *optional* tool (rather than always injecting context), the model itself decides whether
/// the question needs the recordings — so general questions never get forced through activity,
/// and "sources" only appear when the model actually searched.
fn search_tool() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": "search_recordings",
            "description": "Search the user's own recorded screen text (OCR) and audio transcripts. \
                Call this ONLY when answering needs the user's personal context — what they did, \
                saw, heard, worked on, or any reference to 'I/my/me/today/earlier/this'. Do NOT call \
                it for general knowledge questions you can answer on your own.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Keywords to search the recordings for." }
                },
                "required": ["query"]
            }
        }
    })
}

fn post_body(p: &Preset, body: &Value) -> reqwest::RequestBuilder {
    let mut req = crate::screenpipe_api::client()
        .post(endpoint(p))
        .timeout(std::time::Duration::from_secs(60))
        .json(body);
    if !p.api_key.is_empty() {
        req = req.bearer_auth(&p.api_key);
    }
    req
}

/// Answer a question as a general assistant that can *optionally* consult the user's
/// recordings via the `search_recordings` tool. The model decides whether to search.
pub async fn chat(question: &str) -> Result<ChatReply, String> {
    let p = default_preset()?;
    if p.url.is_empty() || p.model.is_empty() {
        return Err("Default preset is missing a URL or model — check the Settings tab.".into());
    }
    if p.provider == "anthropic" {
        return Err("Chat via the Anthropic provider isn't wired yet — use a custom/openai/ollama preset (DeepSeek works).".into());
    }

    let system = "You are Meye, a helpful, knowledgeable personal assistant running locally on \
        the user's Mac. Answer naturally like a capable general-purpose AI. You can also search \
        the user's own screen/audio recordings with the search_recordings tool — use it only when \
        the question actually depends on their personal context (what they did, saw, heard, worked \
        on, or anything referring to themselves/their activity/time). For general questions, just \
        answer directly without searching.";

    let mut messages: Vec<Value> = vec![
        json!({"role": "system", "content": system}),
        json!({"role": "user", "content": question}),
    ];
    let tools = json!([search_tool()]);
    let mut sources: Vec<Source> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // Tool-use loop: let the model search as many times as it needs (capped), then answer.
    let mut answer: Option<String> = None;
    for round in 0..4 {
        // On the last round, stop offering the tool so the model must produce a final answer.
        let body = if round < 3 {
            json!({ "model": p.model, "messages": messages, "tools": tools, "tool_choice": "auto", "max_tokens": 1024 })
        } else {
            json!({ "model": p.model, "messages": messages, "max_tokens": 1024 })
        };
        let resp = post_body(&p, &body).send().await.map_err(|e| e.to_string())?;
        let v: Value = resp.json().await.map_err(|e| e.to_string())?;
        let msg = v
            .pointer("/choices/0/message")
            .cloned()
            .ok_or_else(|| format!("unexpected model response: {v}"))?;

        let tool_calls = msg.get("tool_calls").and_then(|t| t.as_array()).cloned().unwrap_or_default();
        if tool_calls.is_empty() {
            answer = Some(msg.get("content").and_then(|s| s.as_str()).unwrap_or("").to_string());
            break;
        }

        // Echo the assistant's tool-call message back, then answer each call with search results.
        messages.push(msg);
        for tc in &tool_calls {
            let id = tc.get("id").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let args = tc.pointer("/function/arguments").and_then(|s| s.as_str()).unwrap_or("{}");
            let query = serde_json::from_str::<Value>(args)
                .ok()
                .and_then(|a| a.get("query").and_then(|q| q.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| question.to_string());
            let snippets = run_search(&query, &mut sources, &mut seen).await;
            let content = if snippets.is_empty() {
                "(no matching recordings found)".to_string()
            } else {
                snippets
            };
            messages.push(json!({"role": "tool", "tool_call_id": id, "content": content}));
        }
    }

    Ok(ChatReply {
        answer: answer.filter(|a| !a.is_empty()).unwrap_or_else(|| "(no answer returned)".into()),
        sources,
    })
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
