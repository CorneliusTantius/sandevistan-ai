#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum AgentEvent {
    AgentStart,
    TurnStart {
        turn: usize,
    },
    MessageDelta {
        text: String,
    },
    AssistantMessage {
        content: String,
    },
    ToolCallStart {
        id: String,
        name: String,
    },
    ToolCallEnd {
        id: String,
        name: String,
        content: String,
    },
    TurnEnd {
        turn: usize,
    },
    TokenUsage {
        input_tokens: usize,
        output_tokens: usize,
        total_tokens: usize,
    },
    AgentEnd,
    Error {
        message: String,
    },
}
