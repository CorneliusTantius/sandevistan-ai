use crate::wire::NativeMessage;
use serde_json::{json, Value};

pub(super) fn chat_native_messages(messages: Vec<NativeMessage>) -> Vec<Value> {
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

pub(super) fn responses_native_input(messages: Vec<NativeMessage>) -> Vec<Value> {
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
