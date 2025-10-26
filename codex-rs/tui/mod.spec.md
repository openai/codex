## Overview
`codex-tui` implements Codex’s terminal user interface, wiring the CLI surface, app state machine, streaming controller, and Ratatui-based widgets into a cohesive experience. The crate exposes both the command-line entrypoints (`tui/src/main.rs`, `tui/src/cli.rs`) and the reusable application core consumed by integration tests and other binaries.

## Detailed Behavior
- Top-level binaries (`src/main.rs`, `bin/md-events.rs`) parse CLI arguments and dispatch into `codex_tui::run_main`.
- The library surface (`src/lib.rs`) re-exports the CLI, application state (`tui.rs`, `app.rs`), streaming machinery, and widget modules so downstream consumers (e.g., integration harnesses) can assemble or test the UI without invoking the binary.
- UI composition lives in subdirectories:
  - `bottom_pane`, `chatwidget`, `public_widgets`, `render`, `status`, `streaming`—each encapsulating focused Ratatui components.
  - Supporting modules handle markdown rendering, session logging, diff views, and wrap logic for codex-core interactions.

## Broader Context
- The TUI sits atop `codex-core`, orchestrating prompts, tool approvals, and streaming responses while providing a responsive terminal UX that mirrors the desktop app.

## Technical Debt
- The crate has grown organically; as more modules are documented, consider surfacing an architectural guide (state machine, rendering layers, async controller) to orient contributors quickly.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Produce an architectural overview (diagram or README) explaining state flow between `streaming::controller`, `app::App`, and widget layers to reduce onboarding time.
related_specs:
  - ./src/lib.rs.spec.md
