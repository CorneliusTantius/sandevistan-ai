use crate::{
    ai::ChatMessage,
    runtime::CancellationToken,
    runtime_wire::{
        NativeMessage, NativeStreamEvent, NativeToolCall, NativeToolSpec, NativeTurnResult,
    },
    tools,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
    time::Duration,
};

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

const API_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);
const API_STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(45);
const API_POOL_IDLE_TIMEOUT: Duration = Duration::from_secs(180);
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

pub async fn complete_native_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<NativeMessage>,
    tool_specs: Vec<NativeToolSpec>,
    cancellation_token: CancellationToken,
    on_event: F,
) -> Result<NativeTurnResult, String>
where
    F: FnMut(NativeStreamEvent),
{
    ProviderAdapter::from_runtime(runtime)
        .stream_turn(messages, tool_specs, cancellation_token, on_event)
        .await
}

enum ProviderAdapter {
    OpenAiChat(ProviderRuntime),
    OpenAiResponses(ProviderRuntime),
}

impl ProviderAdapter {
    fn from_runtime(runtime: ProviderRuntime) -> Self {
        match runtime.kind.as_str() {
            "openai-responses" => Self::OpenAiResponses(runtime),
            _ => Self::OpenAiChat(runtime),
        }
    }

    async fn stream_turn<F>(
        self,
        messages: Vec<NativeMessage>,
        tool_specs: Vec<NativeToolSpec>,
        cancellation_token: CancellationToken,
        on_event: F,
    ) -> Result<NativeTurnResult, String>
    where
        F: FnMut(NativeStreamEvent),
    {
        match self {
            Self::OpenAiChat(runtime) => {
                send_chat_native_stream(runtime, messages, tool_specs, cancellation_token, on_event)
                    .await
            }
            Self::OpenAiResponses(runtime) => {
                send_responses_native_stream(
                    runtime,
                    messages,
                    tool_specs,
                    cancellation_token,
                    on_event,
                )
                .await
            }
        }
    }
}

async fn send_chat_native_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<NativeMessage>,
    tool_specs: Vec<NativeToolSpec>,
    cancellation_token: CancellationToken,
    on_event: F,
) -> Result<NativeTurnResult, String>
where
    F: FnMut(NativeStreamEvent),
{
    let mut body = json!({
        "model": runtime.model_id,
        "messages": chat_native_messages(messages),
        "stream": true,
    });
    if !tool_specs.is_empty() {
        body["tools"] = Value::Array(
            tool_specs
                .into_iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "function": {
                            "name": tool.openai_name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    })
                })
                .collect(),
        );
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(true);
    }
    send_native_stream_once(
        runtime,
        "chat/completions",
        body,
        NativeParser::Chat(ChatAccumulator::default()),
        cancellation_token,
        on_event,
    )
    .await
}

async fn send_responses_native_stream<F>(
    runtime: ProviderRuntime,
    messages: Vec<NativeMessage>,
    tool_specs: Vec<NativeToolSpec>,
    cancellation_token: CancellationToken,
    on_event: F,
) -> Result<NativeTurnResult, String>
where
    F: FnMut(NativeStreamEvent),
{
    let mut body = json!({
        "model": runtime.model_id,
        "input": responses_native_input(messages),
        "stream": true,
    });
    if !tool_specs.is_empty() {
        body["tools"] = Value::Array(
            tool_specs
                .into_iter()
                .map(|tool| {
                    json!({
                        "type": "function",
                        "name": tool.openai_name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    })
                })
                .collect(),
        );
        body["tool_choice"] = json!("auto");
        body["parallel_tool_calls"] = json!(true);
    }
    send_native_stream_once(
        runtime,
        "responses",
        body,
        NativeParser::Responses(ResponsesAccumulator::default()),
        cancellation_token,
        on_event,
    )
    .await
}

async fn send_native_stream_once<F>(
    runtime: ProviderRuntime,
    path: &str,
    body: Value,
    mut parser: NativeParser,
    cancellation_token: CancellationToken,
    mut on_event: F,
) -> Result<NativeTurnResult, String>
where
    F: FnMut(NativeStreamEvent),
{
    let mut response = open_stream_with_retry(&runtime, path, &body).await?;
    let mut pending_bytes = Vec::new();
    let mut pending = String::new();
    loop {
        let chunk = tokio::select! {
            _ = cancellation_token.cancelled() => return Err("cancelled".into()),
            result = tokio::time::timeout(API_STREAM_IDLE_TIMEOUT, response.chunk()) => match result {
                Ok(Ok(Some(chunk))) => chunk,
                Ok(Ok(None)) => break,
                Ok(Err(error)) => return Err(format!("api stream read failed: {error}")),
                Err(_) => return Err("api stream read timed out".into()),
            },
        };
        pending_bytes.extend_from_slice(&chunk);
        flush_utf8_prefix(&mut pending_bytes, &mut pending)?;
        normalize_newlines(&mut pending);
        while let Some(event) = next_sse_event(&mut pending) {
            parse_native_sse_event(&event, &mut parser, &mut on_event)?;
        }
    }
    if !pending_bytes.is_empty() {
        flush_utf8_prefix(&mut pending_bytes, &mut pending)?;
    }
    if !pending.trim().is_empty() {
        parse_native_sse_event(&pending, &mut parser, &mut on_event)?;
    }
    Ok(parser.finish())
}

fn parse_native_sse_event<F>(
    event: &str,
    parser: &mut NativeParser,
    on_event: &mut F,
) -> Result<(), String>
where
    F: FnMut(NativeStreamEvent),
{
    for data in event_data(event) {
        if data == "[DONE]" {
            continue;
        }
        let value = serde_json::from_str::<Value>(&data)
            .map_err(|error| format!("native stream response parse failed: {error}"))?;
        parser.push(value, on_event)?;
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

fn chat_native_messages(messages: Vec<NativeMessage>) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        match message {
            NativeMessage::System { content } => {
                out.push(json!({"role":"system","content":content}))
            }
            NativeMessage::User { content } => out.push(json!({"role":"user","content":content})),
            NativeMessage::Assistant {
                content,
                tool_calls,
            } => {
                let mut value = json!({"role":"assistant","content": if content.is_empty() { Value::Null } else { Value::String(content) }});
                if !tool_calls.is_empty() {
                    value["tool_calls"] = Value::Array(tool_calls.into_iter().map(|call| json!({
                        "id": call.id,
                        "type": "function",
                        "function": {"name": call.openai_name, "arguments": call.args.to_string()}
                    })).collect());
                }
                out.push(value);
            }
            NativeMessage::ToolResult {
                tool_call_id,
                content,
                ..
            } => out.push(json!({"role":"tool","tool_call_id":tool_call_id,"content":content})),
        }
    }
    out
}

fn responses_native_input(messages: Vec<NativeMessage>) -> Vec<Value> {
    let mut out = Vec::new();
    for message in messages {
        match message {
            NativeMessage::System { content } => {
                out.push(json!({"role":"system","content":content}))
            }
            NativeMessage::User { content } => out.push(json!({"role":"user","content":content})),
            NativeMessage::Assistant {
                content,
                tool_calls,
            } => {
                if !content.trim().is_empty() {
                    out.push(json!({"role":"assistant","content":content}));
                }
                for call in tool_calls {
                    out.push(json!({
                        "type": "function_call",
                        "call_id": call.id,
                        "name": call.openai_name,
                        "arguments": call.args.to_string(),
                    }));
                }
            }
            NativeMessage::ToolResult {
                tool_call_id,
                content,
                ..
            } => out.push(
                json!({"type":"function_call_output","call_id":tool_call_id,"output":content}),
            ),
        }
    }
    out
}

#[derive(Clone, Default)]
struct ToolCallParts {
    id: String,
    name: String,
    args: String,
}

#[derive(Default)]
struct ChatAccumulator {
    content: String,
    calls: HashMap<usize, ToolCallParts>,
}

#[derive(Default)]
struct ResponsesAccumulator {
    content: String,
    calls: Vec<NativeToolCall>,
    pending_calls: HashMap<String, ToolCallParts>,
    finalized_calls: HashSet<String>,
}

enum NativeParser {
    Chat(ChatAccumulator),
    Responses(ResponsesAccumulator),
}

impl NativeParser {
    fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
    where
        F: FnMut(NativeStreamEvent),
    {
        match self {
            NativeParser::Chat(acc) => acc.push(value, on_event),
            NativeParser::Responses(acc) => acc.push(value, on_event),
        }
    }

    fn finish(self) -> NativeTurnResult {
        match self {
            NativeParser::Chat(acc) => acc.finish(),
            NativeParser::Responses(acc) => acc.finish(),
        }
    }
}

impl ChatAccumulator {
    fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
    where
        F: FnMut(NativeStreamEvent),
    {
        let Some(choices) = value.get("choices").and_then(Value::as_array) else {
            return Ok(());
        };
        for choice in choices {
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(content) = delta.get("content").and_then(Value::as_str) {
                self.content.push_str(content);
                on_event(NativeStreamEvent::TextDelta(content.to_string()));
            }
            if let Some(tool_calls) = delta.get("tool_calls").and_then(Value::as_array) {
                for tool_call in tool_calls {
                    let index =
                        tool_call.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
                    let entry = self.calls.entry(index).or_default();
                    if let Some(id) = tool_call.get("id").and_then(Value::as_str) {
                        entry.id = id.to_string();
                    }
                    if let Some(function) = tool_call.get("function") {
                        if let Some(name) = function.get("name").and_then(Value::as_str) {
                            entry.name = name.to_string();
                        }
                        if let Some(args) = function.get("arguments").and_then(Value::as_str) {
                            entry.args.push_str(args);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> NativeTurnResult {
        let mut entries = self.calls.into_iter().collect::<Vec<_>>();
        entries.sort_by_key(|(index, _)| *index);
        let tool_calls = entries
            .into_iter()
            .filter_map(|(_, parts)| native_call_from_parts(parts.id, parts.name, parts.args))
            .collect();
        NativeTurnResult {
            content: self.content,
            tool_calls,
        }
    }
}

impl ResponsesAccumulator {
    fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
    where
        F: FnMut(NativeStreamEvent),
    {
        match value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(Value::as_str) {
                    self.content.push_str(delta);
                    on_event(NativeStreamEvent::TextDelta(delta.to_string()));
                }
            }
            "response.output_item.added" => self.record_response_item(&value, false),
            "response.function_call_arguments.delta" => self.record_response_args_delta(&value),
            "response.function_call_arguments.done" => self.record_response_args_done(&value),
            "response.output_item.done" => self.record_response_item(&value, true),
            "response.completed" => self.record_completed_response(&value),
            "error" | "response.failed" => return Err(response_event_error(&value)),
            "response.incomplete" => return Err(response_event_error(&value)),
            _ => {}
        }
        Ok(())
    }

    fn record_response_args_delta(&mut self, value: &Value) {
        let delta = value
            .get("delta")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if delta.is_empty() {
            return;
        }
        let keys = response_call_keys(value, None);
        let primary = keys.first().cloned().unwrap_or_else(|| "0".into());
        let mut parts = self
            .pending_calls
            .get(&primary)
            .cloned()
            .unwrap_or_default();
        parts.args.push_str(delta);
        self.store_pending(keys, parts);
    }

    fn record_response_args_done(&mut self, value: &Value) {
        let Some(arguments) = response_arguments(value) else {
            return;
        };
        let keys = response_call_keys(value, None);
        let primary = keys.first().cloned().unwrap_or_else(|| "0".into());
        let mut parts = self
            .pending_calls
            .get(&primary)
            .cloned()
            .unwrap_or_default();
        parts.args = arguments;
        self.store_pending(keys, parts);
    }

    fn record_response_item(&mut self, value: &Value, finalize: bool) {
        let Some(item) = value.get("item") else {
            return;
        };
        if item.get("type").and_then(Value::as_str) != Some("function_call") {
            return;
        }

        let keys = response_call_keys(value, Some(item));
        let primary = keys.first().cloned().unwrap_or_else(|| "0".into());
        let mut parts = self
            .pending_calls
            .get(&primary)
            .cloned()
            .unwrap_or_default();
        merge_response_item(&mut parts, value, item);

        for key in &keys {
            if let Some(existing) = self.pending_calls.get(key) {
                if parts.id.is_empty() {
                    parts.id = existing.id.clone();
                }
                if parts.name.is_empty() {
                    parts.name = existing.name.clone();
                }
                if parts.args.is_empty() {
                    parts.args = existing.args.clone();
                }
            }
        }

        self.store_pending(keys.clone(), parts.clone());
        if finalize {
            self.finalize_response_call(keys, parts);
        }
    }

    fn record_completed_response(&mut self, value: &Value) {
        let Some(output) = value
            .get("response")
            .and_then(|response| response.get("output"))
            .and_then(Value::as_array)
        else {
            return;
        };

        for item in output {
            if item.get("type").and_then(Value::as_str) != Some("function_call") {
                continue;
            }
            let keys = response_call_keys(value, Some(item));
            let mut parts = ToolCallParts::default();
            merge_response_item(&mut parts, value, item);
            self.finalize_response_call(keys, parts);
        }
    }

    fn store_pending(&mut self, keys: Vec<String>, parts: ToolCallParts) {
        for key in keys {
            self.pending_calls.insert(key, parts.clone());
        }
    }

    fn finalize_response_call(&mut self, keys: Vec<String>, mut parts: ToolCallParts) {
        let final_key = if parts.id.is_empty() {
            keys.first().cloned().unwrap_or_else(|| "0".into())
        } else {
            parts.id.clone()
        };
        if !self.finalized_calls.insert(final_key.clone()) {
            return;
        }
        if parts.id.is_empty() {
            parts.id = final_key;
        }
        if let Some(call) = native_call_from_parts(parts.id, parts.name, parts.args) {
            self.calls.push(call);
        }
    }

    fn finish(self) -> NativeTurnResult {
        NativeTurnResult {
            content: self.content,
            tool_calls: self.calls,
        }
    }
}

fn response_call_keys(value: &Value, item: Option<&Value>) -> Vec<String> {
    let mut keys = Vec::new();
    push_response_key(&mut keys, value.get("call_id"));
    push_response_key(&mut keys, value.get("item_id"));
    push_response_key(&mut keys, value.get("output_index"));
    if let Some(item) = item {
        push_response_key(&mut keys, item.get("call_id"));
        push_response_key(&mut keys, item.get("id"));
        push_response_key(&mut keys, item.get("output_index"));
    }
    keys.dedup();
    if keys.is_empty() {
        keys.push("0".into());
    }
    keys
}

fn push_response_key(keys: &mut Vec<String>, value: Option<&Value>) {
    let Some(value) = value else {
        return;
    };
    if let Some(text) = value.as_str().filter(|text| !text.is_empty()) {
        keys.push(text.to_string());
    } else if let Some(number) = value.as_u64() {
        keys.push(number.to_string());
    }
}

fn merge_response_item(parts: &mut ToolCallParts, value: &Value, item: &Value) {
    if parts.id.is_empty() {
        parts.id = item
            .get("call_id")
            .or_else(|| value.get("call_id"))
            .or_else(|| item.get("id"))
            .or_else(|| value.get("item_id"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    if parts.name.is_empty() {
        parts.name = item
            .get("name")
            .or_else(|| value.get("name"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
    }
    if let Some(arguments) = response_arguments(item).or_else(|| response_arguments(value)) {
        parts.args = arguments;
    }
}

fn response_arguments(value: &Value) -> Option<String> {
    value
        .get("arguments")
        .and_then(|arguments| match arguments {
            Value::String(text) => Some(text.to_string()),
            Value::Object(_) => Some(arguments.to_string()),
            _ => None,
        })
}

fn response_event_error(value: &Value) -> String {
    let message = value
        .get("error")
        .and_then(|error| error.get("message").or_else(|| error.get("code")))
        .and_then(Value::as_str)
        .or_else(|| value.get("message").and_then(Value::as_str))
        .unwrap_or("responses stream failed");
    format!("responses stream error: {message}")
}

fn native_call_from_parts(id: String, openai_name: String, args: String) -> Option<NativeToolCall> {
    if openai_name.is_empty() {
        return None;
    }
    let args = if args.trim().is_empty() {
        json!({})
    } else {
        serde_json::from_str::<Value>(&args).unwrap_or_else(|_| json!({}))
    };
    let name = tools::original_tool_name(&openai_name)
        .unwrap_or(openai_name.as_str())
        .to_string();
    Some(NativeToolCall {
        id,
        name,
        openai_name,
        args,
    })
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
