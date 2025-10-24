## Overview
`codex-core` orchestrates the Codex agent runtime. It coordinates model clients, tool execution, sandbox enforcement, conversation state, and rollout telemetry for both interactive shells and service entrypoints. The crate exposes APIs consumed by the CLI, TUI, MCP server, and automation flows, making it the central hub where protocol messages turn into actions.

## Detailed Behavior
- `src/lib.rs` re-exports dozens of submodules that cover configuration management (`config`, `config_loader`, `config_profile`), client plumbing (`client`, `client_common`, `model_provider_info`, `default_client`), task orchestration (`codex`, `tasks`, `turn_diff_tracker`), tool routing (`tools`, `function_tool`, `exec`, `unified_exec`, `sandboxing`, `seatbelt`), safety (`command_safety`, `safety`, `rollout`), and UX integration (`project_doc`, `review_format`, `user_notification`, `terminal`).
- Conversation lifecycle is managed through `codex_conversation`, `conversation_manager`, `conversation_history`, and `state/*`. These modules maintain turn history, apply diff tracking, and surface metrics for downstream consumers.
- Command execution flows traverse `exec`, `exec_env`, `spawn`, `shell`, `bash`, and `landlock`, which wrap the underlying sandbox primitives while honoring approval and rollout policies.
- Feature gating lives under `features/*`, allowing consumers to toggle experimental capabilities or maintain compatibility with restricted environments.
- The crate also hosts integration with external services (OpenAI, MCP) via `chat_completions`, `mcp`, `mcp_connection_manager`, `mcp_tool_call`, and compliance utilities such as `approval_presets`.

## Broader Context
- Many workspace crates call into `codex-core`; changes here ripple across CLI/TUI behavior, backend services, and documentation. Specifications for sibling crates (e.g., `codex-protocol`, `codex-common`) describe shared data types imported by this crate.
- Because `codex-core` re-exports `codex-protocol` types, it provides a compatibility layer for legacy call sites that previously accessed protocol definitions directly. Maintaining those re-exports is important for churn-free refactors.
- Context can't yet be determined for long-term modularization; this documentation pass will proceed submodule-by-submodule as outlined in the workspace plan, pausing for user compaction checkpoints after major directory sweeps.

## Technical Debt
- The crate’s breadth makes navigation challenging; as specs progress, consider grouping modules into documented domains (execution, configuration, conversations) and surfacing a lightweight module index.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Produce a navigable module index (possibly in `docs/code-specs/core/overview.md`) once ≥50% of the crate is documented to help contributors locate specs quickly.
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/codex.spec.md
  - ./src/codex_conversation.spec.md
  - ./src/tasks/mod.spec.md
