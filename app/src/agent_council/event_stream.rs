//! Per-CLI JSON event parsing.
//!
//! Each supported CLI emits a different newline-delimited event format. This
//! module normalises one stdout line at a time into a `CouncilEvent`. The
//! parser is **infallible** by design: any line we don't recognise becomes
//! `OutputDelta(line)` so the user never loses content.

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CouncilEvent {
    /// Reasoning chunk. Each chunk flips the persona's phase to Thinking
    /// and appends to the thinking buffer.
    ThinkingDelta(String),
    /// User-visible output chunk. Flips phase to Streaming and appends to
    /// the output buffer.
    OutputDelta(String),
    /// Compact, one-line summary of a tool invocation. Render as a chip.
    ToolCall {
        name: String,
        summary: String,
    },
    /// Stream finished. `ok=false` means the CLI reported an error or
    /// non-zero exit; populate `reason` with whatever short message we have.
    Finished {
        ok: bool,
        reason: Option<String>,
    },
}

/// Which CLI a stream came from. Decides how to interpret each line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind {
    Claude,
    Codex,
    /// Plain text fallback — every non-empty line is an `OutputDelta`.
    PlainText,
}

impl CliKind {
    /// Pick a parser for a binary by its basename. Returns `PlainText` when
    /// we don't have a structured mode wired up.
    pub fn from_basename(basename: &str) -> Self {
        match basename {
            "claude" => Self::Claude,
            "codex" => Self::Codex,
            _ => Self::PlainText,
        }
    }
}

/// Parse one line from the given CLI's stdout into 0..N events. Returns a
/// `Vec` because a single Claude `assistant` message can carry both a
/// thinking block and a text block.
pub fn parse_line(kind: CliKind, line: &str) -> Vec<CouncilEvent> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return Vec::new();
    }
    match kind {
        CliKind::Claude => parse_claude_line(line),
        CliKind::Codex => parse_codex_line(line),
        CliKind::PlainText => vec![CouncilEvent::OutputDelta(format!("{line}\n"))],
    }
}

fn parse_claude_line(line: &str) -> Vec<CouncilEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![CouncilEvent::OutputDelta(format!("{line}\n"))];
    };
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        "system" => Vec::new(),
        "assistant" => parse_claude_assistant(&v),
        "user" => Vec::new(), // tool_result echoes; ignore
        "result" => {
            let subtype = v.get("subtype").and_then(Value::as_str).unwrap_or("");
            let ok = subtype == "success";
            let reason = if ok {
                None
            } else {
                Some(format!("claude reported {subtype}"))
            };
            vec![CouncilEvent::Finished { ok, reason }]
        }
        _ => Vec::new(),
    }
}

fn parse_claude_assistant(v: &Value) -> Vec<CouncilEvent> {
    let Some(content) = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for block in content {
        let bt = block.get("type").and_then(Value::as_str).unwrap_or("");
        match bt {
            "thinking" => {
                if let Some(s) = block.get("thinking").and_then(Value::as_str) {
                    out.push(CouncilEvent::ThinkingDelta(s.to_owned()));
                }
            }
            "text" => {
                if let Some(s) = block.get("text").and_then(Value::as_str) {
                    out.push(CouncilEvent::OutputDelta(s.to_owned()));
                }
            }
            "tool_use" => {
                let name = block
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("tool")
                    .to_owned();
                let summary = block
                    .get("input")
                    .map(|i| i.to_string())
                    .unwrap_or_default();
                out.push(CouncilEvent::ToolCall { name, summary });
            }
            _ => {}
        }
    }
    out
}

fn parse_codex_line(line: &str) -> Vec<CouncilEvent> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return vec![CouncilEvent::OutputDelta(format!("{line}\n"))];
    };
    let kind = v.get("type").and_then(Value::as_str).unwrap_or("");
    match kind {
        // Codex's exact event schema isn't pinned here; keep this
        // permissive. Anything that looks like an assistant text delta
        // becomes OutputDelta; anything completion-shaped becomes Finished.
        s if s.contains("delta") || s.contains("message") => {
            let text = v
                .get("delta")
                .or_else(|| v.get("text"))
                .or_else(|| v.pointer("/item/content"))
                .and_then(Value::as_str)
                .unwrap_or("");
            if text.is_empty() {
                Vec::new()
            } else {
                vec![CouncilEvent::OutputDelta(text.to_owned())]
            }
        }
        s if s.contains("completed") || s.contains("done") || s == "task.completed" => {
            vec![CouncilEvent::Finished {
                ok: true,
                reason: None,
            }]
        }
        s if s.contains("error") || s.contains("failed") => {
            let msg = v
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned);
            vec![CouncilEvent::Finished {
                ok: false,
                reason: msg,
            }]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_thinking_and_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hmm"},{"type":"text","text":"hi"}]}}"#;
        let evs = parse_line(CliKind::Claude, line);
        assert_eq!(
            evs,
            vec![
                CouncilEvent::ThinkingDelta("hmm".into()),
                CouncilEvent::OutputDelta("hi".into()),
            ]
        );
    }

    #[test]
    fn claude_result_success() {
        let line = r#"{"type":"result","subtype":"success"}"#;
        let evs = parse_line(CliKind::Claude, line);
        assert_eq!(
            evs,
            vec![CouncilEvent::Finished {
                ok: true,
                reason: None
            }]
        );
    }

    #[test]
    fn claude_unknown_passthrough() {
        let line = "not json";
        let evs = parse_line(CliKind::Claude, line);
        assert_eq!(evs, vec![CouncilEvent::OutputDelta("not json\n".into())]);
    }

    #[test]
    fn plain_text_lines() {
        assert_eq!(
            parse_line(CliKind::PlainText, "hello\n"),
            vec![CouncilEvent::OutputDelta("hello\n".into())]
        );
        assert_eq!(parse_line(CliKind::PlainText, ""), vec![]);
    }

}
