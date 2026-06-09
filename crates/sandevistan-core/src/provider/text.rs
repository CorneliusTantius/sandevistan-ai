use crate::{ChatMessage, ProviderConfig};
use serde::{Deserialize, Serialize};

use super::{http::post_json, parser::extract_response_text};

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: AssistantMessage,
}

#[derive(Debug, Deserialize)]
struct AssistantMessage {
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct ResponsesRequest {
    model: String,
    input: Vec<ResponseInputMessage>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ResponseInputMessage {
    role: String,
    content: String,
}


pub async fn complete(
    runtime: ProviderConfig,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    match runtime.kind.as_str() {
        "openai-responses" => send_responses(runtime, messages).await,
        _ => send_chat_completions(runtime, messages).await,
    }
}

async fn send_chat_completions(
    runtime: ProviderConfig,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let request = ChatRequest {
        model: runtime.model_id.clone(),
        messages,
        stream: false,
    };
    let body = post_json(&runtime, "chat/completions", &request).await?;
    let response = serde_json::from_str::<ChatResponse>(&body)
        .map_err(|error| format!("chat response parse failed: {error}"))?;

    response
        .choices
        .into_iter()
        .find_map(|choice| choice.message.content)
        .map(|content| content.trim().to_owned())
        .filter(|content| !content.is_empty())
        .ok_or_else(|| "chat response had no assistant content".into())
}

async fn send_responses(
    runtime: ProviderConfig,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    let request = ResponsesRequest {
        model: runtime.model_id.clone(),
        input: response_input(messages),
        stream: false,
    };
    let body = post_json(&runtime, "responses", &request).await?;
    extract_response_text(&body).ok_or_else(|| "responses response had no assistant content".into())
}

fn response_input(messages: Vec<ChatMessage>) -> Vec<ResponseInputMessage> {
    messages
        .into_iter()
        .filter(|message| {
            message.role == "user" || message.role == "assistant" || message.role == "system"
        })
        .map(|message| ResponseInputMessage {
            role: message.role,
            content: message.content,
        })
        .collect()
}
