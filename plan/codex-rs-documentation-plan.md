# Codex-rs Documentation Plan

## Goals
- Establish a repeatable process to add per-file specifications (`*.spec.md`) alongside the Rust source in `codex-rs`.
- Prioritize documentation work so the highest-impact crates and modules are covered first.
- Define how plan files will be organized, linked, and aggregated into a final documentation index.

## Scope & Assumptions
- Target all Rust source files under `codex-rs`, focusing on `.rs` and relevant `.sbpl` policies. Snapshot outputs, generated assets, and non-Rust tooling (e.g., `.snap`, `.toml`, build scripts) are tracked but only documented when they materially affect runtime behavior.
- Seatbelt and sandbox environment guards (`CODEX_SANDBOX*`) remain untouched; note their behavior in specs when encountered.
- Crate-level README content in `codex-rs/docs/` is updated whenever new public APIs or behaviors are documented.
- Plan/spec files live next to the source they describe using the convention `<original_filename>.spec.md`. For directory-level overviews, use `mod.spec.md`.
- A top-level index at `docs/code-specs/README.md` (to be created later) will link to every spec file and summarize coverage status.

## Inventory & Grouping
| Tier | Crates / Areas | Rationale |
| --- | --- | --- |
| 0. Foundational Types | `common`, `protocol`, `protocol-ts`, `codex-backend-openapi-models`, `mcp-types` | Shared data models and serialization contracts; specs unblock downstream crates. |
| 1. Core Orchestration | `core`, `common`, `protocol`, `exec`, `linux-sandbox`, `process-hardening`, `execpolicy` | Dispatch, execution policies, and sandboxing define primary runtime behavior. |
| 2. User Interfaces & Entry Points | `cli`, `tui`, `app-server`, `mcp-server`, `login`, `responses-api-proxy` | Surfaces Codex to users and integrations; specs clarify UX flows. |
| 3. Tooling & Integrations | `file-search`, `apply-patch`, `git-tooling`, `git-apply`, `backend-client`, `cloud-tasks`, `rmcp-client`, `ollama`, `feedback`, `otel`, `async-utils`, `ansi-escape`, `stdio-to-uds` | Support utilities and external service adapters. |
| 4. Utilities & Scripts | `utils/*`, `code`, `app-server-protocol`, `chatgpt`, `arg0`, `process-hardening`, `linux-sandbox` (tests), `ansi-escape` | Helpers, conversions, and lower-level adapters to be documented after main flows. |

## Documentation Order & File Breakdown
The following phases outline the order specs will be written. Within each phase, work proceeds top-down: crate overview (`lib.rs` or `main.rs`), primary modules, supporting modules, tests, then fixtures. For large modules, document entrypoint first, followed by subordinate files in dependency order.

### Phase 0 – Workspace Foundations
1. `common/src/lib.rs`, followed by configuration helpers (`config_summary.rs`, `approval_presets.rs`, `model_presets.rs`, `config_override.rs`, `format_env_display.rs`, `elapsed.rs`, `sandbox_mode_cli_arg.rs`, `approval_mode_cli_arg.rs`).
2. `protocol/src/lib.rs`, core data structures (`protocol.rs`, `models.rs`, `plan_tool.rs`, `items.rs`, `message_history.rs`, `user_input.rs`, `custom_prompts.rs`, `config_types.rs`, `parse_command.rs`, `account.rs`, `conversation_id.rs`, `num_format.rs`).
3. `mcp-types/src/lib.rs` and immediate model files; ensure alignment with `protocol`.
4. `codex-backend-openapi-models/src/lib.rs` and generated schema modules (document generation source & update process rather than line-by-line behavior).
5. `protocol-ts` overview (`src/lib.rs`, module entrypoints) with emphasis on cross-language schema parity.

### Phase 1 – Core Orchestration & Execution
1. `core/src/lib.rs` → `core/src/codex.rs` → `core/src/codex_conversation.rs` → conversation/state management (`conversation_manager.rs`, `conversation_history.rs`, `state/mod.rs`, `state/session.rs`, `state/service.rs`, `state/turn.rs`, `turn_diff_tracker.rs`, `message_history.rs`).
2. Task orchestration (`tasks/mod.rs`, `tasks/regular.rs`, `tasks/review.rs`, `tasks/compact.rs`, `project_doc.rs`).
3. Tooling hub (`tools/mod.rs`, `registry.rs`, `router.rs`, `context.rs`, `orchestrator.rs`, `parallel.rs`, `events.rs`, `spec.rs`, `sandboxing.rs`).
4. Execution and sandboxing pipeline (`exec.rs`, `bash.rs`, `shell.rs`, `unified_exec/mod.rs`, `unified_exec/session_manager.rs`, `unified_exec/session.rs`, `unified_exec/errors.rs`, `exec_env.rs`, `spawn.rs`, `seatbelt.rs`, `seatbelt_base_policy.sbpl`, `landlock.rs`).
5. Safety and policy (`command_safety/mod.rs`, `command_safety/is_safe_command.rs`, `command_safety/is_dangerous_command.rs`, `command_safety/windows_safe_commands.rs`, `safety.rs`, `rollout/mod.rs`, `rollout/policy.rs`, `rollout/list.rs`, `rollout/recorder.rs`, `rollout/tests.rs`).
6. Configuration and auth (`config.rs`, `config_edit.rs`, `config_profile.rs`, `config_loader/mod.rs`, `config_loader/macos.rs`, `config_types.rs`, `features.rs`, `features/legacy.rs`, `auth.rs`, `mcp/auth.rs`, `mcp/mod.rs`, `mcp_connection_manager.rs`).
7. Client plumbing and telemetry (`client.rs`, `client_common.rs`, `default_client.rs`, `chat_completions.rs`, `openai_model_info.rs`, `model_provider_info.rs`, `model_family.rs`, `function_tool.rs`, `environment_context.rs`, `token_data.rs`, `git_info.rs`, `otel_init.rs`, `review_format.rs`, `user_notification.rs`, `user_instructions.rs`, `event_mapping.rs`, `custom_prompts.rs`, `util.rs`, `parse_command.rs`, `truncate.rs`).
8. Gateways (`exec/src/lib.rs`, `exec/src/main.rs`, `exec/src/config.rs` if present) and sandbox support crates (`linux-sandbox/src/lib.rs`, `linux-sandbox/src/policy/*.rs`, `process-hardening/src/lib.rs`, `process-hardening/src/seccomp.rs`, `execpolicy/src/lib.rs`).

### Phase 2 – Interfaces & Services
1. CLI (`cli/src/main.rs`, `cli/src/lib.rs`, `cli/src/login.rs`, `cli/src/mcp_cmd.rs`, `cli/src/debug_sandbox.rs`, `cli/src/exit_status.rs`).
2. TUI (top-level `src/lib.rs`, `src/app.rs`, `src/tui.rs`, `src/app_event.rs`), then modules grouped by feature: rendering (`render/mod.rs`, `render/renderable.rs`, `render/line_utils.rs`, `render/highlight.rs`), widgets (`chatwidget/*.rs`, `status/*.rs`, `bottom_pane/*.rs`, `public_widgets/*.rs`, `exec_cell/*.rs`), helpers (`wrapping.rs`, `text_formatting.rs`, `color.rs`, `terminal_palette.rs`, `live_wrap.rs`, `diff_render.rs`, `markdown.rs`, `markdown_render.rs`, `app_event_sender.rs`, `session_log.rs`, `key_hint.rs`, `resume_picker.rs`, `get_git_diff.rs`, `app_backtrack.rs`, `updates.rs`, `update_prompt.rs`, `slash_command.rs`, `selection_list.rs`, `shimmer.rs`, `ascii_animation.rs`, `ui_consts.rs`, `custom_terminal.rs`, `clipboard_paste.rs`, `insert_history.rs`, `pager_overlay.rs`, onboarding suite). Snapshot documentation: include a single spec per snapshot directory describing update workflow rather than per `.snap` file.
3. Servers (`app-server/src/main.rs`, `app-server/src/lib.rs`, authentication & SSE modules, tests; `app-server-protocol/src/*.rs`), `responses-api-proxy`, `mcp-server/src/main.rs`, `mcp-server/src/lib.rs`, supporting modules including `tests/common`.
4. Authentication/login (`login/src/*.rs`), capturing OAuth and credential storage flows.

### Phase 3 – Tools & External Integrations
1. File & git tooling (`file-search/src/*.rs`, `git-tooling/src/*.rs`, `git-apply/src/*.rs`, `apply-patch/src/*.rs`).
2. Client libraries (`backend-client/src/*.rs`, `cloud-tasks/src/*.rs`, `cloud-tasks-client/src/*.rs`, `rmcp-client/src/*.rs`, `ollama/src/*.rs`, `responses-api-proxy/src/*.rs`).
3. Feedback & analytics (`feedback/src/*.rs`, `otel/src/*.rs`, `ansi-escape/src/*.rs`).
4. Async utilities (`async-utils/src/*.rs`, `arg0/src/*.rs`, `stdio-to-uds/src/*.rs`), documenting concurrency primitives and IPC helpers.

### Phase 4 – Utilities, Tests, and Scripts
1. `utils/*` crates (`json-to-toml`, `string`, `tokenizer`, `pty`, `readiness`): document conversions, wrappers, and readiness checks.
2. `code/`, `chatgpt/`, `app-server/tests/common`, `core/tests/common`, `mcp-server/tests/common`, capturing test harness utilities.
3. script crates (`ansi-escape`, `process-hardening` test harness) and any remaining binaries under `src/bin`.

## Workflow Per File
1. **Analyze context**: trace module exports/imports, note upstream dependencies, identify external services, and record invariants.
2. **Draft spec**: capture overview, responsibilities, key functions/types, call flow, error handling, sandbox considerations, and TODO/tech debt.
3. **Apply canonical outline**: structure each spec with sections `## Overview`, `## Detailed Behavior`, `## Broader Context`, and `## Technical Debt`. When broader context remains unknown, record `Context can't yet be determined` as a placeholder to revisit in later passes.
4. **Cross-link**: link to related specs using relative paths (e.g., `[State Service](../../state/service.rs.spec.md)`).
5. **Context compaction checkpoint**: after finishing all files within a directory or module (i.e., after producing or updating its `mod.spec.md`), pause to hand control back to the user so they can run an updated compaction pass. Resume with the next sibling directory/module only after the user confirms the context is refreshed. Within a submodule walk (from entry file through its children) continue uninterrupted to maintain local continuity.
6. **Review & update index**: add entry to `docs/code-specs/README.md` once spec is merged; flag open questions for follow-up.

## Plan File Layout & Tracking
- For each source file: create `<filename>.spec.md` alongside it. Directory overviews use `mod.spec.md`.
- Include a YAML front matter block in each spec with fields `status`, `owner`, `last_updated`, `tech_debt`, and `related_specs`.
- Maintain hierarchical links: the top-level index points to each crate's `mod.spec.md`, and every module spec links onward to its child file specs.
- Track completion in `docs/code-specs/README.md` with a checklist organized by crate and phase.
- For crates with generated code, create a single `generated.spec.md` describing regeneration steps and boundaries.

## Final Documentation Deliverables
- `docs/code-specs/README.md`: global index, completion metrics, links to specs, and guidance for contributors.
- Per-crate overview pages under `docs/code-specs/<crate>/overview.md`, created once ≥50% of the crate's files have specs.
- Quarterly review issue template referencing this plan to reassess priorities and record drift.

## Next Steps
1. Circulate this plan with maintainers for feedback.
2. Create skeleton `docs/code-specs/README.md` and per-crate subdirectories before authoring individual specs.
3. Kick off Phase 0 by documenting `common/src/lib.rs` and `protocol/src/lib.rs`, updating the index accordingly.
4. Establish lightweight automation (e.g., `just docs-check`) later to ensure new source files ship with companion specs.
