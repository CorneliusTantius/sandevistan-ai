use crate::wire::{NativeStreamEvent, NativeTokenUsage, NativeToolCall, NativeTurnResult};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Default)]
struct ToolCallParts {
    id: String,
    name: String,
    args: String,
}

#[derive(Default)]
pub(super) struct ChatAccumulator {
    content: String,
    calls: HashMap<usize, ToolCallParts>,
    usage: Option<NativeTokenUsage>,
}

#[derive(Default)]
pub(super) struct ResponsesAccumulator {
    content: String,
    calls: Vec<NativeToolCall>,
    pending_calls: HashMap<String, ToolCallParts>,
    finalized_calls: HashSet<String>,
    usage: Option<NativeTokenUsage>,
}

pub(super) enum NativeParser {
    Chat(ChatAccumulator),
    Responses(ResponsesAccumulator),
}

impl NativeParser {
    pub(super) fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
    where
        F: FnMut(NativeStreamEvent),
    {
        match self {
            NativeParser::Chat(acc) => acc.push(value, on_event),
            NativeParser::Responses(acc) => acc.push(value, on_event),
        }
    }

    pub(super) fn finish(self) -> NativeTurnResult {
        match self {
            NativeParser::Chat(acc) => acc.finish(),
            NativeParser::Responses(acc) => acc.finish(),
        }
    }
}

impl ChatAccumulator {
    pub(super) fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
    where
        F: FnMut(NativeStreamEvent),
    {
        if let Some(usage) = parse_chat_usage(&value) {
            self.usage = Some(usage);
        }
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

    pub(super) fn finish(self) -> NativeTurnResult {
        let mut entries = self.calls.into_iter().collect::<Vec<_>>();
        entries.sort_by_key(|(index, _)| *index);
        let tool_calls = entries
            .into_iter()
            .filter_map(|(_, parts)| native_call_from_parts(parts.id, parts.name, parts.args))
            .collect();
        NativeTurnResult {
            content: self.content,
            tool_calls,
            token_usage: self.usage,
        }
    }
}

impl ResponsesAccumulator {
    pub(super) fn push<F>(&mut self, value: Value, on_event: &mut F) -> Result<(), String>
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
            "response.completed" => {
                self.record_completed_response(&value);
                if let Some(usage) = parse_responses_usage(&value) {
                    self.usage = Some(usage);
                }
            }
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

    pub(super) fn finish(self) -> NativeTurnResult {
        NativeTurnResult {
            content: self.content,
            tool_calls: self.calls,
            token_usage: self.usage,
        }
    }
}

fn parse_chat_usage(value: &Value) -> Option<NativeTokenUsage> {
    let usage = value.get("usage")?;
    Some(NativeTokenUsage {
        input_tokens: usage.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0) as usize,
        output_tokens: usage.get("completion_tokens").and_then(Value::as_u64).unwrap_or(0) as usize,
        total_tokens: usage.get("total_tokens").and_then(Value::as_u64).unwrap_or(0) as usize,
    })
    .filter(|usage| usage.total_tokens > 0 || usage.input_tokens > 0 || usage.output_tokens > 0)
}

fn parse_responses_usage(value: &Value) -> Option<NativeTokenUsage> {
    let usage = value.get("response").and_then(|response| response.get("usage"))?;
    let input = usage.get("input_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
    let output = usage.get("output_tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
    let total = usage
        .get("total_tokens")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(input + output);
    Some(NativeTokenUsage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
    })
    .filter(|usage| usage.total_tokens > 0 || usage.input_tokens > 0 || usage.output_tokens > 0)
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
    Some(NativeToolCall {
        id,
        name: openai_name.clone(),
        openai_name,
        args,
    })
}

pub(super) fn extract_response_text(body: &str) -> Option<String> {
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
