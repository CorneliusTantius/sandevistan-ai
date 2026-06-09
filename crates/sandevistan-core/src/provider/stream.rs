use crate::{
    runtime::CancellationToken,
    wire::{NativeMessage, NativeStreamEvent, NativeToolSpec, NativeTurnResult},
    ProviderConfig,
};
use serde_json::{json, Value};

use super::{
    format::{chat_native_messages, responses_native_input},
    http::{
        flush_utf8_prefix, normalize_newlines, next_sse_event, open_stream_with_retry,
        API_STREAM_IDLE_TIMEOUT,
    },
    parser::{ChatAccumulator, NativeParser, ResponsesAccumulator},
};

pub async fn complete_native_stream<F>(
    runtime: ProviderConfig,
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
    OpenAiChat(ProviderConfig),
    OpenAiResponses(ProviderConfig),
}

impl ProviderAdapter {
    fn from_runtime(runtime: ProviderConfig) -> Self {
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
    runtime: ProviderConfig,
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
        "stream_options": { "include_usage": true },
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
    runtime: ProviderConfig,
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
    runtime: ProviderConfig,
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
