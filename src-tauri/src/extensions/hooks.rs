use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone)]
pub enum HookEvent {
    AgentStart,
    BeforeModelCall,
    BeforeToolCall { tool: String, args: Value },
    AfterToolResult { tool: String, content: String },
    AgentEnd,
    Error { message: String },
}

#[derive(Debug, Clone)]
pub enum HookDecision {
    Continue,
    Block { reason: String },
    ModifyToolArgs { args: Value },
    AppendSystemContext { content: String },
}

pub struct HookBus<'a> {
    workspace: &'a Path,
}

impl<'a> HookBus<'a> {
    pub fn new(workspace: &'a Path) -> Self {
        Self { workspace }
    }

    pub fn emit(&self, event: HookEvent) -> Vec<HookDecision> {
        super::external::emit(self.workspace, &event)
    }

    pub fn system_context(&self) -> String {
        self.emit(HookEvent::BeforeModelCall)
            .into_iter()
            .filter_map(|decision| match decision {
                HookDecision::AppendSystemContext { content } => Some(content),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}
