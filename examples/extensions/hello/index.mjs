#!/usr/bin/env node
import { readFileSync } from "node:fs";

const request = JSON.parse(readFileSync(0, "utf8"));

if (request.method === "initialize") {
  console.log(JSON.stringify({
    tools: [{
      name: "greet",
      description: "Greet someone by name",
      parameters: {
        type: "object",
        properties: { name: { type: "string" } },
        required: ["name"],
        additionalProperties: false
      }
    }]
  }));
  process.exit(0);
}

if (request.method === "tool.execute") {
  const name = request.tool_call?.args?.name ?? "world";
  console.log(JSON.stringify({ content: `status: ok\nhello ${name}` }));
  process.exit(0);
}

if (request.method === "hook") {
  if (request.event?.type === "before_model_call") {
    console.log(JSON.stringify({ decisions: [{ action: "append_system_context", content: "Hello extension active." }] }));
    process.exit(0);
  }
  if (request.event?.type === "before_tool_call" && request.event.tool === "shell.run") {
    const command = request.event.args?.command ?? "";
    if (command.includes("rm -rf")) {
      console.log(JSON.stringify({ decisions: [{ action: "block", reason: "hello extension blocked rm -rf" }] }));
      process.exit(0);
    }
  }
}

console.log(JSON.stringify({ decisions: [] }));
