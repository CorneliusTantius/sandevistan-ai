# sandevistan-core

Reusable Rust core for the Sandevistan agent engine.

This crate is meant to be used outside the Tauri desktop app. It owns the generic AI/runtime pieces; the app layer owns UI, sessions, config files, and frontend events.

## What belongs here

- LLM provider calls
- OpenAI-compatible streaming
- tool-call wire types
- chat message types
- prompt/context budgeting
- cancellation token
- runtime events
- shared agent runtime types

## What does not belong here

- Tauri commands/events
- Svelte/frontend code
- `~/.sandevistan` config loading
- workspace/session persistence
- desktop file watcher/terminal UI

## Main modules

```txt
src/lib.rs          public exports
src/config.rs       provider config, agent mods, subagent definitions
src/provider/       OpenAI-compatible provider client
  http.rs           shared HTTP/retry/SSE helpers
  text.rs           non-streaming text completions
  stream.rs         streaming turn dispatcher
  format.rs         provider request formatting
  parser.rs         stream response parsers
src/tools.rs        portable tool host trait/types
src/wire.rs         native messages, tool specs, tool calls, turn result
src/context.rs      prompt context trimming/budgeting
src/runtime/        agent loop, cancellation, events, budgets, internal message types
  loop_runner.rs    main reusable agent loop
  prompt.rs         model-mod prompt and context-window helpers
  stream.rs         provider stream event adapter
```

## Key types

```rust
ChatMessage
ProviderConfig
AgentMods
SubagentDef
PromptConfig
CancellationToken
AgentEvent
ToolHost
ToolRequest
NativeMessage
NativeToolSpec
NativeToolCall
NativeTurnResult
```

## Agent runtime usage

```rust
use sandevistan_core::{AgentRuntime, AgentRuntimeConfig};

let result = AgentRuntime::new().run(AgentRuntimeConfig {
    session_id,
    messages,
    mods,
    prompt_config,
    summary: None,
    system_prompt: None,
    provider,
    read_only: false,
    delegate_depth_remaining: 2,
    budgets,
    cancellation_token,
    tool_host,
    on_event,
}).await?;
```

`tool_host` is your app adapter. It supplies tool specs, system prompt additions, and executes tool calls.

## Provider usage

```rust
use sandevistan_core::{provider, ChatMessage, ProviderConfig};

let config = ProviderConfig {
    kind: "openai".into(),
    api_base: "https://api.openai.com/v1".into(),
    api_key: Some("...".into()),
    api_key_header: "Authorization".into(),
    model_id: "gpt-4o-mini".into(),
};

let text = provider::complete(config, vec![
    ChatMessage { role: "user".into(), content: "hello".into() }
]).await?;
```

## Streaming/tool-call usage

```rust
provider::complete_native_stream(
    config,
    messages,
    tool_specs,
    cancellation_token,
    |event| {
        // NativeStreamEvent::TextDelta(...)
    },
).await?;
```

## Current status

This is the first separation step.

Already moved:

- full `AgentRuntime` loop
- provider
- wire types
- chat message
- provider config
- agent mods/subagent definitions
- portable tool host trait
- context budgeter
- cancellation/events/budgets
- internal agent message conversion/types

Still app-side by design:

- built-in workspace tool implementation
- MCP/extension adapters
- session/config management
- Tauri event bridge

To reuse this crate elsewhere, implement `ToolHost`, provide `ProviderConfig`, then call `AgentRuntime::run`.
