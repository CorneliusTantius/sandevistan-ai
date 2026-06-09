use crate::ai;
use serde_json::Value;

pub(crate) fn validate_noop(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &[])
}

pub(crate) fn validate_fs_list(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    optional_string(args, "path")
}

pub(crate) fn validate_fs_read(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    required_string(args, "path")
}

pub(crate) fn validate_fs_edit(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path", "old", "new"])?;
    required_string(args, "path")?;
    required_string(args, "old")?;
    required_string(args, "new")
}

pub(crate) fn validate_fs_write(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path", "content"])?;
    required_string(args, "path")?;
    required_string(args, "content")
}

pub(crate) fn validate_search_rg(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["query", "path", "case_sensitive", "max_results"])?;
    required_string(args, "query")?;
    optional_string(args, "path")?;
    optional_bool(args, "case_sensitive")?;
    optional_integer(args, "max_results")
}

pub(crate) fn validate_git_diff(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["path"])?;
    optional_string(args, "path")
}

pub(crate) fn validate_shell_run(args: &Value, _: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["command", "timeout_secs"])?;
    required_string(args, "command")?;
    optional_integer(args, "timeout_secs")
}

pub(crate) fn validate_agent_delegate(args: &Value, mods: &ai::ModelMods) -> Result<(), String> {
    ensure_no_extra_args(args, &["tasks"])?;
    validate_delegate_tasks(args, mods)
}

fn ensure_no_extra_args(args: &Value, allowed: &[&str]) -> Result<(), String> {
    let Some(object) = args.as_object() else {
        return Err("args must be a JSON object".into());
    };
    if let Some(extra) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(format!("unexpected arg: {extra}"));
    }
    Ok(())
}

fn required_string(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key).and_then(Value::as_str) {
        Some(value) if !value.trim().is_empty() => Ok(()),
        Some(_) => Err(format!("{key} must not be empty")),
        None => Err(format!("missing required arg: {key}")),
    }
}

fn optional_string(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if !value.is_string() => Err(format!("{key} must be a string")),
        _ => Ok(()),
    }
}

fn optional_bool(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if !value.is_boolean() => Err(format!("{key} must be a boolean")),
        _ => Ok(()),
    }
}

fn optional_integer(args: &Value, key: &str) -> Result<(), String> {
    match args.get(key) {
        Some(value) if value.as_u64().is_none() => Err(format!("{key} must be an integer")),
        _ => Ok(()),
    }
}

fn validate_delegate_tasks(args: &Value, mods: &ai::ModelMods) -> Result<(), String> {
    let tasks = args
        .get("tasks")
        .and_then(Value::as_array)
        .ok_or_else(|| "missing required arg: tasks".to_string())?;
    if tasks.is_empty() {
        return Err("tasks must not be empty".into());
    }
    if tasks.len() > 8 {
        return Err("tasks exceeds maxItems 8".into());
    }
    if tasks.len() == 1 {
        if let Some(task_text) = tasks[0].get("task").and_then(Value::as_str) {
            if task_text.chars().count() > 350 {
                return Err("single delegate task too broad; split into multiple small concurrent tasks".into());
            }
        }
    }
    for (index, task) in tasks.iter().enumerate() {
        let agent = task
            .get("agent")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("tasks[{index}].agent must be a string"))?;
        if !mods.subagents.iter().any(|name| name == agent) {
            return Err(format!("tasks[{index}].agent unknown or disabled: {agent}"));
        }
        let task_text = task
            .get("task")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("tasks[{index}].task must be a string"))?;
        if task_text.trim().is_empty() {
            return Err(format!("tasks[{index}].task must not be empty"));
        }
        if task_text.chars().count() > 1_000 {
            return Err(format!("tasks[{index}].task too broad; split into smaller specific tasks under 1000 chars"));
        }
    }
    Ok(())
}
