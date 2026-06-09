use serde_json::{json, Value};

pub(crate) fn params_empty(_: &[String]) -> Value {
    json!({"type":"object","properties":{},"required":[],"additionalProperties":false})
}

pub(crate) fn params_fs_list(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string","default":"."}},"required":[],"additionalProperties":false})
}

pub(crate) fn params_fs_read(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false})
}

pub(crate) fn params_fs_edit(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string"},"new":{"type":"string"}},"required":["path","old","new"],"additionalProperties":false})
}

pub(crate) fn params_fs_write(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"},"content":{"type":"string"}},"required":["path","content"],"additionalProperties":false})
}

pub(crate) fn params_search_rg(_: &[String]) -> Value {
    json!({"type":"object","properties":{"query":{"type":"string"},"path":{"type":"string","default":"."},"case_sensitive":{"type":"boolean","default":false},"max_results":{"type":"integer","minimum":1,"maximum":200,"default":200}},"required":["query"],"additionalProperties":false})
}

pub(crate) fn params_git_diff(_: &[String]) -> Value {
    json!({"type":"object","properties":{"path":{"type":"string"}},"required":[],"additionalProperties":false})
}

pub(crate) fn params_shell_run(_: &[String]) -> Value {
    json!({"type":"object","properties":{"command":{"type":"string"},"timeout_secs":{"type":"integer","minimum":1,"maximum":300,"default":120}},"required":["command"],"additionalProperties":false})
}

pub(crate) fn params_agent_delegate(subagents: &[String]) -> Value {
    json!({"type":"object","properties":{"tasks":{"type":"array","minItems":1,"maxItems":8,"items":{"type":"object","properties":{"agent":{"type":"string","enum":subagents},"task":{"type":"string","maxLength":1000}},"required":["agent","task"],"additionalProperties":false}}},"required":["tasks"],"additionalProperties":false})
}
