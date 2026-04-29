//! LLM provider for warp-oss inference paths.
//!
//! Backend selection is purely env-var driven for v1; a settings UI can
//! supersede this later by writing to the same env vars before app boot,
//! or by storing keys via `crates/ai/src/api_keys.rs`.
//!
//! Provider selection (`WARP_OSS_LLM_PROVIDER`):
//!   - `anthropic`  → POSTs to `https://api.anthropic.com/v1/messages`
//!   - `openai`     → POSTs to `${WARP_OSS_LLM_BASE_URL:-https://api.openai.com/v1}/chat/completions`
//!                    (also covers LM Studio, vLLM, LiteLLM, OpenRouter)
//!   - `ollama`     → POSTs to `${WARP_OSS_LLM_BASE_URL:-http://localhost:11434}/api/chat`
//!
//! Auto-detection if `WARP_OSS_LLM_PROVIDER` is unset:
//!   - `ANTHROPIC_API_KEY` set → anthropic
//!   - `OPENAI_API_KEY` set → openai
//!   - else → ollama (no key required)
//!
//! Other env vars:
//!   - `WARP_OSS_LLM_API_KEY` (overrides `ANTHROPIC_API_KEY` / `OPENAI_API_KEY`)
//!   - `WARP_OSS_LLM_MODEL`   (provider-default if unset)

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

const ANTHROPIC_DEFAULT_MODEL: &str = "claude-opus-4-7";
const OPENAI_DEFAULT_MODEL: &str = "gpt-5";
const OLLAMA_DEFAULT_MODEL: &str = "qwen2.5-coder";

#[derive(Clone, Debug)]
pub enum LocalLlmProvider {
    Anthropic { api_key: String, model: String },
    OpenAi { base_url: String, api_key: String, model: String },
    Ollama { base_url: String, model: String },
    /// No provider configured. Inference methods return an actionable error
    /// pointing the user at the env vars they need to set.
    Disabled,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl LocalLlmProvider {
    pub fn from_env() -> Self {
        let provider = std::env::var("WARP_OSS_LLM_PROVIDER").ok();
        let key_override = std::env::var("WARP_OSS_LLM_API_KEY").ok();
        let model_override = std::env::var("WARP_OSS_LLM_MODEL").ok();
        let base_url_override = std::env::var("WARP_OSS_LLM_BASE_URL").ok();

        let kind = provider
            .as_deref()
            .map(str::trim)
            .map(str::to_lowercase)
            .unwrap_or_else(|| {
                if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                    "anthropic".into()
                } else if std::env::var("OPENAI_API_KEY").is_ok() {
                    "openai".into()
                } else {
                    "ollama".into()
                }
            });

        match kind.as_str() {
            "anthropic" => {
                let Some(api_key) = key_override
                    .clone()
                    .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
                else {
                    return LocalLlmProvider::Disabled;
                };
                let model = model_override.unwrap_or_else(|| ANTHROPIC_DEFAULT_MODEL.to_string());
                LocalLlmProvider::Anthropic { api_key, model }
            }
            "openai" => {
                let Some(api_key) = key_override
                    .clone()
                    .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                else {
                    return LocalLlmProvider::Disabled;
                };
                let base_url =
                    base_url_override.unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let model = model_override.unwrap_or_else(|| OPENAI_DEFAULT_MODEL.to_string());
                LocalLlmProvider::OpenAi {
                    base_url,
                    api_key,
                    model,
                }
            }
            "ollama" => {
                let base_url =
                    base_url_override.unwrap_or_else(|| "http://localhost:11434".to_string());
                let model = model_override.unwrap_or_else(|| OLLAMA_DEFAULT_MODEL.to_string());
                LocalLlmProvider::Ollama { base_url, model }
            }
            other => {
                log::warn!(
                    "warp-oss: unknown WARP_OSS_LLM_PROVIDER value {other:?}; LLM disabled"
                );
                LocalLlmProvider::Disabled
            }
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            LocalLlmProvider::Anthropic { .. } => "anthropic",
            LocalLlmProvider::OpenAi { .. } => "openai",
            LocalLlmProvider::Ollama { .. } => "ollama",
            LocalLlmProvider::Disabled => "disabled",
        }
    }

    pub fn model(&self) -> &str {
        match self {
            LocalLlmProvider::Anthropic { model, .. }
            | LocalLlmProvider::OpenAi { model, .. }
            | LocalLlmProvider::Ollama { model, .. } => model.as_str(),
            LocalLlmProvider::Disabled => "disabled",
        }
    }

    /// Send a chat completion. Streaming is buffered into a single response
    /// since the upstream `AIClient` trait methods are non-streaming.
    pub async fn chat(&self, system: Option<&str>, messages: Vec<Message>) -> Result<String> {
        match self {
            LocalLlmProvider::Anthropic { api_key, model } => {
                anthropic_chat(api_key, model, system, messages).await
            }
            LocalLlmProvider::OpenAi {
                base_url,
                api_key,
                model,
            } => openai_chat(base_url, api_key, model, system, messages).await,
            LocalLlmProvider::Ollama { base_url, model } => {
                ollama_chat(base_url, model, system, messages).await
            }
            LocalLlmProvider::Disabled => Err(anyhow!(
                "warp-oss: no LLM provider configured. Set WARP_OSS_LLM_PROVIDER \
                 (anthropic / openai / ollama) and the corresponding key/base URL."
            )),
        }
    }
}

// ---- Anthropic ---------------------------------------------------------

#[derive(Serialize)]
struct AnthropicRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<AnthropicMessage<'a>>,
}

#[derive(Serialize)]
struct AnthropicMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

async fn anthropic_chat(
    api_key: &str,
    model: &str,
    system: Option<&str>,
    messages: Vec<Message>,
) -> Result<String> {
    let messages: Vec<AnthropicMessage> = messages
        .iter()
        .map(|m| AnthropicMessage {
            role: match m.role {
                Role::System => "user", // Anthropic system goes in the top-level field
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: m.content.as_str(),
        })
        .collect();
    let body = AnthropicRequest {
        model,
        max_tokens: 4096,
        system,
        messages,
    };
    let response = reqwest::Client::new()
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .context("anthropic: request failed")?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("anthropic: read body failed")?;
    if !status.is_success() {
        return Err(anyhow!("anthropic: HTTP {status}: {text}"));
    }
    let parsed: AnthropicResponse = serde_json::from_str(&text)
        .with_context(|| format!("anthropic: parse response failed: {text}"))?;
    Ok(parsed
        .content
        .into_iter()
        .filter(|c| c.kind == "text")
        .map(|c| c.text)
        .collect::<Vec<_>>()
        .join("\n"))
}

// ---- OpenAI-compatible -------------------------------------------------

#[derive(Serialize)]
struct OpenAiRequest<'a> {
    model: &'a str,
    messages: Vec<OpenAiMessage<'a>>,
}

#[derive(Serialize)]
struct OpenAiMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
}

async fn openai_chat(
    base_url: &str,
    api_key: &str,
    model: &str,
    system: Option<&str>,
    messages: Vec<Message>,
) -> Result<String> {
    let mut combined: Vec<OpenAiMessage> = Vec::new();
    if let Some(system) = system {
        combined.push(OpenAiMessage {
            role: "system",
            content: system,
        });
    }
    for m in &messages {
        combined.push(OpenAiMessage {
            role: match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: m.content.as_str(),
        });
    }
    let body = OpenAiRequest {
        model,
        messages: combined,
    };
    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .context("openai: request failed")?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("openai: read body failed")?;
    if !status.is_success() {
        return Err(anyhow!("openai: HTTP {status}: {text}"));
    }
    let parsed: OpenAiResponse = serde_json::from_str(&text)
        .with_context(|| format!("openai: parse response failed: {text}"))?;
    Ok(parsed
        .choices
        .into_iter()
        .next()
        .and_then(|c| c.message.content)
        .unwrap_or_default())
}

// ---- Ollama ------------------------------------------------------------

#[derive(Serialize)]
struct OllamaRequest<'a> {
    model: &'a str,
    messages: Vec<OllamaMessage<'a>>,
    stream: bool,
}

#[derive(Serialize)]
struct OllamaMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: OllamaResponseMessage,
}

#[derive(Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

async fn ollama_chat(
    base_url: &str,
    model: &str,
    system: Option<&str>,
    messages: Vec<Message>,
) -> Result<String> {
    let mut combined: Vec<OllamaMessage> = Vec::new();
    if let Some(system) = system {
        combined.push(OllamaMessage {
            role: "system",
            content: system,
        });
    }
    for m in &messages {
        combined.push(OllamaMessage {
            role: match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: m.content.as_str(),
        });
    }
    let body = OllamaRequest {
        model,
        messages: combined,
        stream: false,
    };
    let url = format!("{}/api/chat", base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(&url)
        .json(&body)
        .send()
        .await
        .context("ollama: request failed")?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("ollama: read body failed")?;
    if !status.is_success() {
        return Err(anyhow!("ollama: HTTP {status}: {text}"));
    }
    let parsed: OllamaResponse = serde_json::from_str(&text)
        .with_context(|| format!("ollama: parse response failed: {text}"))?;
    Ok(parsed.message.content)
}
