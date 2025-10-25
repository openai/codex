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
Phase 3 is the next active focus. Before opening new crates, sweep remaining Phase 1/2 stragglers so upstream docs stay consistent:
- `core`: add specs for `command_safety/mod.rs`, `error.rs`, `flags.rs`, `mcp/mod.rs`, `mcp/auth.rs`, `mcp_connection_manager.rs`, `message_history.rs`, `safety.rs`, and `terminal.rs`.
- `app-server`: cover `main.rs`, `codex_message_processor.rs`, `models.rs`, `outgoing_message.rs`, `error_code.rs`, and `fuzzy_file_search.rs`.
- `mcp-server`: cover `main.rs`, `lib.rs`, `codex_tool_runner.rs`, `codex_tool_config.rs`, `exec_approval.rs`, `patch_approval.rs`, `tool_handlers/mod.rs`, `outgoing_message.rs`, and `error_code.rs`.

After the sweep, proceed crate-by-crate with the following order, pausing for a context compaction checkpoint after each crate-level `mod.spec.md` is finalized.

#### 3A. File & Git Tooling
1. `file-search`: `mod.spec.md`, `lib.rs`, `cli.rs`, `main.rs` — document search pipeline, filters, and CLI wiring. ✅ Completed; crate specs now committed locally.
2. `git-tooling`: `mod.spec.md`, `lib.rs`, `operations.rs`, `ghost_commits.rs`, `errors.rs`, `platform.rs` — emphasize safety guards around git mutations.
3. `git-apply`: `mod.spec.md`, `lib.rs` — describe patch application and reconciliation helpers.
4. `apply-patch`: `mod.spec.md`, `lib.rs`, `main.rs`, `parser.rs`, `seek_sequence.rs`, `standalone_executable.rs` — include how this crate cooperates with `core::tools::handlers::apply_patch`.

#### 3B. Client Libraries & Proxies
1. `backend-client`: `mod.spec.md`, `lib.rs`, `client.rs`, `types.rs` — outline request flow to backend services. ✅ Completed.
2. `cloud-tasks`: `mod.spec.md`, `lib.rs`, `app.rs`, `cli.rs`, `new_task.rs`, `scrollable_diff.rs`, `ui.rs`, `util.rs`, `env_detect.rs`. ✅ Completed.
3. `cloud-tasks-client`: `mod.spec.md`, `lib.rs`, `api.rs`, `http.rs`, `mock.rs` — clarify local vs. remote execution paths. ✅ Completed.
4. `responses-api-proxy`: add `mod.spec.md`, `lib.rs.spec.md`, `read_api_key.rs.spec.md`, and reconcile with existing `main.rs` spec. ✅ Completed.
5. `rmcp-client`: `mod.spec.md`, `lib.rs`, `rmcp_client.rs`, `perform_oauth_login.rs`, `oauth.rs`, `auth_status.rs`, `logging_client_handler.rs`, `utils.rs`, `find_codex_home.rs`. ✅ Completed.
6. `ollama`: `mod.spec.md`, `lib.rs`, `client.rs`, `parser.rs`, `pull.rs`, `url.rs`. ✅ Completed.

#### 3C. Feedback & Telemetry
1. `feedback`: `mod.spec.md`, `lib.rs`. ✅ Completed.
2. `otel`: `mod.spec.md`, `lib.rs`, `config.rs`, `otel_provider.rs`, `otel_event_manager.rs`. ✅ Completed.
3. `ansi-escape`: `mod.spec.md`, `lib.rs` — focus on terminal formatting helpers reused across crates. ✅ Completed.

#### 3D. Async & IPC Utilities
1. `async-utils`: `mod.spec.md`, `lib.rs`. ✅ Completed.
2. `arg0`: `mod.spec.md`, `lib.rs`. ✅ Completed.
3. `stdio-to-uds`: `mod.spec.md`, `lib.rs`, `main.rs` — capture IPC bridging and safety checks. ✅ Completed.

### Phase 4 – Utilities, Tests, and Scripts
1. `utils/*` crates (`json-to-toml`, `string`, `tokenizer`, `pty`, `readiness`): document conversions, wrappers, and readiness checks.
2. `code/`, `chatgpt/`, `app-server/tests/common`, `core/tests/common`, `mcp-server/tests/common`, capturing test harness utilities.
3. script crates (`ansi-escape`, `process-hardening` test harness) and any remaining binaries under `src/bin`.

## Progress Snapshot
- **Phase 0 – Workspace Foundations:** Complete. Specs exist for all `common`, `protocol`, `mcp-types`, `codex-backend-openapi-models`, and `protocol-ts` modules identified in the phase outline.
- **Phase 1 – Core Orchestration & Execution:** Core conversation flow, tooling orchestration, execution, and configuration stacks are documented. Remaining files: `command_safety/mod.rs`, `error.rs`, `flags.rs`, `mcp/auth.rs`, `mcp/mod.rs`, `mcp_connection_manager.rs`, `message_history.rs`, `safety.rs`, `terminal.rs`.
- **Phase 2 – Interfaces & Services:** CLI, TUI, and initial service entrypoints (`responses-api-proxy/main.rs`, `app-server/lib.rs`, `app-server/message_processor.rs`, `mcp-server/message_processor.rs`) are covered. Outstanding modules slated for the Phase 3 sweep: remaining `app-server` and `mcp-server` internals plus supporting response helpers.
- **Phase 3 – Tools & External Integrations:** File search crate documented; git tooling, git-apply, and apply-patch specs underway next.

## Workflow Per File
1. **Analyze context**: trace module exports/imports, note upstream dependencies, identify external services, and record invariants.
2. **Draft spec**: capture overview, responsibilities, key functions/types, call flow, error handling, sandbox considerations, and TODO/tech debt.
3. **Apply canonical outline**: structure each spec with sections `## Overview`, `## Detailed Behavior`, `## Broader Context`, and `## Technical Debt`. When broader context remains unknown, record `Context can't yet be determined` as a placeholder to revisit in later passes.
4. **Footer metadata**: append YAML at the end of the file with `tech_debt` (containing `severity` and `highest_priority_items`) and `related_specs`. Use `low`/`medium`/`high` severity. `related_specs` should reference the parent `mod.spec.md` plus any known siblings or children; omit comments when unknown.
5. **Cross-link**: link to related specs using relative paths (e.g., `[State Service](../../state/service.rs.spec.md)`).
6. **Context compaction checkpoint**: after finishing all files within a directory or module (i.e., after producing or updating its `mod.spec.md`), pause to hand control back to the user so they can run an updated compaction pass. Resume with the next sibling directory/module only after the user confirms the context is refreshed. Within a submodule walk (from entry file through its children) continue uninterrupted to maintain local continuity.
7. **Review & update index**: add entry to `docs/code-specs/README.md` once spec is merged; flag open questions for follow-up.

## Plan File Layout & Tracking
- For each source file: create `<filename>.spec.md` alongside it. Directory overviews use `mod.spec.md`.
- Append footer YAML to each spec instead of front matter. Only include `tech_debt` (with `severity` and `highest_priority_items`) and `related_specs` as described above.
- Maintain hierarchical links: the top-level index points to each crate's `mod.spec.md`, and every module spec links onward to its child file specs.
- Track completion in `docs/code-specs/README.md` with a checklist organized by crate and phase.
- For crates with generated code, create a single `generated.spec.md` describing regeneration steps and boundaries.

## Final Documentation Deliverables
- `docs/code-specs/README.md`: global index, completion metrics, links to specs, and guidance for contributors.
- Per-crate overview pages under `docs/code-specs/<crate>/overview.md`, created once ≥50% of the crate's files have specs.
- Quarterly review issue template referencing this plan to reassess priorities and record drift.

## Next Steps
1. Confirm with maintainers that the Phase 1/2 straggler sweep ordering matches expectations; adjust if additional files surface.
2. Document the remaining `core`, `app-server`, and `mcp-server` modules listed under Phase 3 and pause for a context compaction checkpoint.
3. Start the Phase 3 crate cadence with `file-search`, producing the crate `mod.spec.md` plus specs for `lib.rs`, `cli.rs`, and `main.rs`.
4. Continue through the Git tooling crates (`git-tooling`, `git-apply`, `apply-patch`), coordinating compaction checkpoints between crates.
5. Reassess plan readiness for Phase 3B (client libraries) once the Git tooling sweep is complete and update this document before proceeding.
