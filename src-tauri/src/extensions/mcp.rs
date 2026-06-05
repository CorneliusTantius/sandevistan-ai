use super::config;

pub fn enabled() -> bool {
    config::extension_enabled("mcp", false)
}

pub fn system_prompt() -> Option<String> {
    enabled().then(|| {
        "MCP extension configured, but MCP runtime tools are not loaded in this build.".into()
    })
}

pub fn list_servers() -> String {
    if !enabled() {
        return "status: failed\nerror: mcp extension disabled".into();
    }
    "status: ok\nservers: none\nnote: MCP extension scaffold is enabled; server protocol integration is not configured yet.".into()
}
