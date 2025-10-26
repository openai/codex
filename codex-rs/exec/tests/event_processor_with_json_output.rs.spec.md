## Overview
`event_processor_with_json_output` unit-tests the translation between core execution events and the JSONL thread event schema produced by `EventProcessorWithJsonOutput`.

## Detailed Behavior
- Helper `event(id, msg)` constructs protocol events reused across tests.
- Verifies mapping logic for:
  - Session/task lifecycle (`SessionConfigured` → `ThreadStarted`, `TaskStarted` → `TurnStarted`).
  - Web search completions, plan updates, and plan clear operations generating TODO list events.
  - Tool call flows (shell, custom tools, MCP) producing `ItemStarted`, `ItemUpdated`, and `ItemCompleted` entries with appropriate statuses.
  - Agent/resoning messages and error events.
  - Apply-patch begin/end pairs populating `PatchChangeKind` metadata and failure scenarios.
  - Token usage roll-ups (`TaskComplete` → `TurnCompleted` with usage snapshot).
- Includes regression checks for thread errors, TODO list pruning, agent summary events, and MCP call output consolidation.
- Exercises serialization of patch change content, reasoning traces, and custom tool call payloads to ensure the JSON view remains stable.

## Broader Context
- Protects the CLI’s JSON output contract described in Phase 1, ensuring downstream consumers (e.g., UI or logging tooling) receive consistent event streams.

## Technical Debt
- File is extensive and mixes many independent assertions; breaking it into thematic modules could improve readability, but the current layout keeps all JSONL expectations in one place.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider splitting tests by event category (tools, reasoning, usage) for faster navigation as the schema evolves.
related_specs:
  - ./mod.spec.md
  - ../src/event_processor_with_jsonl_output.rs.spec.md
