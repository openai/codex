## Overview
`core::tools::handlers::view_image` attaches a local image to the conversation so downstream models receive it as `UserInput::LocalImage`. It validates file existence, injects the image into the session input queue, and emits a progress event.

## Detailed Behavior
- Accepts `ToolPayload::Function` with JSON `{ path }`. Other payloads return model-facing errors.
- Resolves the supplied path against the turn’s cwd, checks that the file exists and is a regular file via `tokio::fs::metadata`, and surfaces descriptive errors when checks fail.
- Calls `Session::inject_input` with a `UserInput::LocalImage` referencing the absolute path. If no task is active, the handler returns an error prompting the model to wait.
- Emits `EventMsg::ViewImageToolCall` with the call ID and path so clients can display the attachment.
- Returns `ToolOutput::Function { content: "attached local image path", success: Some(true) }`, signaling success to the model.

## Broader Context
- The view-image tool is optional and exposed via feature flags. It complements tools that rely on local context (e.g., screenshot viewers) by ensuring binary data is staged for the next turn.
- Injecting input allows the conversation loop to treat the image like any other user input in subsequent prompts.
- Context can't yet be determined for remote URIs or data URIs; current logic targets local filesystem paths only.

## Technical Debt
- None noted.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.rs.spec.md
  - ../spec.rs.spec.md
  - ../../mod.spec.md
