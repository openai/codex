## Overview
`pull.rs` defines the streaming events emitted while downloading a model from Ollama and provides progress reporters for CLI/TUI outputs.

## Detailed Behavior
- `PullEvent` variants:
  - `Status(String)` for human-readable status messages.
  - `ChunkProgress { digest, total, completed }` for per-layer byte progress.
  - `Success` indicates the pull finished.
  - `Error(String)` captures error messages from the stream.
- `PullProgressReporter` trait allows different UIs to react to events.
- `CliProgressReporter`:
  - Tracks totals per digest and renders inline progress on stderr.
  - Suppresses noisy manifest messages, prints total size once, and shows throughput in MB/s.
  - After `Success`, prints a newline; `Error` is left for callers to handle.
- `TuiProgressReporter` currently delegates to the CLI reporter to keep behavior aligned until a dedicated TUI integration exists.

## Broader Context
- Used by `OllamaClient::pull_with_reporter` to surface download progress to users in both CLI and TUI contexts.

## Technical Debt
- Future work may provide a richer TUI implementation; current delegation keeps code simple.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./client.rs.spec.md
