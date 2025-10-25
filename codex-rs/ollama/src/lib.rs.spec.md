## Overview
`lib.rs` ties together the Ollama integration. It re-exports the client and progress reporters, defines the default OSS model string, and provides `ensure_oss_ready` to verify the local environment before running Codex in OSS mode.

## Detailed Behavior
- Module wiring: private modules (`client`, `parser`, `pull`, `url`) and public re-exports (`OllamaClient`, `PullEvent`, progress reporters).
- `DEFAULT_OSS_MODEL` identifies the built-in open-source model pulled when `--oss` is used without `-m`.
- `ensure_oss_ready`:
  - Looks up the OSS provider configuration from `Config` (respecting overrides).
  - Constructs an `OllamaClient` (probing the server with `try_from_oss_provider`).
  - Calls `fetch_models`; if the desired model is missing, pulls it using `CliProgressReporter`.
  - Logs warnings but does not fail hard when model listing fails, allowing higher layers to surface errors later.

## Broader Context
- Invoked by Codex CLI/TUI before running OSS workflows to ensure the required model is available locally.

## Technical Debt
- None.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./client.rs.spec.md
  - ./pull.rs.spec.md
