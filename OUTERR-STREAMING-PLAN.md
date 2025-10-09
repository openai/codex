# Plan: Toggleable Live Stdout/Stderr Streaming in Codex CLI

## Current Behavior and Constraints

- `codex-rs/core/src/exec.rs` already emits `EventMsg::ExecCommandOutputDelta` via `read_capped`, streaming stdout/stderr chunks while a command runs. The deltas carry the `call_id`, stream discriminator, and raw bytes, so the backend is ready.
- The TUI ignores those deltas today (`tui/src/chatwidget.rs` drops them in `on_exec_command_output_delta`), and `ExecCell` only renders final `CommandOutput` once `ExecCommandEnd` arrives. While a command is running the UI shows only a spinner and the command header.
- The bottom status indicator (`tui/src/status_indicator_widget.rs`) has infrastructure for queued hints but no hook for live command output lines or toggle affordances.
- Output folding is handled inside `tui/src/exec_cell/render.rs` by `output_lines`, which trims to `TOOL_CALL_MAX_LINES`. We should reuse its conventions to keep styling consistent.

## Goal for the “simple” version
Expose a lightweight toggle that lets the user reveal or hide the most recent stdout/stderr lines for the currently running exec call directly inside the existing active command cell. When hidden, keep the status line showing the latest line so users still see progress. The implementation should cap memory, obey the Stylize helpers, and avoid protocol changes.

## Implementation Steps

1. **Model live output inside `ExecCell`**
   - Introduce a `LiveExecStream` helper in `tui/src/exec_cell/model.rs` that tracks buffered lines per stream (stdout/stderr), pending partial line segments, and a capped `VecDeque<String>` (e.g., keep the last 200 wrapped lines combined). Provide `push_chunk(stream, &[u8])` that decodes via `String::from_utf8_lossy`, splits on `\n`, updates the per-stream buffers, and returns the newest rendered line for status updates.
   - Extend `ExecCall` with `live: LiveExecStream` (initialized when the call is created) and a `show_live_output: bool` flag scoped to the cell. Add convenience methods (`append_live_chunk`, `latest_live_line`, `toggle_live_output`) that `ChatWidget` can call.
   - Ensure `ExecCell::complete_call` clears the live buffer and resets the toggle so completed history entries never keep the transient data.

2. **Handle streaming events in `ChatWidget`**
   - Update `ChatWidget::on_exec_command_output_delta` in `tui/src/chatwidget.rs` to locate the active `ExecCell`, forward the chunk to `ExecCell::append_live_chunk`, request a redraw, and store the last emitted line (if any) so we can surface it in the status indicator.
   - Because events may land while the exec cell is queued, guard against missing cells by falling back to the `running_commands` map and a short-lived cache keyed by `call_id`.
   - When we capture a new visible line, call a new `BottomPane::set_status_live_line` helper that updates the status indicator (see step 3).

3. **Render live output and status hints**
   - Extend `ExecCell::command_display_lines` in `tui/src/exec_cell/render.rs` to append the capped live output block whenever the call is active and `show_live_output` is true. Reuse `word_wrap_line`, `prefix_lines`, and `ansi_escape_line` so styling matches final output. Prefix stdout lines with a dim angle-pipe like the final output uses; color stderr lines red for quick scanning.
   - When the toggle is off but buffered output exists, render a single dim hint line (“live output hidden — press ctrl+r”) so users discover the shortcut without expanding accidentally.
   - In `StatusIndicatorWidget`, add optional storage for the latest streamed line and draw it on a second line under the header (trimmed to the pane width). Add a method such as `set_live_output_preview(String)` that the chat widget calls whenever a new line arrives. Clear it when the command completes.

4. **Wire up the toggle gesture**
   - Add a `KeyBinding` constant (e.g., `CTRL_R`) in `tui/src/status_indicator_widget.rs`/`tui/src/chatwidget.rs`. In `ChatWidget::handle_key_event`, detect `ctrl+r` while a command is running and flip the active exec cell’s `show_live_output` flag. Request a redraw and update the status indicator hint text to reflect the new state (“ctrl+r hide output” vs “ctrl+r show output”).
   - Surface the shortcut in the status indicator by appending the key hint line (using the existing `key_hint` helpers) so the UI communicates the toggle while a task is running.
   - Reset the toggle state, status preview line, and key hint when handling `ExecCommandEnd`.

5. **Testing & polish**
   - Add unit coverage for `LiveExecStream::push_chunk` to ensure partial lines and stderr/stdout separation work, and that line caps drop the oldest content.
   - Extend `tui/src/chatwidget/tests.rs` with a snapshot covering: (a) live output hidden (shows hint), and (b) live output visible (shows wrapped lines and hints). Update `cargo insta` snapshots if needed.
   - Manually confirm that streamed output appears for local exec commands and for MCP-driven commands (the protocol already forwards deltas, but sanity-check via Playwright or a fake tool).
   - Implementation runbook: after coding, run `just fmt`, `just fix -p codex-tui`, and `cargo test -p codex-tui` per repo conventions.

## Open Questions / Follow-ups
- Decide the exact cap (lines vs bytes). Lines are easier for the TUI; start with ~200 lines and revisit if memory spikes.
- Multi-command turns: today an exec cell can queue multiple shell calls (e.g., exploring lists). Confirm whether we should allow toggling per call or only the most recent active one. The simple path is to scope the toggle to the active call only.
- For non-UTF8 output we currently lossily decode; if that becomes an issue we may need to surface a “[binary chunk]” placeholder later.
- Future enhancements could reuse the same buffer to back a scrollable pager overlay, or persist the streamed lines into history when the command ends.
