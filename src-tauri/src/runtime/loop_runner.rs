use crate::{
    ai::{self, ChatMessage},
    context as prompt_context,
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
    types::{to_native_messages, AgentMessage, AgentRole},
};

pub type AgentEventSink = Arc<dyn Fn(AgentEvent) + Send + Sync>;

#[derive(Clone)]
pub struct AgentRuntimeConfig {
    pub workspace: PathBuf,
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

        let system_prompt = config
            .system_prompt
            .clone()
            .unwrap_or_else(|| context::system_prompt(&config.mods));
        let prompt = context::build_prompt(
            &system_prompt,
            config.summary.as_deref(),
            &config.messages,
            &config.prompt_config,
        );
        let mut agent_messages = chat_to_agent_messages(prompt);
        let native_tools = tools::ToolRegistry::new(
            config.mods.subagents_enabled && !config.mods.subagents.is_empty(),
            &config.mods.subagents,
            config.mods.shell_enabled,
            config.read_only,
        )
        .specs();

        let mut turn_index = 0usize;
        loop {
            if config.cancellation_token.is_cancelled() {
                return Err(AgentRuntimeError::new("cancelled", &config.messages));
            }
            let turn_number = turn_index + 1;
            (config.on_event)(AgentEvent::TurnStart { turn: turn_number });

            let turn_messages = context_window(&agent_messages, &config.prompt_config);
            let streamed = Arc::new(Mutex::new(false));
            let sink = config.on_event.clone();
            let streamed_for_delta = streamed.clone();
            let turn = ai::complete_native_stream_model(
                to_native_messages(&turn_messages),
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
            .map_err(|error| AgentRuntimeError::new(error, &config.messages))?;

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
                (config.on_event)(AgentEvent::AgentEnd);
                return Ok(AgentRunResult {
                    messages: config.messages,
                });
            }

            let mut handles = Vec::new();
            for (index, call) in tool_calls.into_iter().enumerate() {
                if config.cancellation_token.is_cancelled() {
                    return Err(AgentRuntimeError::new("cancelled", &config.messages));
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
                        return Err(AgentRuntimeError::new(
                            format!("tool task failed: {error}"),
                            &config.messages,
                        ));
                    }
                }
            }
            records.sort_by_key(|record| record.index);

            if config.cancellation_token.is_cancelled() {
                return Err(AgentRuntimeError::new("cancelled", &config.messages));
            }

            for record in records {
                (config.on_event)(AgentEvent::ToolCallEnd {
                    id: record.call.id.clone(),
                    name: record.call.name.clone(),
                    content: record.content.clone(),
                });
                config.messages.push(ChatMessage {
                    role: "tool".into(),
                    content: record.content.clone(),
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

fn context_window(
    messages: &[AgentMessage],
    prompt_config: &prompt_context::PromptConfig,
) -> Vec<AgentMessage> {
    let mut system = Vec::new();
    let mut rest = Vec::new();
    for message in messages {
        if matches!(message.role, AgentRole::System) {
            system.push(message.clone());
        } else {
            rest.push(message.clone());
        }
    }

    let mut used = system
        .iter()
        .map(AgentMessage::estimated_chars)
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
