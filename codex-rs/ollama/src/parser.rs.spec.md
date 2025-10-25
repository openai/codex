## Overview
`parser.rs` converts JSON objects emitted by the Ollama pull API into `PullEvent`s, enabling streaming progress updates during downloads.

## Detailed Behavior
- `pull_events_from_value` examines a `serde_json::Value` and emits:
  - `PullEvent::Status` for the `"status"` field, plus `PullEvent::Success` when the status equals `"success"`.
  - `PullEvent::ChunkProgress` when `"digest"`, `"total"`, or `"completed"` fields are present.
  - Any combination of events found in a single JSON object.
- Unit tests verify status/success handling and chunk-progress decoding.

## Broader Context
- Used by `OllamaClient::pull_model_stream` to translate the streaming JSON lines returned by `/api/pull` into higher-level events consumed by progress reporters.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./pull.rs.spec.md
