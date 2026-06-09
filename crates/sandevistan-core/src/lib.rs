pub mod config;
pub mod context;
pub mod provider;
pub mod runtime;
pub mod tools;
pub mod wire;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

pub use config::{AgentMods, ProviderConfig, SubagentDef};
pub use context::PromptConfig;
pub use provider::{complete, complete_native_stream};
pub use runtime::{
    AgentBudgets, AgentEvent, AgentRunResult, AgentRuntime, AgentRuntimeConfig, AgentRuntimeError,
    CancellationToken,
};
pub use tools::{ToolHost, ToolRequest};
pub use wire::{NativeMessage, NativeStreamEvent, NativeTokenUsage, NativeToolCall, NativeToolSpec, NativeTurnResult};
