use std::time::Duration;

pub(crate) const CONFIG_DIR_NAME: &str = ".sandevistan";
pub(crate) const MAX_LIST_ENTRIES: usize = 200;
pub(crate) const MAX_READ_BYTES: u64 = 24_000;
pub(crate) const MAX_WRITE_BYTES: usize = 200_000;
pub(crate) const MAX_TOOL_OUTPUT_BYTES: usize = 16_000;
pub(crate) const MAX_SEARCH_RESULTS: usize = 200;
pub(crate) const MAX_DIFF_BYTES: usize = 50_000;
pub(crate) const TOOL_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
pub(crate) const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(45);
pub(crate) const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const RTK_REWRITE_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const MAX_SHELL_OUTPUT_BYTES: usize = 24_000;
