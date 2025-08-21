### Overview
codex-custom is a Rust workspace of multiple crates that together implement the Codex CLI: core agent logic, interactive TUI, programmatic “exec” mode, sandboxing, MCP client/server, OSS model plumbing, and helper utilities.

### Workspace root
- `codex-custom/Cargo.toml`: Declares the workspace members, global lints, release profile tweaks, and a patched `ratatui` dependency.
- `codex-custom/Cargo.lock`: Locked dependency graph for reproducible builds.
- `codex-custom/README.md`: Project overview, installation, features (e.g., TUI, `--cd`, approvals, images).
- `codex-custom/config.md`: Configuration reference (TOML) used by CLI/TUI.
- `codex-custom/default.nix`: Nix expression to build workspace in Nix environments.
- `codex-custom/docs/protocol_v1.md`: Protocol documentation for Codex’s stream/IPC.
- `codex-custom/justfile`: Command aliases for dev workflows (building, testing, etc.).
- `codex-custom/rust-toolchain.toml`: Pins Rust toolchain channel/targets.
- `codex-custom/rustfmt.toml`: Formatting rules.
- `codex-custom/scripts/create_github_release.sh`: Release helper script.

### Crate: ansi-escape
- `ansi-escape/Cargo.toml`: Crate manifest.
- `ansi-escape/README.md`: Crate-level documentation.
- `ansi-escape/src/lib.rs`: Utilities for emitting/parsing ANSI escape sequences used by the TUI rendering and status lines.

### Crate: apply-patch
- `apply-patch/Cargo.toml`: Manifest.
- `apply-patch/apply_patch_tool_instructions.md`: Tool instructions/usage doc.
- `apply-patch/src/lib.rs`: Public API to apply unified diffs/patches safely to files (used by “apply” operations).
- `apply-patch/src/parser.rs`: Patch format parsing and validation.
- `apply-patch/src/seek_sequence.rs`: Byte/sequence scanner to find insertion points efficiently.

### Crate: arg0
- `arg0/Cargo.toml`: Manifest.
- `arg0/src/lib.rs`: `arg0_dispatch_or_else` to conditionally dispatch runtime behavior based on the invoked executable name (supports multi-tool binaries like `codex-custom`, `codex`).

### Crate: chatgpt
- `chatgpt/Cargo.toml`: Manifest.
- `chatgpt/README.md`: Crate docs for ChatGPT-specific flows.
- `chatgpt/src/lib.rs`: Public module surface (`apply_command`, `get_task`).
- `chatgpt/src/apply_command.rs`: Implements the “Apply latest diff” command; reads agent-produced diffs and applies them to the working tree.
- `chatgpt/src/chatgpt_client.rs`: ChatGPT API client (auth, requests, retries).
- `chatgpt/src/chatgpt_token.rs`: Token discovery/refresh to talk to ChatGPT endpoints securely.
- `chatgpt/src/get_task.rs`: Pulls queued tasks/jobs for agent operation.
- `chatgpt/tests/apply_command_e2e.rs`: End-to-end tests for apply command behavior.
- `chatgpt/tests/task_turn_fixture.json`: Fixture of a task turn used by tests.

### Crate: cli (multitool entrypoint)
- `cli/Cargo.toml`: Manifest for `codex-custom` binary.
- `cli/src/main.rs`: Main entrypoint. Parses top-level CLI, dispatches to subcommands: interactive TUI, `exec`, `mcp`, `login/logout`, `proto`, `completion`, `debug`, `apply`. Bridges root-level config flags into subcommand-specific flags.
- `cli/src/lib.rs`: Shared CLI utilities used by subcommands.
- `cli/src/debug_sandbox.rs`: Runs arbitrary commands under macOS Seatbelt or Linux Landlock for debugging sandbox policy behavior.
- `cli/src/exit_status.rs`: Normalizes and renders process exit statuses.
- `cli/src/login.rs`: Interactive and API-key login flows; status and logout helpers.
- `cli/src/proto.rs`: Runs the protocol stream (stdin/stdout) mode for embedding Codex in other tools.

### Crate: common
- `common/Cargo.toml`: Manifest.
- `common/README.md`: Crate docs.
- `common/src/lib.rs`: Re-exports helpers used across binaries.
- `common/src/approval_mode_cli_arg.rs`: Clap arg parsing for approval policies.
- `common/src/config_override.rs`: Parsing key=value CLI overrides; merging order.
- `common/src/config_summary.rs`: Human-readable summaries of loaded config.
- `common/src/elapsed.rs`: Timer utilities for measuring durations.
- `common/src/fuzzy_match.rs`: Fuzzy string/file matching used by `@` search.
- `common/src/sandbox_mode_cli_arg.rs`: Clap arg parsing for sandbox modes.
- `common/src/sandbox_summary.rs`: Summarizes active sandbox settings.

### Crate: core (codex-core library)
- `core/Cargo.toml`: Manifest.
- `core/README.md`: Core library overview.
- `core/prompt.md`: Base/embedding prompts used by the agent.
- `core/src/lib.rs`: Root of the library. Exposes `Codex`, provider info, config, protocol types, spawn/shell APIs, sandbox helpers, etc. Forbids accidental stdout/stderr writes in library code.
- `core/src/apply_patch.rs`: Integration to apply patches within agent turn flow; exposes `CODEX_APPLY_PATCH_ARG1`.
- `core/src/bash.rs`: Shell quoting/escaping helpers; bash-specific utilities.
- `core/src/chat_completions.rs`: Model conversation assembly; streaming/accumulation of responses; tool-call plumbing.
- `core/src/client_common.rs`: Shared HTTP client setup (timeouts, headers).
- `core/src/client.rs`: High-level client that interacts with model providers.
- `core/src/codex.rs`: Main agent orchestration (turn loop, tool/exec decisions, approvals, persistence).
- `core/src/codex_wrapper.rs`: Wraps `Codex` to expose simplified lifecycle or alternate modes.
- `core/src/config.rs`: Loads/merges config files, env, CLI overrides; `find_codex_home`, log dirs.
- `core/src/config_profile.rs`: Profile selection and overrides by named profiles.
- `core/src/config_types.rs`: Strongly-typed config schema (e.g., `SandboxMode`, approvals).
- `core/src/conversation_history.rs`: Persistent conversation state and summarization.
- `core/src/error.rs`: Error types and conversions across crates.
- `core/src/exec_env.rs`: Execution environment setup (cwd, env vars).
- `core/src/exec.rs`: Safe command execution under sandbox; capturing output and exit codes.
- `core/src/flags.rs`: Feature flags/rollouts toggles used internally.
- `core/src/git_info.rs`: Git repo detection, current branch/dirty state, diffs.
- `core/src/is_safe_command.rs`: Command allow/deny heuristics to avoid dangerous ops.
- `core/src/mcp_connection_manager.rs`: Manages MCP client connections lifecycle.
- `core/src/mcp_tool_call.rs`: Executes MCP tool calls from model/tool messages.
- `core/src/message_history.rs`: Structured message storage (user, assistant, tool).
- `core/src/model_family.rs`: Model family taxonomy (OpenAI, OSS, etc.).
- `core/src/model_provider_info.rs`: Metadata and helpers for built-in providers; exports `built_in_model_providers`, `create_oss_provider_with_base_url`, `BUILT_IN_OSS_MODEL_PROVIDER_ID`.
- `core/src/models.rs`: Model catalog and default selection logic.
- `core/src/openai_model_info.rs`: OpenAI-specific model capabilities/limits.
- `core/src/openai_tools.rs`: Tool definitions and argument schemas for function calling.
- `core/src/parse_command.rs`: Parses natural-language to shell commands safely.
- `core/src/plan_tool.rs`: Implements “plan” tool producing step lists for transparency.
- `core/src/project_doc.rs`: Builds project documentation snapshot for context.
- `core/src/prompt_for_compact_command.md`: Short prompt template for compact commands.
- `core/src/protocol.rs`: Protocol data types used by CLI/TUI (e.g., `TokenUsage`, SSE forms, `FinalOutput`).
- `core/src/rollout.rs`: Controlled rollouts and gating of features.
- `core/src/safety.rs`: Platform safety layer; exposes `get_platform_sandbox`.
- `core/src/seatbelt_base_policy.sbpl`: Base macOS sandbox policy.
- `core/src/seatbelt.rs`: Seatbelt integration (macOS).
- `core/src/shell.rs`: Abstractions for shell execution and quoting.
- `core/src/spawn.rs`: Process spawning and I/O plumbing.
- `core/src/turn_diff_tracker.rs`: Tracks diffs across turns for apply action.
- `core/src/user_notification.rs`: Notifies on turn completion via configured hooks.
- `core/src/util.rs`: Misc utilities used broadly.
- Tests in `core/tests/`: Cover CLI streams, exec events, sandbox, live agent/CLI flows; includes fixtures (`*.json`, `*.sse`) and a small test support crate in `common/`.

### Crate: exec (headless/programmatic mode)
- `exec/Cargo.toml`: Manifest.
- `exec/src/main.rs`: Entrypoint for `codex exec` subcommand.
- `exec/src/lib.rs`: Library surface for programmatic mode.
- `exec/src/cli.rs`: Clap args for non-interactive execution.
- `exec/src/event_processor.rs`: Core event processing pipeline.
- `exec/src/event_processor_with_human_output.rs`: Formats human-friendly console output.
- `exec/src/event_processor_with_json_output.rs`: Emits structured JSON events.
- `exec/tests/apply_patch.rs`: Tests around applying patches in exec mode.

### Crate: execpolicy
- `execpolicy/Cargo.toml`, `build.rs`, `README.md`: Manifest/build/doc.
- `execpolicy/src/lib.rs`: Exposes exec policy checking API.
- `execpolicy/src/default.policy`: Policy file defining allowed commands/paths.
- `execpolicy/src/policy.rs`: Policy data structures.
- `execpolicy/src/policy_parser.rs`: Parser for the policy file format.
- `execpolicy/src/arg_type.rs`: Types of arguments (path, url, literal).
- `execpolicy/src/arg_matcher.rs`: Matching logic for arguments against policy.
- `execpolicy/src/arg_resolver.rs`: Resolves arg values to canonical forms.
- `execpolicy/src/exec_call.rs`: Represents a proposed exec call to validate.
- `execpolicy/src/execv_checker.rs`: Low-level execv validation.
- `execpolicy/src/program.rs`: Program rules and matching.
- `execpolicy/src/opt.rs`: Option parsing helpers within the policy language.
- `execpolicy/src/sed_command.rs`: Specific handling for `sed` safety.
- `execpolicy/src/valid_exec.rs`: End-to-end validation of a candidate exec call.
- Tests in `execpolicy/tests/*.rs`: Good/bad cases for many programs and sed parsing.

### Crate: file-search
- `file-search/Cargo.toml`, `README.md`: Manifest/doc.
- `file-search/src/lib.rs`: Fuzzy search core logic.
- `file-search/src/cli.rs`: CLI interface (used by TUI’s `@` search).
- `file-search/src/main.rs`: Entrypoint.

### Crate: linux-sandbox
- `linux-sandbox/Cargo.toml`, `README.md`: Manifest/doc.
- `linux-sandbox/src/main.rs`: Entrypoint to run commands under Linux sandbox.
- `linux-sandbox/src/lib.rs`: Shared sandbox utilities.
- `linux-sandbox/src/landlock.rs`: Landlock + seccomp setup and enforcement.
- `linux-sandbox/src/linux_run_main.rs`: Program bootstrap for sandboxed runs.
- `linux-sandbox/tests/landlock.rs`: Landlock integration tests.

### Crate: login
- `login/Cargo.toml`: Manifest.
- `login/src/lib.rs`: Auth helpers to read/store tokens for model providers.
- `login/src/login_with_chatgpt.py`: Browser-based ChatGPT login helper script.
- `login/src/token_data.rs`: Token data structures (expiry, refresh, scope).

### Crate: mcp-client
- `mcp-client/Cargo.toml`: Manifest.
- `mcp-client/src/main.rs`: Entrypoint to run as an MCP client.
- `mcp-client/src/lib.rs`: Library surface for embedding MCP client functionality.
- `mcp-client/src/mcp_client.rs`: Implements MCP handshake, message loop.

### Crate: mcp-server
- `mcp-server/Cargo.toml`: Manifest.
- `mcp-server/src/main.rs`: Entrypoint to run Codex as an MCP server.
- `mcp-server/src/lib.rs`: Exposes server bootstrap functions.
- `mcp-server/src/mcp_protocol.rs`: Protocol datatypes/messages for MCP.
- `mcp-server/src/codex_tool_config.rs`: Loads/validates tool config for MCP server.
- `mcp-server/src/codex_tool_runner.rs`: Executes configured tools; result marshaling.
- `mcp-server/src/conversation_loop.rs`: Server conversation loop with turn handling.
- `mcp-server/src/exec_approval.rs`: Approval flow integration for tool execs.
- `mcp-server/src/json_to_toml.rs`: Tool to convert JSON config to TOML.
- `mcp-server/src/message_processor.rs`: Processes inbound MCP messages.
- `mcp-server/src/outgoing_message.rs`: Constructs outbound messages.
- `mcp-server/src/patch_approval.rs`: Approval logic for applying patches.
- `mcp-server/src/tool_handlers/mod.rs`: Tool handler registry.
- `mcp-server/src/tool_handlers/create_conversation.rs`: Implements create-conversation tool.
- `mcp-server/src/tool_handlers/send_message.rs`: Implements send-message tool.
- Tests in `mcp-server/tests/*.rs`: Validate tool flows, conversation lifecycle, interrupts; includes a mock model server.

### Crate: mcp-types
- `mcp-types/Cargo.toml`: Manifest.
- `mcp-types/README.md`: Docs.
- `mcp-types/schema/2025-03-26/schema.json`: MCP schema version (older).
- `mcp-types/schema/2025-06-18/schema.json`: Latest MCP schema.
- `mcp-types/generate_mcp_types.py`: Generates Rust types from schema.
- `mcp-types/src/lib.rs`: Generated or hand-written types for MCP messages.
- Tests in `mcp-types/tests/*.rs`: Initialization and progress notifications.

### Crate: ollama (OSS provider support)
- `ollama/Cargo.toml`: Manifest.
- `ollama/src/lib.rs`: Public API to interact with OSS model provider.
- `ollama/src/client.rs`: HTTP client to local Ollama server (pull/run/status).
- `ollama/src/parser.rs`: Parses streaming responses and errors.
- `ollama/src/pull.rs`: Image/model pulling orchestration.
- `ollama/src/url.rs`: URL builders and endpoint helpers.

### Crate: tui (interactive terminal UI)
- `tui/Cargo.toml`: Manifest for `codex-tui` binary/lib.
- `tui/prompt_for_init_command.md`: Minimal prompt used at first-run.
- `tui/src/lib.rs`: Library entry to run the TUI (`run_main`), logging setup, config resolution, trust-screen decision, and terminal lifecycle.
- `tui/src/main.rs`: TUI binary entrypoint (sets up terminal and calls into `lib`).
- `tui/src/app.rs`: Core app state and event loop (`App::run`), token accounting.
- `tui/src/app_event.rs`: Event enum driving UI updates (key presses, logs, etc.).
- `tui/src/app_event_sender.rs`: Channel sender abstraction for app events.
- `tui/src/bottom_pane/mod.rs`: Bottom pane module root re-exporting subviews.
- `tui/src/bottom_pane/bottom_pane_view.rs`: Layout and rendering of the bottom input pane.
- `tui/src/bottom_pane/chat_composer.rs`: Input handling, multiline editor, paste handling, image attachment placeholders.
- `tui/src/bottom_pane/chat_composer_history.rs`: History of composer inputs.
- `tui/src/bottom_pane/approval_modal_view.rs`: Approvals popup UI.
- `tui/src/bottom_pane/command_popup.rs`: Slash command popup rendering/logic.
- `tui/src/bottom_pane/file_search_popup.rs`: `@` file search popup UI and selection.
- `tui/src/bottom_pane/live_ring_widget.rs`: Live ring (activity indicator).
- `tui/src/bottom_pane/past_inputs_popup.rs`: Recently sent inputs UI.
- `tui/src/bottom_pane/popup_consts.rs`: Popup layout constants.
- `tui/src/bottom_pane/prompts_popup.rs`: Prompt suggestions UI.
- `tui/src/bottom_pane/scroll_state.rs`: Scroll state handling.
- `tui/src/bottom_pane/selection_popup_common.rs`: Shared popup selection utilities.
- `tui/src/bottom_pane/resume_popup.rs`: Resume session popup.
- `tui/src/bottom_pane/snapshots/*.snap`: Snapshot tests of UI fragments.
- `tui/src/bottom_pane/textarea.rs`: Textarea widget with cursor/selection logic.
- `tui/src/chatwidget.rs`: Main chat transcript rendering and layout.
- `tui/src/citation_regex.rs`: Regex helpers to detect/render citations.
- `tui/src/cli.rs`: Clap args for TUI (model, sandbox flags, `--oss`, images).
- `tui/src/colors.rs`: Centralized color palette for UI components.
- `tui/src/custom_terminal.rs`: Lower-level terminal setup/teardown helpers.
- `tui/src/diff_render.rs`: Side-by-side diff rendering in the UI.
- `tui/src/exec_command.rs`: Glue to run commands from UI and stream output.
- `tui/src/file_search.rs`: Wiring to `file-search` crate for `@` feature.
- `tui/src/get_git_diff.rs`: Fetches current git diff for display.
- `tui/src/history_cell.rs`: History cell rendering in chat.
- `tui/src/insert_history.rs`: Logic for inserting historical messages.
- `tui/src/live_wrap.rs`: Adaptive line-wrapping for terminal width.
- `tui/src/log_layer.rs`: Tracing subscriber layer that streams logs to UI.
- `tui/src/markdown.rs`: Markdown rendering to terminal widgets.
- `tui/src/onboarding/mod.rs`: Onboarding flow module root.
- `tui/src/onboarding/auth.rs`: Login screen component.
- `tui/src/onboarding/continue_to_chat.rs`: Transition screen logic.
- `tui/src/onboarding/onboarding_screen.rs`: Main onboarding UI composition.
- `tui/src/onboarding/trust_directory.rs`: Trust project directory workflow.
- `tui/src/onboarding/welcome.rs`: Welcome screen.
- `tui/src/shimmer.rs`: Loading shimmer effects.
- `tui/src/slash_command.rs`: Slash command parsing and execution.
- `tui/src/status_indicator_widget.rs`: Status line widget.
- `tui/src/text_block.rs`: Rich text block layout/flow.
- `tui/src/text_formatting.rs`: Text styling and spans.
- `tui/src/tui.rs`: Terminal init/restore and ratatui backend glue.
- `tui/src/updates.rs`: Update notification logic (release checks).
- `tui/src/user_approval_widget.rs`: Approval banners and prompts.
- `tui/tests/*.rs`: TUI-specific tests (status indicator, VT100 history).

### How the big pieces fit
- CLI (`cli`) is the front door and dispatches to TUI (`tui`), headless exec (`exec`), or MCP server/client.
- `core` holds agent logic, config, model/provider wiring, exec/safety, and protocol types.
- TUI renders chat, approvals, diffs, and integrates with `core` and `file-search`.
- Sandbox layers (`execpolicy`, `linux-sandbox`, macOS Seatbelt in `core`) enforce safe execution.
- `chatgpt`, `login`, and `ollama` provide provider-specific clients and auth.
- MCP client/server expose Codex via Model Context Protocol.

- Added support: Image attachments in TUI composer, approvals toggles, OSS provider bootstrap.

- Tests cover E2E flows for apply, protocol streaming, sandboxing, and UI snapshots.

- Docs and configuration live in `config.md`, `docs/protocol_v1.md`, and TUI/CLI prompts.

- Build/infra: release script, Nix build, toolchain pin, and formatting rules.

- Utilities (`ansi-escape`, `common`, `file-search`) are shared across crates.

- Safety: `execpolicy` + platform sandboxes gate external command execution.

- Extensibility: MCP server exposes tool handlers to connect Codex to other clients.

- Model providers: OpenAI and OSS (`ollama`) are abstracted via `core` provider info.

- Patch application: `apply-patch` and `core/src/apply_patch.rs` implement safe diffs and the “Apply” command integrates through `chatgpt`.

- Event processing: `exec` provides both human-readable and JSON event streams for automation.

- Logging: `tui` sets up layered logging to file and to UI via a custom tracing layer.

- Trust flow: TUI determines repo trust and sets defaults for sandbox/approval if unset.

- File search/popups: bottom pane components implement quick file lookup and prompt tools.

- Protocol: `core/src/protocol.rs` defines message formats and usage accounting, printed by CLI in headless mode when appropriate.

- Seatbelt policy: `core/src/seatbelt_base_policy.sbpl` supplies macOS sandbox rules.

- Landlock: `linux-sandbox` configures Landlock/seccomp for Linux execution.

- MCP types are generated from JSON schema (`mcp-types`).

- `arg0` enables a single binary to behave differently based on its invoked name.

- `ansi-escape` supports accurate terminal rendering, colors, and cursor control.

- `execpolicy` enforces command-level safety via a policy language and parser.

- `common` centralizes CLI arg parsing for approval/sandbox and summaries.

- `ollama` supports running an OSS model locally and bootstrapping on demand.

- `chatgpt` includes token handling for ChatGPT web auth and task retrieval.

- `mcp-server` wires `core` agent operations to MCP tools and conversation loops.

- `mcp-client` implements the counterpart client for MCP testing and integrations.

- `file-search` powers the inline `@` search within TUI.

- `login` provides API key or ChatGPT-based auth flows stored in Codex home.

- `tui` brings it all together with ratatui for the interactive experience.

- `cli` exposes all functionality under `codex-custom`.

- `core` is the heart of Codex agent behavior and safety.

- `apply-patch` provides robust patch application across the workspace.

- `ansi-escape` + `log_layer` ensure clean, non-blocking UI logging.

- `docs/` and config files round out the developer and user experience.

- Release infra and toolchain ensure consistent builds across macOS/Linux.

- Snapshot tests ensure UI regressions are caught.

- E2E and integration tests validate critical flows (apply, protocol, sandbox).

- OSS model support is first-class via `--oss` TUI flag and bootstrap.

- Approval policies are toggled live from the TUI with Shift+Tab.

- Composer supports image attachments via paste or CLI `-i`.

- Headless mode prints `FinalOutput` with token usage when non-zero.

- Safety nets prevent accidental stdout/stderr from library code.

- Providers are abstracted to add/modify models cleanly.

- Turn diff tracking supports resume/apply workflows.

- Project docs and git diffs are surfaced in the UI.

- Trust decision can auto-enable workspace-write sandbox for trusted repos.

- Protocol streaming mode allows embedding Codex into other tools via stdin/stdout.

- Fuzzy matching and `@` search improve navigation and context curation.

- Landlock/Seatbelt enable safe experimentation with external commands.

- Policy language supports nuanced safety constraints.

- Upgrade checks nudge users to newer releases.

- The architecture separates business logic (`core`) from presentation (`tui`) and shell/exec (`exec`, sandboxes), improving maintainability.

- The workspace uses strict lints to avoid footguns (`clippy::print_*` denies).

- The TUI wiring uses a custom tracing layer to stream logs to the UI in near real-time.

- Many components are reusable crates to keep file sizes small and code modular.

- Test coverage spans unit, integration, and snapshot layers, following best practices.

- The CLI multitool presents a cohesive UX through subcommands and shared flags.

- Provider and model metadata in `core` centralize capabilities and defaults.

- Patch parser and seeker are optimized for reliability and speed.

- Generated MCP types ensure protocol compatibility with evolving schemas.

- The repository is organized for clarity and composability across features.

- Configuration supports profiles and overrides with clear precedence.

- The system is built for production-grade safety, extensibility, and UX.

- Logging is both file-based and in-UI, with non-blocking appenders.

- Error handling is surfaced to UI and exit codes consistently.

- Modify/extend components with minimal cross-crate coupling.

- Sandbox policies default to safe, with opt-in elevated modes.

- Terminal initialization/restore handles panics gracefully.

- Image flows map placeholders to `input_image` parts for OpenAI Responses API.

- Headless JSON event stream is suited for CI/CD usage.

- CLI completion generation supports multiple shells and binary names.

- Arg0 support eases installation and symlink-based invocation variants.

- Utility crates like `ansi-escape` and `file-search` keep TUI fast and snappy.

- The project is engineered to scale features while maintaining safety and UX.

- All critical paths have targeted tests and fixtures.

- Git integration is deep (diffs, branch, dirty state, apply diffs).

- OSS model support checks readiness and prompts install as needed.

- Final outputs are compact and machine-readable when appropriate.

- Overall: a clean separation of concerns enabling long-term maintainability.

- End-to-end from UX to safety and protocol surfaces is covered by dedicated crates.

- Tight but flexible config/overrides model supports many workflows.

- The codebase adheres to strict quality bars and modern Rust practices.

- Tooling (Nix, scripts, justfile) ease contributors’ onboarding.

- The MCP server exposes Codex to broader ecosystems through well-defined tools.

- Many internal helpers isolate complexity (parsers, matchers, shells).

- Developer ergonomics like live approvals toggling and `@` search significantly improve DX.

- The architecture supports adding new providers and tools with minimal friction.

- Patch/apply workflows integrate closely with the agent turn lifecycle.

- UI components are modular, snapshot-tested, and theme-consistent.

- Non-interactive mode enables automation and scripting use cases.

- The code avoids printing directly from libs, channeling output through UI/log layers.

- Safety constraints balance power and protections for local systems.

- The repository is designed to be extended and maintained professionally.

- This mapping should make it straightforward to find and modify any behavior across the stack.

- For deeper specifics, we can open any file you want to inspect next.
