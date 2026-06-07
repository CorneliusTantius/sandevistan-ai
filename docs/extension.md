# Extensions

## Model

Extension = executable + `extension.toml`.

App:
1. discovers manifests
2. checks enabled state
3. starts extension executable
4. sends one JSON request to stdin
5. reads one JSON response from stdout
6. registers tools / applies hook decisions

No bundled runtime. Rust/native binary recommended.

## Discovery

Global:
```text
~/.sandevistan/extensions/<id>/extension.toml
~/.sandevistan/extensions/<id>.toml
```

Workspace:
```text
<workspace-or-parent>/.sandevistan/extensions/<id>/extension.toml
<workspace-or-parent>/.sandevistan/extensions/<id>.toml
```

Workspace search stops at repo root (`.git`).

Duplicate ids: first discovered wins.

## Manifest

```toml
id = "my-ext"
name = "My Extension"
description = "Native binary extension"
enabled = true
hooks = ["before_model_call", "before_tool_call"]
timeout_ms = 1500
args = []

[commands]
linux-x86_64 = "bin/linux-x86_64/my-ext"
linux-aarch64 = "bin/linux-aarch64/my-ext"
macos-x86_64 = "bin/macos-x86_64/my-ext"
macos-aarch64 = "bin/macos-aarch64/my-ext"
windows-x86_64 = "bin/windows-x86_64/my-ext.exe"
default = "bin/my-ext"
```

Legacy single command:
```toml
command = "target/release/my-ext"
```

Command resolution order:
1. `<os>-<arch>`
2. `<os>`
3. `default`
4. `command`

Relative command path = relative to manifest folder.

Valid id:
- lowercase letters
- numbers
- `-`
- `.`
- max 64 chars
- cannot start/end with `-`

## Hooks

Supported hooks:
```text
agent_start
before_model_call
before_tool_call
after_tool_result
agent_end
error
```

Hook decisions:
```json
{ "action": "continue" }
{ "action": "block", "reason": "blocked" }
{ "action": "modify_tool_args", "args": { } }
{ "action": "append_system_context", "content": "extra instruction" }
```

## Protocol

Request:
```json
{
  "protocol": "sandevistan.extension.v1",
  "request_id": "...",
  "extension_id": "my-ext",
  "workspace": "/repo",
  "method": "initialize",
  "event": null,
  "tool_call": null
}
```

Methods:
```text
initialize     -> return tool specs
tool.execute   -> execute registered tool
hook           -> handle hook event
```

Response:
```json
{
  "decisions": [],
  "tools": [],
  "content": "status: ok\nresult"
}
```

Extension must print valid JSON to stdout.

## Register tool

`initialize` response:
```json
{
  "tools": [
    {
      "name": "greet",
      "description": "Greet someone by name",
      "parameters": {
        "type": "object",
        "properties": {
          "name": { "type": "string" }
        },
        "required": ["name"],
        "additionalProperties": false
      }
    }
  ]
}
```

Tool is exposed to model as:
```text
ext.<extension_id>.<tool_name>
```

## Rust example

Full copyable example:
```text
docs/examples/extensions/rust-greet/
```

`Cargo.toml`:
```toml
[package]
name = "my-ext"
version = "0.1.0"
edition = "2021"

[dependencies]
serde_json = "1"
```

`src/main.rs`:
```rust
use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let request: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));

    match request.get("method").and_then(Value::as_str).unwrap_or_default() {
        "initialize" => initialize(),
        "tool.execute" => execute_tool(&request),
        "hook" => hook(&request),
        _ => println!("{}", json!({ "decisions": [] })),
    }
}

fn initialize() {
    println!("{}", json!({
        "tools": [{
            "name": "greet",
            "description": "Greet someone by name",
            "parameters": {
                "type": "object",
                "properties": { "name": { "type": "string" } },
                "required": ["name"],
                "additionalProperties": false
            }
        }]
    }));
}

fn execute_tool(request: &Value) {
    let name = request
        .pointer("/tool_call/args/name")
        .and_then(Value::as_str)
        .unwrap_or("world");
    println!("{}", json!({ "content": format!("status: ok\nhello {name}") }));
}

fn hook(request: &Value) {
    if request.pointer("/event/type").and_then(Value::as_str) == Some("before_model_call") {
        println!("{}", json!({
            "decisions": [{
                "action": "append_system_context",
                "content": "My extension is active."
            }]
        }));
        return;
    }

    if request.pointer("/event/type").and_then(Value::as_str) == Some("before_tool_call")
        && request.pointer("/event/tool").and_then(Value::as_str) == Some("shell.run")
    {
        let command = request.pointer("/event/args/command").and_then(Value::as_str).unwrap_or("");
        if command.contains("rm -rf") {
            println!("{}", json!({
                "decisions": [{ "action": "block", "reason": "blocked rm -rf" }]
            }));
            return;
        }
    }

    println!("{}", json!({ "decisions": [] }));
}
```

Build:
```bash
cd docs/examples/extensions/rust-greet
cargo build --release
```

Install:
```text
~/.sandevistan/extensions/my-ext/
  extension.toml
  target/release/my-ext
```

Reload app or press Extensions reload.

## Ship per-platform binaries

Package layout:
```text
my-ext/
  extension.toml
  bin/linux-x86_64/my-ext
  bin/linux-aarch64/my-ext
  bin/macos-x86_64/my-ext
  bin/macos-aarch64/my-ext
  bin/windows-x86_64/my-ext.exe
```

Copy `my-ext/` to:
```text
~/.sandevistan/extensions/my-ext/
```

## Debug

Checklist:
- manifest path valid
- `enabled = true` or enabled in UI
- command exists for current OS/arch
- binary executable bit set on Unix/macOS
- stdout contains only JSON
- stderr allowed for logs
- response returns before `timeout_ms`

Manual test:
```bash
echo '{"protocol":"sandevistan.extension.v1","request_id":"test","extension_id":"my-ext","workspace":"/tmp","method":"initialize"}' \
  | ~/.sandevistan/extensions/my-ext/bin/linux-x86_64/my-ext
```
