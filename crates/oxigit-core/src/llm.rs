use serde::{Deserialize, Serialize};

use crate::error::{OxigitError, Result};

#[derive(Debug, Clone)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: Option<String>,
    pub model: String,
    pub base_url: Option<String>,
}

const SYSTEM_PROMPT: &str = "\
You are a code review assistant. Given a unified diff, provide a 2-4 sentence \
plain-English summary of WHAT changed and WHY (infer intent from the changes). \
Keep it concise and accessible. Do not include the diff itself in your response.";

const MAX_DIFF_CHARS: usize = 8000;

/// Generate a plain-English summary of a diff using an LLM.
pub async fn generate_summary(
    config: &LlmConfig,
    diff: &str,
    ai_prompt_context: Option<&str>,
) -> Result<String> {
    if config.provider == "none" {
        return Err(OxigitError::InvalidInput("LLM not configured".into()));
    }

    // Truncate diff if too large
    let diff_text = if diff.len() > MAX_DIFF_CHARS {
        format!(
            "{}...\n\n[diff truncated — showing first {} characters]",
            &diff[..MAX_DIFF_CHARS],
            MAX_DIFF_CHARS
        )
    } else {
        diff.to_string()
    };

    let mut user_message = String::new();
    if let Some(prompt) = ai_prompt_context {
        user_message.push_str(&format!(
            "The developer's original prompt/instruction was: \"{}\"\n\n",
            prompt
        ));
    }
    user_message.push_str("Here is the diff:\n\n");
    user_message.push_str(&diff_text);

    match config.provider.as_str() {
        "openai" => call_openai(config, &user_message).await,
        "anthropic" => call_anthropic(config, &user_message).await,
        "ollama" => call_ollama(config, &user_message).await,
        other => Err(OxigitError::InvalidInput(format!(
            "Unknown LLM provider: {}",
            other
        ))),
    }
}

/// Send a chat completion request and extract the response text.
async fn llm_request(
    url: &str,
    provider_name: &str,
    auth_header: Option<(&str, &str)>,
    extra_headers: &[(&str, &str)],
    body: &impl Serialize,
    extract_text: fn(&str) -> Result<String>,
) -> Result<String> {
    let client = reqwest::Client::new();
    let mut req = client.post(url).json(body);

    if let Some((key, value)) = auth_header {
        req = req.header(key, value);
    }
    for (key, value) in extra_headers {
        req = req.header(*key, *value);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| OxigitError::Llm(format!("{} request failed: {}", provider_name, e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(OxigitError::Llm(format!(
            "{} API error {}: {}",
            provider_name, status, text
        )));
    }

    let raw = resp.text().await.map_err(|e| {
        OxigitError::Llm(format!("Failed to read {} response: {}", provider_name, e))
    })?;

    extract_text(&raw)
}

// --- OpenAI ---

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: ChatMessage,
}

async fn call_openai(config: &LlmConfig, user_message: &str) -> Result<String> {
    let api_key = config
        .api_key
        .as_ref()
        .ok_or_else(|| OxigitError::InvalidInput("OpenAI API key required".into()))?;
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.openai.com");
    let url = format!("{}/v1/chat/completions", base_url);

    let body = OpenAiRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: SYSTEM_PROMPT.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_message.into(),
            },
        ],
        max_tokens: 512,
    };

    llm_request(
        &url,
        "OpenAI",
        Some(("Authorization", &format!("Bearer {}", api_key))),
        &[],
        &body,
        |raw| {
            let data: OpenAiResponse = serde_json::from_str(raw)
                .map_err(|e| OxigitError::Llm(format!("Failed to parse OpenAI response: {}", e)))?;
            data.choices
                .first()
                .map(|c| c.message.content.trim().to_string())
                .ok_or_else(|| OxigitError::Llm("Empty response from OpenAI".into()))
        },
    )
    .await
}

// --- Anthropic ---

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
}

#[derive(Deserialize)]
struct AnthropicContent {
    text: String,
}

async fn call_anthropic(config: &LlmConfig, user_message: &str) -> Result<String> {
    let api_key = config
        .api_key
        .as_ref()
        .ok_or_else(|| OxigitError::InvalidInput("Anthropic API key required".into()))?;
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("https://api.anthropic.com");
    let url = format!("{}/v1/messages", base_url);

    let body = AnthropicRequest {
        model: config.model.clone(),
        max_tokens: 512,
        system: SYSTEM_PROMPT.into(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: user_message.into(),
        }],
    };

    llm_request(
        &url,
        "Anthropic",
        Some(("x-api-key", api_key)),
        &[("anthropic-version", "2023-06-01")],
        &body,
        |raw| {
            let data: AnthropicResponse = serde_json::from_str(raw).map_err(|e| {
                OxigitError::Llm(format!("Failed to parse Anthropic response: {}", e))
            })?;
            data.content
                .first()
                .map(|c| c.text.trim().to_string())
                .ok_or_else(|| OxigitError::Llm("Empty response from Anthropic".into()))
        },
    )
    .await
}

// --- Ollama ---

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Deserialize)]
struct OllamaResponse {
    message: ChatMessage,
}

async fn call_ollama(config: &LlmConfig, user_message: &str) -> Result<String> {
    let base_url = config
        .base_url
        .as_deref()
        .unwrap_or("http://localhost:11434");
    let url = format!("{}/api/chat", base_url);

    let body = OllamaRequest {
        model: config.model.clone(),
        messages: vec![
            ChatMessage {
                role: "system".into(),
                content: SYSTEM_PROMPT.into(),
            },
            ChatMessage {
                role: "user".into(),
                content: user_message.into(),
            },
        ],
        stream: false,
    };

    llm_request(&url, "Ollama", None, &[], &body, |raw| {
        let data: OllamaResponse = serde_json::from_str(raw)
            .map_err(|e| OxigitError::Llm(format!("Failed to parse Ollama response: {}", e)))?;
        Ok(data.message.content.trim().to_string())
    })
    .await
}
