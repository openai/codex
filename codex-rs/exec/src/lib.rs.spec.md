## Overview
`exec::lib` drives the `codex-exec` binary. `run_main` parses CLI options, loads configuration (including OSS bootstrap), streams prompt input, and mediates the async event loop that proxies Codex core events into either human-oriented stderr output or structured JSONL. Helper utilities in this module load JSON schemas for structured output and resolve resume rollouts.

## Detailed Behavior
- CLI unpacking:
  - `Cli` (from `cli.rs`) supplies prompt, model, OSS flag, sandbox overrides, resume parameters, color settings, and config overrides. `run_main` unwraps these fields, handling prompt resolution (argument vs stdin) and optional output schema loading via `load_output_schema`.
  - Color settings determine ANSI enablement for stdout/stderr by probing terminal support (`supports_color`).
- Telemetry setup:
  - Calls `codex_core::otel_init::build_provider` and installs tracing layers (`fmt_layer` and OTEL bridge when enabled), falling back to stderr-only logging on failure before exiting.
- Event processing mode:
  - Constructs either `EventProcessorWithJsonOutput` (JSONL mode) or `EventProcessorWithHumanOutput` (default) using the CLI’s `--json` flag and last-message output path.
- Configuration and auth:
  - Builds `ConfigOverrides` from CLI flags, forcing approval policy to `Never` in headless mode and wiring OSS defaults (`DEFAULT_OSS_MODEL`, built-in provider). Parses `-c` overrides via `CliConfigOverrides`.
  - Loads `Config::load_with_cli_overrides`, enforces login restrictions, ensures OSS models are present (`codex_ollama::ensure_oss_ready`), and validates Git repo presence unless `--skip-git-repo-check` is set.
  - Installs tracing originator override (`set_default_originator("codex_exec")`).
- Conversation lifecycle:
  - Creates shared `AuthManager` and `ConversationManager` (SessionSource::Exec).
  - Resume handling (`ExecCommand::Resume`) resolves rollout paths with `resolve_resume_path`, falling back to `new_conversation` when none provided.
  - Prints configuration summary via `EventProcessor::print_config_summary`.
  - Spawns a task that pulls events from `Conversation::next_event`, relaying them into an unbounded channel and shutting down on `ShutdownComplete` or error. CTRL+C triggers `Op::Interrupt`, breaking out of the receive loop.
- Prompt dispatch:
  - Sends initial images as `Op::UserInput`, waiting for the task completion event before proceeding.
  - Submits the main prompt as `Op::UserTurn`, carrying sandbox/model overrides and optional JSON schema.
- Event loop:
  - Drains events from the channel, flagging `EventMsg::Error` to determine exit status.
  - Delegates each event to the chosen `EventProcessor`; upon `CodexStatus::InitiateShutdown` issues `Op::Shutdown`, and breaks on `CodexStatus::Shutdown`.
  - After loop completion, calls `print_final_output` (which may write the final agent message to stdout). Exits with non-zero status if any error events were seen.
- `resolve_resume_path`:
  - Supports `--last` by listing rollouts via `RolloutRecorder::list_conversations`.
  - Resolves explicit session IDs through `find_conversation_path_by_id_str`.
- `load_output_schema` reads a JSON Schema file and exits with error messaging if the file is unreadable or invalid JSON.

## Broader Context
- `main.rs` wraps `run_main`, merging top-level config overrides and dispatching `codex-linux-sandbox` via `codex_arg0`. Specifications for sandbox execution (`linux-sandbox`) and process hardening describe the other entrypoint.
- Event processors (`event_processor_with_human_output.rs`, `event_processor_with_jsonl_output.rs`) define the streaming behavior summarized here; updates must stay in sync with expectations in this loop.
- CLI and config overrides mirror shared primitives in `codex_common`/`codex_core`; notable cross-links include `config.rs.spec.md` and `ConversationManager` coverage.
- Context can't yet be determined for multi-turn interactions in exec; future features (e.g., interactive approvals) would require revisiting shutdown semantics and event handling.

## Technical Debt
- `run_main` is monolithic and blends argument parsing, configuration loading, telemetry setup, and conversation orchestration; splitting into smaller helpers or a struct state machine would improve testability.
- Error handling for event loop shutdown is best-effort (logs + break); propagating errors back to the caller would enable richer exit statuses and simplify future automation.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Refactor `run_main` into smaller helpers (CLI→config, telemetry setup, conversation lifecycle) to ease maintenance and unit testing.
    - Tighten event-loop error handling so transport failures or shutdown races surface deterministic exit codes instead of logging and continuing.
related_specs:
  - ./main.rs.spec.md
  - ./event_processor_with_human_output.rs.spec.md
  - ./event_processor_with_jsonl_output.rs.spec.md
  - ../core/src/config.rs.spec.md
