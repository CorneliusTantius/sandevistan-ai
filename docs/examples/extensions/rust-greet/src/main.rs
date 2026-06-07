use serde_json::{json, Value};
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let request: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));

    match request.get("method").and_then(Value::as_str).unwrap_or_default() {
        "initialize" => initialize(),
        "tool.execute" => execute_tool(&request),
        "hook" => handle_hook(&request),
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

fn handle_hook(request: &Value) {
    if request.pointer("/event/type").and_then(Value::as_str) == Some("before_model_call") {
        println!("{}", json!({
            "decisions": [{ "action": "append_system_context", "content": "Rust extension active." }]
        }));
        return;
    }
    println!("{}", json!({ "decisions": [] }));
}
