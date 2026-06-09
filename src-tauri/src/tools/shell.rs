use crate::command_utils;
use serde_json::Value;
use std::{path::Path, process::Command, time::Duration};

use super::{
    arg_string, rtk_available, rtk_command, truncate_string, ToolOptions,
    constants::{MAX_SHELL_OUTPUT_BYTES, RTK_REWRITE_TIMEOUT, SHELL_COMMAND_TIMEOUT},
};

pub(crate) fn shell_run(workspace: &Path, args: &Value, options: ToolOptions) -> Result<String, String> {
    if !options.shell_enabled {
        return Err("shell tools disabled for this profile".into());
    }
    let command = arg_string(args, "command").ok_or_else(|| "missing command".to_string())?;
    let command = command.trim();
    if command.is_empty() {
        return Err("command is empty".into());
    }
    let timeout = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .map(|value| value.clamp(1, 300))
        .map(Duration::from_secs)
        .unwrap_or(SHELL_COMMAND_TIMEOUT);

    let output = if options.rtk_enabled {
        let rewritten = rtk_rewrite_shell_command(workspace, command)?;
        let mut cmd = if let Some(rewritten) = rewritten {
            let mut cmd = Command::new("sh");
            cmd.current_dir(workspace).arg("-lc").arg(rewritten);
            cmd
        } else {
            let mut cmd = rtk_command()?;
            cmd.current_dir(workspace).arg("run").arg("-c").arg(command);
            cmd
        };
        command_utils::output_with_timeout("rtk-shell", &mut cmd, timeout)?
    } else {
        let mut cmd = Command::new("sh");
        cmd.current_dir(workspace).arg("-lc").arg(command);
        command_utils::output_with_timeout("shell", &mut cmd, timeout)?
    };

    let status = output.status.code().map_or_else(|| "signal".into(), |code| code.to_string());
    let stdout = truncate_string(String::from_utf8_lossy(&output.stdout).to_string(), MAX_SHELL_OUTPUT_BYTES / 2);
    let stderr = truncate_string(String::from_utf8_lossy(&output.stderr).to_string(), MAX_SHELL_OUTPUT_BYTES / 2);
    let mut parts = vec![format!("exit: {status}")];
    if !stdout.trim().is_empty() {
        parts.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    Ok(parts.join("\n"))
}

fn rtk_rewrite_shell_command(workspace: &Path, command: &str) -> Result<Option<String>, String> {
    if !rtk_available() {
        return Err("rtk enabled but unavailable on PATH".into());
    }
    let mut rewrite = rtk_command()?;
    rewrite.current_dir(workspace).arg("rewrite").arg(command);
    let output = command_utils::output_with_timeout("rtk-rewrite", &mut rewrite, RTK_REWRITE_TIMEOUT)?;
    let rewritten = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!rewritten.is_empty()).then_some(rewritten))
}
