use std::time::Duration;

#[derive(Debug, Clone)]
pub struct AgentBudgets {
    pub tool_timeout: Duration,
}

impl Default for AgentBudgets {
    fn default() -> Self {
        Self {
            tool_timeout: Duration::from_secs(120),
        }
    }
}
