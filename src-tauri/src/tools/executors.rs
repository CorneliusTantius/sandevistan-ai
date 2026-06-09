use serde_json::Value;
use std::path::Path;

use super::{
    fs_ops, git_ops, search, shell,
    arg_string, rtk_available, rtk_git_diff, rtk_git_status, rtk_grep, rtk_list, rtk_read,
    ToolOptions,
};

pub(crate) fn exec_fs_list(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    let path = arg_string(args, "path").unwrap_or_else(|| ".".into());
    if options.rtk_enabled && rtk_available() {
        rtk_list(workspace, &path).or_else(|_| fs_ops::list_dir(workspace, path))
    } else {
        fs_ops::list_dir(workspace, path)
    }
}

pub(crate) fn exec_fs_read(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    let path = arg_string(args, "path").ok_or_else(|| "missing path".to_string())?;
    if options.rtk_enabled && rtk_available() {
        rtk_read(workspace, &path).or_else(|_| fs_ops::read_file(workspace, path))
    } else {
        fs_ops::read_file(workspace, path)
    }
}

pub(crate) fn exec_fs_edit(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    fs_ops::edit_file(workspace, args, options)
}

pub(crate) fn exec_fs_write(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    fs_ops::write_file(workspace, args, options)
}

pub(crate) fn exec_search_rg(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_grep(workspace, args).or_else(|_| search::ripgrep(workspace, args))
    } else {
        search::ripgrep(workspace, args)
    }
}

pub(crate) fn exec_git_status(workspace: &Path, _: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_git_status(workspace).or_else(|_| git_ops::status(workspace))
    } else {
        git_ops::status(workspace)
    }
}

pub(crate) fn exec_git_diff(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if options.rtk_enabled && rtk_available() {
        rtk_git_diff(workspace, args).or_else(|_| git_ops::diff(workspace, args))
    } else {
        git_ops::diff(workspace, args)
    }
}

pub(crate) fn exec_shell_run(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    shell::shell_run(workspace, args, options)
}

pub(crate) fn exec_agent_delegate(_: &Path, _: &Value, _: ToolOptions) -> Result<String, String> {
    Err("agent.delegate must be executed by agent runtime".into())
}
