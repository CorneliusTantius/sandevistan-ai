use crate::{
    ai::{self, ChatMessage},
    context as prompt_context,
    extensions::hooks::HookEvent,
    runtime_wire::{NativeStreamEvent, NativeToolCall},
    tools,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use super::{
    budget::AgentBudgets,
    context,
    events::AgentEvent,
    messages::chat_to_agent_messages,
    tool_exec,
    types::{to_native_message_refs, AgentMessage, AgentRole},
};

pub type AgentEventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Clone)]
pub struct AgentRuntimeConfig {
    pub workspace: PathBuf,
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub mods: ai::ModelMods,
    pub prompt_config: prompt_context::PromptConfig,
    pub summary: Option<String>,
    pub system_prompt: Option<String>,
    pub model: Option<String>,
    pub read_only: bool,
    pub delegate_depth_remaining: usize,
    pub budgets: AgentBudgets,
    pub cancellation_token: super::cancel::CancellationToken,
    pub on_event: AgentEventSink,
}

pub struct AgentRuntime;

#[derive(Debug)]
pub struct AgentRunResult {
    pub messages: Vec<ChatMessage>,
}

#[derive(Debug)]
pub struct AgentRuntimeError {
    pub message: String,
    pub messages: Vec<ChatMessage>,
}

impl AgentRuntimeError {
    pub fn new(message: impl Into<String>, messages: &[ChatMessage]) -> Self {
        Self {
            message: message.into(),
            messages: messages.to_vec(),
        }
    }
}

const STORED_TOOL_CHARS: usize = 4_000;

struct ToolRunRecord {
    index: usize,
    call: NativeToolCall,
    content: String,
}

impl AgentRuntime {
    pub fn new() -> Self {
        Self
    }

    pub async fn run(
        &self,
        mut config: AgentRuntimeConfig,
    ) -> Result<AgentRunResult, AgentRuntimeError> {
        (config.on_event)(AgentEvent::AgentStart);
        crate::extensions::hook_bus(&config.workspace).emit(HookEvent::AgentStart);

        let mut system_prompt = config
            .system_prompt
            .clone()
            .unwrap_or_else(|| context::system_prompt(&config.mods));
        let extension_prompt = crate::extensions::system_prompt(&config.workspace);
        if !extension_prompt.trim().is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&extension_prompt);
        }
        let prompt = context::build_prompt(
            &system_prompt,
            config.summary.as_deref(),
            &config.messages,
            &config.prompt_config,
        );
        let mut agent_messages = chat_to_agent_messages(prompt);
        let mut native_tools = tools::ToolRegistry::new(
            config.mods.subagents_enabled && !config.mods.subagents.is_empty(),
            &config.mods.subagents,
            config.mods.shell_enabled,
            config.read_only,
        )
        .specs();
        if config.mods.mcp_enabled {
            native_tools.extend(crate::mcp::tool_specs());
        }
        native_tools.extend(crate::extensions::tool_specs(&config.workspace));

        let mut turn_index = 0usize;
        loop {
            if config.cancellation_token.is_cancelled() {
                return Err(runtime_error(
                    &config.workspace,
                    "cancelled",
                    &config.messages,
                ));
            }
            let turn_number = turn_index + 1;
            (config.on_event)(AgentEvent::TurnStart { turn: turn_number });

            let turn_messages = context_window(&agent_messages, &config.prompt_config);
            let streamed = Arc::new(Mutex::new(false));
            let sink = config.on_event.clone();
            let streamed_for_delta = streamed.clone();
            let turn = ai::complete_native_stream_model(
                to_native_message_refs(&turn_messages),
                native_tools.clone(),
                config
                    .model
                    .clone()
                    .or_else(|| Some(config.mods.main_model.clone())),
                config.cancellation_token.clone(),
                move |event| match event {
                    NativeStreamEvent::TextDelta(delta) => {
                        if delta.is_empty() {
                            return;
                        }
                        if let Ok(mut has_streamed) = streamed_for_delta.lock() {
                            *has_streamed = true;
                        }
                        sink(AgentEvent::MessageDelta { text: delta });
                    }
                },
            )
            .await
            .map_err(|error| runtime_error(&config.workspace, error, &config.messages))?;
            if let Some(usage) = &turn.token_usage {
                (config.on_event)(AgentEvent::TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    total_tokens: usage.total_tokens,
                });
            }

            let content = turn.content.trim().to_string();
            let has_streamed = streamed.lock().map(|value| *value).unwrap_or(false);
            let tool_calls = turn.tool_calls.clone();
            agent_messages.push(AgentMessage::assistant(content.clone(), tool_calls.clone()));

            if tool_calls.is_empty() {
                if !has_streamed && !content.is_empty() {
                    (config.on_event)(AgentEvent::MessageDelta {
                        text: content.clone(),
                    });
                }
                (config.on_event)(AgentEvent::AssistantMessage {
                    content: content.clone(),
                });
                config.messages.push(ChatMessage {
                    role: "assistant".into(),
                    content,
                });
                (config.on_event)(AgentEvent::TurnEnd { turn: turn_number });
                crate::extensions::hook_bus(&config.workspace).emit(HookEvent::AgentEnd);
                (config.on_event)(AgentEvent::AgentEnd);
                return Ok(AgentRunResult {
                    messages: config.messages,
                });
            }

            let mut handles = Vec::new();
            for (index, call) in tool_calls.into_iter().enumerate() {
                if config.cancellation_token.is_cancelled() {
                    return Err(runtime_error(
                        &config.workspace,
                        "cancelled",
                        &config.messages,
                    ));
                }
                let tool_call = tools::ToolCall {
                    name: call.name.clone(),
                    args: call.args.clone(),
                };
                let name = tool_call.name.clone();
                (config.on_event)(AgentEvent::ToolCallStart {
                    id: call.id.clone(),
                    name,
                });
                let workspace = config.workspace.clone();
                let session_id = config.session_id.clone();
                let mods = config.mods.clone();
                let budgets = config.budgets.clone();
                let read_only = config.read_only;
                let delegate_depth_remaining = config.delegate_depth_remaining;
                let cancellation_token = config.cancellation_token.clone();
                handles.push(tauri::async_runtime::spawn(async move {
                    if cancellation_token.is_cancelled() {
                        return ToolRunRecord {
                            index,
                            call,
                            content: "cancelled".into(),
                        };
                    }
                    let content = tool_exec::run_streamed_tool_call(
                        workspace,
                        session_id,
                        tool_call,
                        mods,
                        &budgets,
                        read_only,
                        delegate_depth_remaining,
                    )
                    .await;
                    ToolRunRecord {
                        index,
                        call,
                        content,
                    }
                }));
            }

            let mut records = Vec::new();
            for handle in handles {
                match handle.await {
                    Ok(record) => records.push(record),
                    Err(error) => {
                        return Err(runtime_error(
                            &config.workspace,
                            format!("tool task failed: {error}"),
                            &config.messages,
                        ));
                    }
                }
            }
            records.sort_by_key(|record| record.index);

            if config.cancellation_token.is_cancelled() {
                return Err(runtime_error(
                    &config.workspace,
                    "cancelled",
                    &config.messages,
                ));
            }

            for record in records {
                crate::extensions::hook_bus(&config.workspace).emit(HookEvent::AfterToolResult {
                    tool: record.call.name.clone(),
                    content: record.content.clone(),
                });
                (config.on_event)(AgentEvent::ToolCallEnd {
                    id: record.call.id.clone(),
                    name: record.call.name.clone(),
                    content: record.content.clone(),
                });
                let stored_content = truncate_chars(&record.content, STORED_TOOL_CHARS);
                config.messages.push(ChatMessage {
                    role: "tool".into(),
                    content: stored_content,
                });
                agent_messages.push(AgentMessage::tool_result(
                    record.call.id,
                    record.call.openai_name,
                    record.content,
                ));
            }

            (config.on_event)(AgentEvent::TurnEnd { turn: turn_number });
            turn_index += 1;
        }
    }
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.into();
    }
    let mut end = 0;
    for (count, (index, ch)) in value.char_indices().enumerate() {
        if count >= max_chars {
            break;
        }
        end = index + ch.len_utf8();
    }
    format!("{}\n... truncated to {max_chars} chars", &value[..end])
}

fn runtime_error(
    workspace: &std::path::Path,
    message: impl Into<String>,
    messages: &[ChatMessage],
) -> AgentRuntimeError {
    let message = message.into();
    crate::extensions::hook_bus(workspace).emit(HookEvent::Error {
        message: message.clone(),
    });
    AgentRuntimeError::new(message, messages)
}

fn context_window<'a>(
    messages: &'a [AgentMessage],
    prompt_config: &prompt_context::PromptConfig,
) -> Vec<&'a AgentMessage> {
    let mut system = Vec::new();
    let mut rest = Vec::new();
    for message in messages {
        if matches!(message.role, AgentRole::System) {
            system.push(message);
        } else {
            rest.push(message);
        }
    }

    let mut used = system
        .iter()
        .map(|message| message.estimated_chars())
        .sum::<usize>();
    let mut selected = Vec::new();
    for message in rest.into_iter().rev() {
        let cost = message.estimated_chars();
        if !selected.is_empty() && used + cost > prompt_config.max_prompt_chars {
            break;
        }
        used += cost;
        selected.push(message);
    }
    selected.reverse();
    while matches!(
        selected.first().map(|message| &message.role),
        Some(AgentRole::Tool)
    ) {
        selected.remove(0);
    }

    system.extend(selected);
    system
}
