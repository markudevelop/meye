//! Voice-command parsing for Meye. Pure + unit-tested. The frontend feeds in the latest
//! local mic transcript (already produced by screenpipe's whisper); this turns a recognised
//! "wake phrase + command" into a structured action the UI dispatches. Everything stays local.
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VoiceCommand {
    /// "pause" | "resume" | "new-chat" | "open" | "search"
    pub action: String,
    /// argument for "open" (tab id) and "search" (query); empty otherwise.
    pub arg: String,
}

/// Wake phrases. Includes common whisper mis-hearings of "Meye" (my / may / me / mai).
const WAKES: &[&str] = &[
    "hey meye", "hey mai", "hey may", "hey my", "hey me", "hey mei", "ok meye", "okay meye",
    "hey computer", "okay computer",
];

/// Canonical tab ids the "open" command can target, matched by substring.
const TABS: &[&str] = &[
    "home", "chat", "status", "search", "timeline", "devices", "pipes", "performance", "settings",
];

fn strip_edges(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphanumeric())
}

/// Parse a transcript into a command, or None if there's no wake phrase + recognised command.
pub fn parse_voice_command(transcript: &str) -> Option<VoiceCommand> {
    let lower = transcript.to_lowercase();

    // Require a wake phrase. Take what follows the earliest one — and when several wakes match
    // at the same spot (e.g. "hey me" is a prefix of "hey meye"), consume the longest so we
    // don't leave stray letters in front of the command.
    let mut best: Option<(usize, usize)> = None; // (start, end)
    for w in WAKES {
        if let Some(s) = lower.find(w) {
            let e = s + w.len();
            best = match best {
                Some((bs, be)) if bs < s || (bs == s && be >= e) => Some((bs, be)),
                _ => Some((s, e)),
            };
        }
    }
    let (_, end) = best?;
    let rest = strip_edges(lower[end..].trim());
    if rest.is_empty() {
        return None;
    }

    let cmd = |action: &str, arg: &str| {
        Some(VoiceCommand { action: action.into(), arg: arg.into() })
    };

    // pause / resume
    if rest.starts_with("pause") || rest.starts_with("stop") {
        return cmd("pause", "");
    }
    if rest.starts_with("resume") || rest.starts_with("continue") || rest.starts_with("unpause") {
        return cmd("resume", "");
    }

    // new chat
    if rest.starts_with("new chat") || rest.starts_with("new conversation") || rest.starts_with("start chat") {
        return cmd("new-chat", "");
    }

    // open <tab>: "open timeline", "go to settings", "show pipes", "switch to search"
    for verb in ["open ", "go to ", "show ", "switch to ", "navigate to "] {
        if let Some(target) = rest.strip_prefix(verb) {
            let target = strip_edges(target);
            // map synonyms then match a known tab by substring
            let target = if target.contains("setting") {
                "settings"
            } else if target.contains("time") {
                "timeline"
            } else if target.contains("device") {
                "devices"
            } else if target.contains("pipe") || target.contains("schedule") {
                "pipes"
            } else if target.contains("perf") {
                "performance"
            } else if target.contains("chat") {
                "home"
            } else {
                target
            };
            if let Some(tab) = TABS.iter().find(|t| target.contains(**t)) {
                return cmd("open", tab);
            }
        }
    }

    // search <query>: "search for cats", "search my recordings for X", "find the pdf", "look for Y"
    for verb in ["search for ", "search my recordings for ", "search ", "find ", "look for "] {
        if let Some(q) = rest.strip_prefix(verb) {
            let q = strip_edges(q);
            if !q.is_empty() {
                return cmd("search", q);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Option<VoiceCommand> {
        parse_voice_command(s)
    }

    #[test]
    fn requires_a_wake_phrase() {
        assert_eq!(parse("pause recording"), None);
        assert_eq!(parse("just talking about my new chat with a friend"), None);
    }

    #[test]
    fn pause_and_resume() {
        assert_eq!(parse("hey meye pause").unwrap().action, "pause");
        assert_eq!(parse("Hey Meye, stop recording.").unwrap().action, "pause");
        assert_eq!(parse("hey computer resume").unwrap().action, "resume");
        assert_eq!(parse("okay meye continue").unwrap().action, "resume");
    }

    #[test]
    fn new_chat() {
        assert_eq!(parse("hey meye new chat").unwrap().action, "new-chat");
        assert_eq!(parse("hey my, new conversation").unwrap().action, "new-chat");
    }

    #[test]
    fn open_tab_with_synonyms() {
        assert_eq!(parse("hey meye open timeline").unwrap(), VoiceCommand { action: "open".into(), arg: "timeline".into() });
        assert_eq!(parse("hey meye go to settings").unwrap().arg, "settings");
        assert_eq!(parse("hey meye show pipes").unwrap().arg, "pipes");
        assert_eq!(parse("hey meye switch to chat").unwrap().arg, "home");
        // mis-heard wake word still works
        assert_eq!(parse("hey may open performance").unwrap().arg, "performance");
    }

    #[test]
    fn search_variants() {
        assert_eq!(parse("hey meye search for invoices").unwrap(), VoiceCommand { action: "search".into(), arg: "invoices".into() });
        assert_eq!(parse("hey meye find the pdf").unwrap().arg, "the pdf");
        assert_eq!(parse("hey meye search my recordings for python errors").unwrap().arg, "python errors");
    }

    #[test]
    fn wake_without_command_is_none() {
        assert_eq!(parse("hey meye"), None);
        assert_eq!(parse("hey meye uhh"), None);
    }
}
