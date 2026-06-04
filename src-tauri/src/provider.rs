use crate::ai::ChatMessage;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::OnceLock, time::Duration};

#[derive(Debug, Clone)]
pub struct ProviderRuntime {
    pub kind: String,
    pub api_base: String,
    pub api_key: Option<String>,
    pub api_key_header: String,
    pub model_id: String,
}

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

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: Option<ApiErrorBody>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: Option<String>,
}

const API_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const API_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const API_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(90);
const API_TCP_KEEPALIVE: Duration = Duration::from_secs(60);
const API_RETRY_ATTEMPTS: usize = 2;

pub async fn complete(
    runtime: ProviderRuntime,
    messages: Vec<ChatMessage>,
) -> Result<String, String> {
    match runtime.kind.as_str() {
        "openai-responses" => send_responses(runtime, messages).await,
        _ => send_chat_completions(runtime, messages).await,
    }
}

pub async fn complete_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<ChatMessage>,
    on_delta: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    match runtime.kind.as_str() {
        "openai-responses" => send_responses_stream(runtime, messages, on_delta).await,
        _ => send_chat_completions_stream(runtime, messages, on_delta).await,
    }
}

async fn send_chat_completions(
    runtime: ProviderRuntime,
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

async fn send_chat_completions_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<ChatMessage>,
    on_delta: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let request = ChatRequest {
        model: runtime.model_id.clone(),
        messages,
        stream: true,
    };
    send_stream(&runtime, "chat/completions", &request, on_delta).await
}

async fn send_responses(
    runtime: ProviderRuntime,
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

async fn send_responses_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<ChatMessage>,
    on_delta: F,
) -> Result<String, String>
where
    F: FnMut(String),
{
    let request = ResponsesRequest {
        model: runtime.model_id.clone(),
        input: response_input(messages),
        stream: true,
    };
    send_stream(&runtime, "responses", &request, on_delta).await
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

async fn post_json<T: Serialize>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
) -> Result<String, String> {
    let mut last_error = String::new();
    for attempt in 1..=API_RETRY_ATTEMPTS {
        match send_json_once(runtime, path, request).await {
            Ok(body) => return Ok(body),
            Err(error) if attempt < API_RETRY_ATTEMPTS && error.retryable => {
                last_error = error.message;
                retry_delay(attempt).await;
            }
            Err(error) => return Err(error.message),
        }
    }
    Err(last_error)
}

async fn send_json_once<T: Serialize>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
) -> Result<String, ApiRequestError> {
    let response = tokio::time::timeout(
        API_REQUEST_TIMEOUT,
        request_builder(runtime, path, request).send(),
    )
    .await
    .map_err(|_| ApiRequestError::retryable("api request timed out"))?
    .map_err(|error| ApiRequestError::from_reqwest("api request failed", error))?;

    let status = response.status();
    let body = tokio::time::timeout(API_REQUEST_TIMEOUT, response.text())
        .await
        .map_err(|_| ApiRequestError::retryable("api response read timed out"))?
        .map_err(|error| ApiRequestError::from_reqwest("api response read failed", error))?;

    if !status.is_success() {
        return Err(ApiRequestError {
            message: api_error(status, body),
            retryable: is_retryable_status(status),
        });
    }

    Ok(body)
}

async fn send_stream<T, F>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
    mut on_delta: F,
) -> Result<String, String>
where
    T: Serialize,
    F: FnMut(String),
{
    let mut response = open_stream_with_retry(runtime, path, request).await?;

    let mut pending_bytes = Vec::new();
    let mut pending = String::new();
    let mut output = String::new();
    while let Some(chunk) = tokio::time::timeout(API_STREAM_IDLE_TIMEOUT, response.chunk())
        .await
        .map_err(|_| "api stream read timed out".to_string())?
        .map_err(|error| format!("api stream read failed: {error}"))?
    {
        pending_bytes.extend_from_slice(&chunk);
        flush_utf8_prefix(&mut pending_bytes, &mut pending)?;
        normalize_newlines(&mut pending);
        while let Some(event) = next_sse_event(&mut pending) {
            handle_sse_event(&event, &mut output, &mut on_delta)?;
        }
    }

    if !pending_bytes.is_empty() {
        flush_utf8_prefix(&mut pending_bytes, &mut pending)?;
        if !pending_bytes.is_empty() {
            return Err("api stream ended with incomplete utf-8".into());
        }
    }
    if !pending.trim().is_empty() {
        handle_sse_event(&pending, &mut output, &mut on_delta)?;
    }

    let content = output.trim().to_owned();
    if content.is_empty() {
        return Err("stream response had no assistant content".into());
    }
    Ok(content)
}

fn request_builder<T: Serialize>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
) -> reqwest::RequestBuilder {
    let endpoint = format!("{}/{}", runtime.api_base.trim_end_matches('/'), path);
    let mut builder = http_client().post(endpoint).json(request);

    if let Some(key) = &runtime.api_key {
        if runtime.api_key_header == "api-key" {
            builder = builder.header("api-key", key);
        } else {
            builder = builder.bearer_auth(key);
        }
    }

    builder
}

async fn open_stream_with_retry<T: Serialize>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
) -> Result<reqwest::Response, String> {
    let mut last_error = String::new();
    for attempt in 1..=API_RETRY_ATTEMPTS {
        match open_stream_once(runtime, path, request).await {
            Ok(response) => return Ok(response),
            Err(error) if attempt < API_RETRY_ATTEMPTS && error.retryable => {
                last_error = error.message;
                retry_delay(attempt).await;
            }
            Err(error) => return Err(error.message),
        }
    }
    Err(last_error)
}

async fn open_stream_once<T: Serialize>(
    runtime: &ProviderRuntime,
    path: &str,
    request: &T,
) -> Result<reqwest::Response, ApiRequestError> {
    let response = tokio::time::timeout(
        API_REQUEST_TIMEOUT,
        request_builder(runtime, path, request).send(),
    )
    .await
    .map_err(|_| ApiRequestError::retryable("api request timed out"))?
    .map_err(|error| ApiRequestError::from_reqwest("api request failed", error))?;

    let status = response.status();
    if !status.is_success() {
        let body = tokio::time::timeout(API_REQUEST_TIMEOUT, response.text())
            .await
            .map_err(|_| ApiRequestError::retryable("api response read timed out"))?
            .map_err(|error| ApiRequestError::from_reqwest("api response read failed", error))?;
        return Err(ApiRequestError {
            message: api_error(status, body),
            retryable: is_retryable_status(status),
        });
    }

    Ok(response)
}

#[derive(Debug)]
struct ApiRequestError {
    message: String,
    retryable: bool,
}

impl ApiRequestError {
    fn retryable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retryable: true,
        }
    }

    fn from_reqwest(prefix: &str, error: reqwest::Error) -> Self {
        Self {
            retryable: error.is_timeout() || error.is_connect(),
            message: format!("{prefix}: {error}"),
        }
    }
}

fn is_retryable_status(status: reqwest::StatusCode) -> bool {
    status == reqwest::StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

async fn retry_delay(attempt: usize) {
    tokio::time::sleep(Duration::from_millis(750 * attempt as u64)).await;
}

fn api_error(status: reqwest::StatusCode, body: String) -> String {
    let message = serde_json::from_str::<ApiErrorResponse>(&body)
        .ok()
        .and_then(|value| value.error.and_then(|error| error.message))
        .unwrap_or(body);
    format!("api error {status}: {message}")
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(API_CONNECT_TIMEOUT)
            .pool_max_idle_per_host(8)
            .pool_idle_timeout(API_POOL_IDLE_TIMEOUT)
            .tcp_keepalive(Some(API_TCP_KEEPALIVE))
            .tcp_nodelay(true)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

fn flush_utf8_prefix(bytes: &mut Vec<u8>, output: &mut String) -> Result<(), String> {
    match std::str::from_utf8(bytes) {
        Ok(text) => {
            output.push_str(text);
            bytes.clear();
            Ok(())
        }
        Err(error) if error.error_len().is_none() => {
            let valid = error.valid_up_to();
            if valid > 0 {
                let text = std::str::from_utf8(&bytes[..valid])
                    .map_err(|error| format!("api stream utf-8 decode failed: {error}"))?;
                output.push_str(text);
                bytes.drain(..valid);
            }
            Ok(())
        }
        Err(error) => Err(format!("api stream utf-8 decode failed: {error}")),
    }
}

fn normalize_newlines(value: &mut String) {
    if value.contains('\r') {
        *value = value.replace("\r\n", "\n").replace('\r', "\n");
    }
}

fn next_sse_event(pending: &mut String) -> Option<String> {
    let index = pending.find("\n\n")?;
    let event = pending[..index].to_string();
    pending.drain(..index + 2);
    Some(event)
}

fn handle_sse_event<F>(event: &str, output: &mut String, on_delta: &mut F) -> Result<(), String>
where
    F: FnMut(String),
{
    for data in event_data(event) {
        if data == "[DONE]" {
            continue;
        }
        let value = serde_json::from_str::<Value>(&data)
            .map_err(|error| format!("stream response parse failed: {error}"))?;
        if let Some(delta) = extract_stream_delta(&value) {
            output.push_str(&delta);
            on_delta(delta);
        }
    }
    Ok(())
}

fn event_data(event: &str) -> Vec<String> {
    let mut values = Vec::new();
    let mut current = Vec::new();
    for line in event.lines() {
        let line = line.trim_start();
        if let Some(data) = line.strip_prefix("data:") {
            current.push(data.trim_start().to_string());
        }
    }
    if !current.is_empty() {
        values.push(current.join("\n"));
    }
    values
}

fn extract_stream_delta(value: &Value) -> Option<String> {
    if let Some(choices) = value.get("choices").and_then(Value::as_array) {
        let mut parts = Vec::new();
        for choice in choices {
            if let Some(content) = choice
                .get("delta")
                .and_then(|delta| delta.get("content"))
                .and_then(Value::as_str)
            {
                parts.push(content.to_string());
            }
        }
        return (!parts.is_empty()).then(|| parts.join(""));
    }

    if value.get("type").and_then(Value::as_str) == Some("response.output_text.delta") {
        return value
            .get("delta")
            .and_then(Value::as_str)
            .map(str::to_string);
    }

    value
        .get("delta")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn extract_response_text(body: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(body).ok()?;
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        return clean(text);
    }

    let mut parts = Vec::new();
    for item in value.get("output")?.as_array()? {
        if item.get("type").and_then(Value::as_str) == Some("message") {
            if let Some(content) = item.get("content").and_then(Value::as_array) {
                for part in content {
                    if let Some(text) = part.get("text").and_then(Value::as_str) {
                        parts.push(text.to_string());
                    }
                }
            }
        }
    }
    clean(&parts.join("\n"))
}

fn clean(value: &str) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}
