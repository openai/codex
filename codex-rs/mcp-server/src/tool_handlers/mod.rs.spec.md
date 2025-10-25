## Overview
The `tool_handlers` module namespaces the MCP-facing tool handlers that wrap Codex conversations. It currently exposes submodules for creating conversations and sending follow-up messages.

## Detailed Behavior
- Re-exports two handler modules:
  - `create_conversation` – expected to launch new Codex sessions when the MCP client invokes the `codex` tool.
  - `send_message` – expected to relay additional prompts into an existing Codex conversation.
- The implementations live in sibling files and plug into the MCP message processor when handling `tools/call` requests.

## Broader Context
- Context can't yet be determined; the handler implementations are not present in this snapshot, so their interaction with the MCP router remains unknown.

## Technical Debt
- Document and audit the underlying handler modules once they are restored to the repository.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Restore or implement the `create_conversation` and `send_message` tool handlers so the module exports work as intended.
related_specs:
  - ./codex_tool_runner.rs.spec.md
  - ./message_processor.rs.spec.md
