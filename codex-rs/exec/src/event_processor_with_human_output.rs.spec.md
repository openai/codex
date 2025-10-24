## Overview
`event_processor_with_human_output` renders Codex events as a human-readable transcript. It mirrors the TUI’s summary style: colored sections for agent responses, tool calls, patches, and token usage, while respecting CLI color preferences and optional reasoning visibility.

## Detailed Behavior
- Construction:
  - `EventProcessorWithHumanOutput::create_with_ansi` accepts an ANSI flag, the active `Config`, and an optional last-message path. It precomputes `owo_colors::Style` variants (bold, italic, dimmed, magenta/red/green/cyan) based on ANSI availability, and records whether to show agent reasoning or raw reasoning content.
- Event handling (`EventProcessor` implementation):
  - `print_config_summary` prints version info, `create_config_summary_entries`, session ID, and echoes the user prompt with styling (`user` in cyan).
  - `process_event` pattern-matches `EventMsg` variants:
    - Errors/warnings (`Error`, `StreamError`, `BackgroundEvent`) print dimmed/styled messages.
    - `TaskComplete` records the final message, writes last-message file via `handle_last_message`, and returns `InitiateShutdown`.
    - Reasoning-related events (`AgentReasoning`, raw sections, deltas) obey `show_agent_reasoning` and `show_raw_agent_reasoning` flags.
    - Agent messages print under the “codex” label; command execution events (`ExecCommandBegin/End`, output deltas) show command, cwd, duration, exit code, and truncated output with diff-like colorization.
    - MCP tool calls, web searches, patch applies, plan updates, and view-image events each receive dedicated formatted blocks with icons/status markers. Patch apply begin/end uses stored metadata (start time, auto-approved flag) to compute durations and color diffs (`A/D/M/R` markers).
    - Turn aborts emit textual summaries; `ShutdownComplete` returns `CodexStatus::Shutdown`.
    - Token counts cache latest totals for final reporting.
  - Any unhandled events (approval requests, list responses) are ignored to avoid clutter.
  - `print_final_output` prints token usage (if recorded) and writes the final agent message to stdout (preserving trailing newline semantics).
- Helpers:
  - `escape_command` uses `shlex::try_join` to restore shell commands for display.
  - `format_file_change` and `format_mcp_invocation` derive succinct labels for file diffs and MCP invocations.

## Broader Context
- The CLI event loop relies on this processor for default output in non-JSON mode. Style choices align with logs users expect from the TUI, ensuring parity between interactive and headless workflows.
- Last-message handling integrates with `run_main`’s `--output-last-message` flag, while token usage ties back to `TokenUsageInfo` for transparency about costs.
- Context can't yet be determined for multi-turn transcripts; current implementation assumes a single-turn session per invocation.

## Technical Debt
- `process_event` is lengthy and manually maps each event type; extracting per-event helpers or using a formatter struct would ease maintenance when new protocol events are added.
- Output truncation uses hard-coded limits (`MAX_OUTPUT_LINES_FOR_EXEC_TOOL_CALL`); allowing configuration would improve UX for CI or debugging contexts.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Break out event-specific handlers to reduce the size of `process_event` and make it easier to update when protocol events change.
    - Make command/patch output limits configurable (or expose an environment variable) so CI logs can opt into full output.
related_specs:
  - ./event_processor.rs.spec.md
  - ./event_processor_with_jsonl_output.rs.spec.md
  - ../core/src/protocol.rs.spec.md
