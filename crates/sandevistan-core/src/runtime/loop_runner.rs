use crate::{
    context::PromptConfig,
    provider,
    runtime::{
        messages::chat_to_agent_messages,
        prompt::{context_window, mods_prompt, truncate_chars},
        stream::{AgentEventSink, StreamState},
        types::{to_native_message_refs, AgentMessage},
        AgentBudgets, AgentEvent, CancellationToken,
    },
    tools::{ToolHost, ToolRequest},
    wire::NativeToolCall,
    AgentMods, ChatMessage, ProviderConfig,
};
use std::sync::Arc;

#[derive(Clone)]
pub struct AgentRuntimeConfig {
    pub session_id: String,
    pub messages: Vec<ChatMessage>,
    pub mods: AgentMods,
    pub prompt_config: PromptConfig,
    pub summary: Option<String>,
    pub system_prompt: Option<String>,
    pub provider: ProviderConfig,
    pub read_only: bool,
    pub delegate_depth_remaining: usize,
    pub budgets: AgentBudgets,
    pub cancellation_token: CancellationToken,
    pub tool_host: Arc<dyn ToolHost>,
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

        let mut system_prompt = config
            .system_prompt
            .clone()
            .unwrap_or_else(|| config.tool_host.system_prompt());
        let mods_prompt = mods_prompt(&config.mods);
        if !mods_prompt.is_empty() {
            system_prompt.push_str("\n\n");
            system_prompt.push_str(&mods_prompt);
        }

        let prompt = crate::context::build_prompt(
            &system_prompt,
            config.summary.as_deref(),
            &config.messages,
            &config.prompt_config,
        );
        let mut agent_messages = chat_to_agent_messages(prompt);
        let native_tools = config.tool_host.specs();

        let mut turn_index = 0usize;
        loop {
            if config.cancellation_token.is_cancelled() {
                return Err(runtime_error("cancelled", &config.messages));
            }
            let turn_number = turn_index + 1;
            (config.on_event)(AgentEvent::TurnStart { turn: turn_number });

            let turn_messages = context_window(&agent_messages, &config.prompt_config);
            let stream_state = StreamState::new(config.on_event.clone());
            let turn = provider::complete_native_stream(
                config.provider.clone(),
                to_native_message_refs(&turn_messages),
                native_tools.clone(),
                config.cancellation_token.clone(),
                stream_state.handler(),
            )
            .await
            .map_err(|error| runtime_error(error, &config.messages))?;
            if let Some(usage) = &turn.token_usage {
                (config.on_event)(AgentEvent::TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    total_tokens: usage.total_tokens,
                });
            }

            let content = turn.content.trim().to_string();
            let has_streamed = stream_state.has_streamed();
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
                    return Err(runtime_error("cancelled", &config.messages));
                }
                (config.on_event)(AgentEvent::ToolCallStart {
                    id: call.id.clone(),
                    name: call.name.clone(),
                });
                let tool_host = config.tool_host.clone();
                let request = ToolRequest {
                    session_id: config.session_id.clone(),
                    name: call.name.clone(),
                    args: call.args.clone(),
                    read_only: config.read_only,
                    delegate_depth_remaining: config.delegate_depth_remaining,
                };
                let cancellation_token = config.cancellation_token.clone();
                let timeout = config.budgets.tool_timeout;
                handles.push(tokio::spawn(async move {
                    if cancellation_token.is_cancelled() {
                        return ToolRunRecord {
                            index,
                            call,
                            content: "cancelled".into(),
                        };
                    }
                    let content = tokio::time::timeout(timeout, tool_host.run(request))
                        .await
                        .unwrap_or_else(|_| {
                            format!(
                                "{}\nstatus: failed\nerror: tool execution timed out after {}s",
                                call.name,
                                timeout.as_secs()
                            )
                        });
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
                            format!("tool task failed: {error}"),
                            &config.messages,
                        ));
                    }
                }
            }
            records.sort_by_key(|record| record.index);

            if config.cancellation_token.is_cancelled() {
                return Err(runtime_error("cancelled", &config.messages));
            }

            for record in records {
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

fn runtime_error(message: impl Into<String>, messages: &[ChatMessage]) -> AgentRuntimeError {
    AgentRuntimeError::new(message, messages)
}
