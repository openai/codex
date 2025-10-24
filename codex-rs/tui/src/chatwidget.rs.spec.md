## Overview
`codex-tui::chatwidget` renders the main chat transcript, handles agent/user events, manages approval flows, and coordinates with the `BottomPane` composer. It converts Codex protocol events into history cells, maintains command execution state, and drives auxiliary popups (model selection, approvals, diff views).

## Detailed Behavior
- Key components:
  - `ChatWidget` stores references to the conversation manager, app event sender, bottom pane, file search manager, and session metadata (e.g., rate limit warnings, ghost commits).
  - Submodules (`agent`, `interrupts`, `session_header`) encapsulate agent spawning/resume logic, Ctrl+C handling, and header rendering.
- Initialization:
  - `ChatWidget::new` (or `new_from_existing`) constructs the widget with `ChatWidgetInit` (config, frame requester, event sender, initial prompt/images, auth manager, feedback).
  - When resuming, `spawn_agent_from_existing` restores conversation state before rendering.
- Event handling:
  - Methods respond to `EventMsg` variants: agent messages/reasoning, command begin/end, approvals, diff updates, background/status events, rate limit snapshots, turn completion, errors, etc.
  - Maintains a `VecDeque` of history cells (`HistoryCell` trait) representing transcript entries (user prompts, agent messages, command output, MCP calls).
  - Tracks running commands (`RunningCommand`) to aggregate output, parse commands, and flush results into history cells once complete.
  - Triggers rate-limit warnings when thresholds (75/90/95%) are crossed, queuing messages via `RateLimitWarningState`.
- Bottom pane coordination:
  - Interfaces with `BottomPane` to display composer, selection popups, and approval prompts (`ApprovalRequest`).
  - Handles slash commands (`SlashCommand`), diff requests, custom prompt view, selection actions, and queued user messages.
- File operations / git integration:
  - Manages ghost commits (`create_ghost_commit`, `restore_ghost_commit`) to safely preview agent edits.
  - Launches `FileSearchManager` tasks for `@search` commands and displays results.
  - Fetches git diff (`get_git_diff`) and branch/commit lists for review workflows.
- Rendering:
  - Implements `Renderable`/`ColumnRenderable` to draw transcript columns with Ratatui.
  - Uses `SessionHeader` to show workspace metadata (model, cwd, git branch).
  - Integrates approval overlays via `ApprovalOverlay`.

## Broader Context
- `App::handle_event` forwards Codex events to `ChatWidget` methods, while the bottom pane returns input events through `AppEvent`.
- Rendering relies on history cells (`history_cell` module) and diff rendering utilities. Other widgets (status indicator, bottom pane) consume the same app event channel.
- Context can't yet be determined for multi-threaded rendering; current design assumes single-threaded UI updates with background tasks using `AppEventSender`.

## Technical Debt
- `chatwidget.rs` is large (handles command parsing, approvals, git integration); extracting features into dedicated structs (e.g., rate limits, file operations) would reduce complexity.
- Running command tracking is ad-hoc; consider generalizing into an execution manager shared with other UI surfaces.

---
tech_debt:
  severity: high
  highest_priority_items:
    - Refactor command/execution handling into a dedicated component to simplify event methods.
    - Separate git/ghost commit logic from the chat widget core to limit responsibilities.
related_specs:
  - ./bottom_pane/mod.rs.spec.md
  - ./history_cell.rs.spec.md
  - ./render/mod.rs.spec.md
