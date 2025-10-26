## Overview
Extensive regression suite for the ChatWidget. It exercises conversation replay, review flow, approvals, queue management, slash commands, status indicators, history rendering, and vt100 snapshots. The tests combine unit-style assertions with snapshot-based verification of Ratatui output to guard against UI regressions.

## Detailed Behavior
- **Construction helpers**: `make_chatwidget_manual` (and variants) build a widget with in-memory channels for direct event injection. Utility functions drain `AppEvent::InsertHistoryCell`, convert lines to strings, and open fixtures captured from prior sessions.
- **Conversation replay & review**:
  - Ensures resumed sessions render initial history, review mode banners honor hints, and exiting review with findings shows popups and banners.
  - Validates review popup navigation, custom prompt submission, commit picker output, branch picker behavior, and ESC navigation between nested popups.
- **Approval & patch workflows**:
  - Exec approval tests cover modal rendering, keyboard decisions, snippet truncation, and resulting history entries. Snapshot tests capture exec and patch approval modals (with/without reasons).
  - Patch approval tests verify diff summaries, manual flows, modal visibility, decision events, and integration-like flows that forward approval ops back to Codex.
- **Command execution & status**:
  - `begin_exec`/`end_exec` helpers simulate command lifecycle to assert that history cells flush correctly on success/failure, interruptions, and chained commands.
  - Status indicator tests check pause/resume timing, interrupt behavior, queue restoration, and vt100 snapshots of combined status + exec layout.
- **Queueing & composer behavior**:
  - Tests confirm queued messages edit via Alt+Up, history recall stability, CTRL+C shutdown/interrupt semantics, queued submissions during streaming final answers, and restart behavior.
- **Slash commands & popups**:
  - Validate disabled slash commands during active tasks, model/approval presets popups (with snapshots), full access confirmation, and reasoning-level popup navigation.
- **History rendering**:
  - Snapshot tests capture markdown rendering, plan updates, rate-limit warnings, multiple agent messages, reasoning deltas, and final message handling.
  - vt100-based transcripts replay recorded logs (e.g., binary size session, complex code blocks) to compare against golden fixtures.
- **Plan tooling & stream errors**:
  - Confirms plan updates render structured history cells, stream error events update the status indicator, and rate-limit warnings trigger appropriate user-facing hints.

## Broader Context
- Provides broad coverage of the TUI’s most critical behaviors, combining functional assertions with visual regression tests. The suite is used when refactoring ChatWidget internals, ensuring UI output and event semantics stay stable.
- Relies on shared helpers (`insert_history`, `selection_list`, `status_indicator_widget`, etc.) and touches numerous protocol event types from `codex_core`.

## Technical Debt
- Large reliance on snapshot fixtures increases maintenance when intentional UI tweaks occur; grouping snapshots or adding targeted assertions could reduce churn.
- Fixtures expect specific models (`gpt-5-codex`) and environment behavior; running on alternative configurations skips some tests or may require additional baselines.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Consider modularizing snapshot fixtures or adding helper assertions to isolate intent and reduce flakiness when UI styling evolves.
related_specs:
  - chatwidget.rs.spec.md
  - bottom_pane/mod.rs.spec.md
  - status_indicator_widget.rs.spec.md
