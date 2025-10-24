## Overview
`codex-tui::history_cell` defines the transcript entries rendered in the chat history. Each `HistoryCell` captures a Codex event (user prompts, agent messages, command output, MCP interactions, approvals, etc.) and produces styled Ratatui lines with appropriate wrapping and metadata.

## Detailed Behavior
- `HistoryCell` trait:
  - `display_lines(width)` returns lines for the scrollable history view.
  - `desired_height`/`desired_transcript_height` compute layout heights via `Paragraph::line_count`.
  - `transcript_lines` provides lines for the transcript log (may differ from on-screen display).
  - `is_stream_continuation` marks cells that append to an existing stream (e.g., streaming Markdown).
- Implementations (extensive list):
  - `UserHistoryCell`: renders user prompts with dimmed prefix (`›`), applying wrapping via `word_wrap_lines`.
  - `ReasoningSummaryCell`: shows reasoning summaries (optional transcript-only), italic/dim styling with bullet prefix.
  - `AgentMessageCell`, `McpToolCallCell`, `CommandOutputCell` (not shown in excerpt but in file) handle agent responses, MCP invocations, command output (with spinner states, aggregated output, diff summaries).
  - `PlanHistoryCell`, `RateLimitCell`, `ApprovalRequestCell`, `TokenUsageCell`, etc., provide specialized renderings (plan steps, rate limit warnings, approvals, token summaries).
  - Cells leverage utilities:
    - Markdown rendering (`append_markdown`), diff display (`create_diff_summary`, `display_path_for`), wrapping (`RtOptions`, `word_wrap_lines`).
    - Execution helpers (`output_lines`, `format_and_truncate_tool_result`, `strip_bash_lc_and_escape`).
    - Git integration (`create_ghost_commit`, `restore_ghost_commit`) to preview edits safely.
  - Many cells highlight role-specific context (e.g., agent messages with styling, stream controllers for incremental output).
- Shared helpers:
  - `prefix_lines`, `push_owned_lines`, `line_to_static` manage line concatenation.
  - Rate limit thresholds warn users at 75/90/95% consumption.
  - `SessionConfiguredEvent` header cell displays session metadata (model, API key status).
- Structs maintain additional metadata for decisions (e.g., `ReasoningSummaryFormat`, `UpdateAction` for upgrade prompts, `McpAuthStatus`).

## Broader Context
- `ChatWidget` maintains a list of `HistoryCell`s to render the transcript. Cells are added in response to `AppEvent::InsertHistoryCell`.
- Transcript logging (`session_log`) uses `HistoryCell::transcript_lines` to record per-cell output.
- Cells feed into TUI layout helpers (`ColumnRenderable`, `RowRenderable`) and bottom pane status indicators.
- Context can't yet be determined for new event types; adding a variant requires implementing `HistoryCell` with appropriate styling/wrapping.

## Technical Debt
- `history_cell.rs` is large and handles many variants; splitting into submodules (per cell type) or introducing builder structs could reduce complexity.
- Some cells rely on direct width arithmetic; consolidating into reusable formatting helpers would avoid duplication.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Refactor `HistoryCell` implementations into smaller modules or structs to reduce file size and ease maintenance.
related_specs:
  - ./chatwidget.rs.spec.md
  - ./render/renderable.rs.spec.md
  - ./bottom_pane/mod.rs.spec.md
