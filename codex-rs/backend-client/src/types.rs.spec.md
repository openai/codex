## Overview
`types.rs` wraps the generated backend models with hand-rolled structures and convenience traits. It focuses on task detail payloads returned by Cloud Tasks/WHAM endpoints, offering helpers to extract diffs, messages, prompts, and errors without wading through raw JSON.

## Detailed Behavior
- Re-exports select OpenAPI-generated types for rate limits and task listings (`PaginatedListTaskListItem`, `TaskListItem`, `PlanType`, `RateLimitStatusPayload`, etc.) so callers don’t import the generated crate directly.
- Hand-authored models:
  - `CodeTaskDetailsResponse` and nested `Turn`, `TurnItem`, `ContentFragment`, `Worklog`, `TurnError` mirror the backend task-details schema with `serde` defaults to tolerate missing fields.
  - Deserialization helpers (`deserialize_vec`) ensure arrays default to empty vectors rather than `null`.
- `ContentFragment::text` inspects structured/text fragments, honoring `content_type == "text"` for structured payloads.
- `TurnItem` helpers gather text/diff content (`text_values`, `diff_text`), supporting different item kinds (`message`, `output_diff`, `pr`).
- `Turn` utilities:
  - `unified_diff` prioritizes diff turn output but falls back to assistant PR diff payloads.
  - `message_texts` aggregates assistant responses from output items and worklog messages authored by the assistant.
  - `user_prompt` stitches user message parts together with blank lines.
  - `error_summary` formats error code/message combinations.
- `WorklogMessage` and `TurnError` helper methods support the extraction routines above.
- `CodeTaskDetailsResponseExt` trait exposes four high-level accessors: unified diff text, assistant messages, user prompt, and assistant error string.
- `TurnAttemptsSiblingTurnsResponse` is a thin wrapper around the sibling turns payload, keeping raw JSON maps to avoid overfitting the schema.
- Tests ensure fixtures deserialize correctly and helper methods return expected diffs, prompts, and errors.

## Broader Context
- Consumed by tooling that renders task history in the CLI/TUI and surfaces errors/diffs to users.
- Complements `Client` methods: `get_task_details` returns `CodeTaskDetailsResponse`, enabling immediate use of the extension trait to project meaningful fields.

## Technical Debt
- Structs are partially hand-rolled until the OpenAPI models are improved; long term we may replace these shims with fully custom models.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Replace the partial hand-rolled task detail structs with purpose-built models once backend schemas stabilize.
related_specs:
  - ../mod.spec.md
  - ./client.rs.spec.md
