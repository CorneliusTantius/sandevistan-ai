mod constants;
mod entries;
mod executors;
mod fs_ops;
mod git_ops;
mod path_utils;
mod registry;
mod rtk;
mod schema;
mod search;
mod shell;
mod types;
mod validation;

pub use registry::{native_system_prompt, original_tool_name, ToolRegistry, ValidatedToolCall};
pub use types::{ToolCall, ToolOptions};

pub(crate) use path_utils::{
    arg_bool, arg_string, arg_usize, backup_file, clean_relative, ignored_name, read_text,
    resolve_existing, resolve_for_write, truncate_string,
};
pub(crate) use rtk::{
    rtk_available, rtk_command, rtk_git_diff, rtk_git_status, rtk_grep, rtk_list, rtk_read,
};

pub fn run_with_options(
    workspace: &std::path::Path,
    call: &ToolCall,
    options: ToolOptions,
) -> String {
    registry::run_with_options(workspace, call, options)
}
