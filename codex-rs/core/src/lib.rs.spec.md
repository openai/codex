## Overview
`core::lib` wires together the `codex-core` crate. It enforces global lint policy, declares each submodule, and re-exports the entry points and helper types that other crates use to drive the agent. No business logic lives here; the file serves as the central router for the crate’s public API.

## Detailed Behavior
- Denies `clippy::print_stdout` and `clippy::print_stderr` at the crate root so library code cannot emit user-visible output directly. Front-end crates must surface messages through their own abstractions.
- Declares and exposes modules that span configuration (`config`, `config_loader`, `config_edit`, `config_profile`, `config_types`), execution (`exec`, `exec_env`, `shell`, `bash`, `spawn`, `seatbelt`, `sandboxing`, `unified_exec`), client plumbing (`client`, `client_common`, `model_provider_info`, `default_client`, `project_doc`, `openai_model_info`), and orchestration (`codex`, `codex_conversation`, `conversation_manager`, `conversation_history`, `tasks`, `state`, `turn_diff_tracker`).
- Re-exports frequently used types and helpers to simplify downstream import paths: model provider catalog (`ModelProviderInfo`, `built_in_model_providers`), conversation management (`ConversationManager`, `CodexConversation`), protocol aliases (`codex_protocol::protocol`, `codex_protocol::config_types`), safety helpers (`is_safe_command`, `get_platform_sandbox`), and diff/rollout utilities.
- Centralizes tool integrations by exposing modules like `tools`, `function_tool`, `apply_patch`, `token_data`, `review_format`, and `util`.
- Provides direct access to prompt helpers (`client_common::Prompt`, `REVIEW_PROMPT`, `content_items_to_text`) and protocol model types needed by caller crates (`ContentItem`, `ResponseItem`, `LocalShellExecAction`, etc.).

## Broader Context
- Downstream crates treat `codex-core` as the façade into the agent runtime. Maintaining coherent re-exports here minimizes churn when internal modules move, and it keeps CLI/TUI code focused on orchestration rather than wiring.
- The lint guard reflects an architectural commitment to route all user-visible output through higher-level presentation layers (TUI, logging). Specs for those layers should reference this constraint to explain why helper functions return structured events instead of printing.
- Context can't yet be determined for feature gating at the lib level; if future consumers require subset builds, this file may need targeted `#[cfg]` blocks to hide implementation details.

## Technical Debt
- The list of `mod` declarations is extensive and unsorted; adopting a documented grouping (e.g., configuration, execution, tooling) would improve discoverability and make future refactors less error-prone.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Organize module declarations/re-exports into logical sections or apply doc comments to clarify groupings for maintainers.
related_specs:
  - ../mod.spec.md
  - ./codex.rs.spec.md
  - ./codex_conversation.rs.spec.md
  - ./conversation_manager.rs.spec.md
  - ./exec/mod.spec.md
