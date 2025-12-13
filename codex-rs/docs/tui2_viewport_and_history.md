# TUI2 Viewport, Transcript, and History – Design Notes

This document describes the current design of the Codex **TUI2** viewport and history model, and
explains how it maps the original TUI viewport work into the new crate. It is intentionally
evolutionary: as long as the legacy TUI remains the default, we want TUI2 to be able to innovate on
viewport behavior without destabilizing existing users, while still sharing the same core mental
model.

The target audience is Codex developers and curious contributors who want to understand or critique
how TUI2 owns its viewport, scrollback integration, and suspend/exit behavior.

---

## 1. Background and Goals

The original `docs/tui_viewport_and_history.md` describes why the legacy TUI moved away from trying
to “cooperate” with the terminal’s scrollback and instead treats the TUI viewport as an owned
surface. TUI2 inherits that philosophy and applies it to the new crate:

- The **in‑memory transcript** is the single source of truth for what appears on screen.
- The **viewport** is a rectangular region above the composer that TUI2 completely controls.
- Scrollback is treated as an **append‑only log** that we write to explicitly on suspend/exit,
  rather than something we try to maintain incrementally on every frame.

Concretely, TUI2 aims to:

1. Keep history correct, ordered, and never silently dropped.
2. Make scrolling, selection, and copy work in terms of logical transcript content, not raw terminal
   coordinates.
3. Keep the mental model for suspend and exit aligned with the legacy TUI so users do not need to
   relearn basic flows when opting into TUI2 via the `tui2` feature flag.

Where the original document speaks generically about “the TUI”, this document calls out the TUI2
paths and files explicitly so you can map the design back to code in `codex-rs/tui2`.

---

## 2. Transcript and Viewport Model in TUI2

### 2.1 Transcript as a logical sequence of cells

As in the original TUI, TUI2’s transcript is a list of **history cells**, each representing one
logical thing in the conversation:

- A user prompt (with padding and a distinct background).
- An agent response (which may arrive in multiple streaming chunks).
- System or info rows (session headers, migration banners, reasoning summaries, etc.).

Each cell knows how to draw itself for a given width. The transcript itself is **purely logical**:

- It has no baked‑in scrollback coordinates or terminal state.
- It can be re‑rendered for any viewport width.

The main app struct in `codex-rs/tui2/src/app.rs` holds the transcript as:

- `transcript_cells: Vec<Arc<dyn HistoryCell>>` – the logical history.
- `transcript_scroll: TranscriptScroll` – whether the viewport is pinned to the bottom or anchored
  at a specific cell/line pair.
- `transcript_selection: TranscriptSelection` – a selection expressed in screen coordinates over the
  flattened transcript region.
- `transcript_view_top` / `transcript_total_lines` – the current viewport’s top line index and total
  number of wrapped lines for the inline transcript area.

Together, these fields give TUI2 enough state to render the transcript, track scroll position, and
support selection and copy without relying on the terminal’s own scrollback.

### 2.2 Building viewport lines from the transcript

The main inline viewport is rendered by `App::render_transcript_cells` in `codex-rs/tui2/src/app.rs`.
At a high level it:

1. Defines a **transcript region** as “the full frame minus the height of the bottom input area”.
2. Flattens all cells into a list of visual lines with `App::build_transcript_lines`, remembering
   for each visual line which `(cell_index, line_in_cell)` it came from.
3. Passes those lines through `word_wrap_lines_borrowed` in `codex-rs/tui2/src/wrapping.rs` so
   wrapping happens consistently with the rest of the UI.
4. Uses `transcript_scroll` plus the flattened metadata to compute a `top_offset` into the wrapped
   lines.
5. Clears the transcript region and draws the visible slice into it, updating
   `transcript_view_top` and `transcript_total_lines`.
6. Applies selection styling on top of the rendered lines via `apply_transcript_selection`.

Scrolling (mouse wheel, PageUp/PageDown, Home/End) operates entirely in terms of these flattened
lines and the current scroll anchor. The terminal’s own scrollback is *not* part of this
calculation; it only ever sees fully rendered frames.

### 2.3 Bottom pane and footer awareness

The bottom pane and footer need to reflect transcript state without having to know about the full
history structure. TUI2 threads that information through:

- `ChatWidget::set_transcript_ui_state` in `codex-rs/tui2/src/chatwidget.rs` forwards:
  - `transcript_scrolled`
  - `transcript_selection_active`
  - `transcript_scroll_position` (current/total)  
  into the bottom pane.
- `BottomPane::set_transcript_ui_state` passes those flags into the `ChatComposer`.
- `ChatComposer::footer_props` includes them in `FooterProps`.
- `footer_lines` in `codex-rs/tui2/src/bottom_pane/footer.rs` uses those props to:
  - Show “PgUp/PgDn scroll · Home/End jump · current/total” when scrolled away from the bottom.
  - Show “Ctrl‑Y copy selection” when a transcript selection is active.

This mirrors the legacy TUI behavior while keeping the wiring localized to the bottom pane.

---

## 3. Input, Selection, and Copy

### 3.1 Mouse and keyboard scrolling

TUI2 treats scrolling as a first‑class concern:

- **Mouse wheel**  
  `App::handle_mouse_event` interprets wheel events over the transcript area as fixed line deltas and
  calls `scroll_transcript`. It also:
  - Clears any existing selection on scroll.
  - Anchors the scroll state away from the bottom when selection begins while a response is
    streaming, so the viewport stops auto‑following new output under the selection.

- **Keyboard shortcuts**  
  `App::handle_key_event` wires:
  - `PageUp` / `PageDown` to scroll by a full transcript viewport at the current size.
  - `Home` to jump to the top of the transcript.
  - `End` to return to `TranscriptScroll::ToBottom`.

Both mouse and keyboard scrolling share the same `TranscriptScroll` state and flattened transcript
representation, so the footer can show consistent hints regardless of input device.

### 3.2 Selection

Mouse‑driven selection is handled by:

- `TranscriptSelection` on `App`, which tracks `anchor` and `head` in screen coordinates.
- `App::handle_mouse_event`, which:
  - Clamps mouse coordinates into the transcript region above the composer.
  - Starts or updates the selection on left‑button down/drag.
  - Clears a zero‑length selection on mouse up.
- `App::apply_transcript_selection`, which:
  - Scans each visible row for non‑space glyphs in the transcript area.
  - Applies reversed styling to the intersection of the visible text and the selection region,
    intentionally skipping the left gutter used for prefixes.

Because the selection is defined in terms of the transcript viewport, it remains meaningful even as
the composer grows, shrinks, or the window is resized.

### 3.3 Copying the selected transcript

Copy behavior is implemented in `App::copy_transcript_selection` in `codex-rs/tui2/src/app.rs` and
`codex-rs/tui2/src/clipboard_copy.rs`:

1. The handler checks for a non‑empty `TranscriptSelection`.
2. It reconstructs just the transcript region off‑screen using the current width and transcript
   scroll state.
3. It walks the selected rows and columns to build a `Vec<String>` containing the selected text
   lines, preserving internal spaces and intentionally retaining empty lines inside the selection.
4. It joins those lines with `\n` and calls `clipboard_copy::copy_text`.
5. Any errors are logged; the footer exposes the `Ctrl‑Y copy selection` hint whenever a selection is
   active so users can discover both the shortcut and the fact that copy operates on transcript
   content rather than raw buffer memory.

This mirrors the legacy TUI design while being implemented entirely within the TUI2 crate.

---

## 4. Printing History to Scrollback (Exit Transcript)

TUI2 reuses the original “append‑only history” approach for scrollback, but does so through the new
crate and CLI integration:

- At the end of `App::run` in `codex-rs/tui2/src/app.rs`, TUI2:
  - Uses `App::build_transcript_lines` to flatten the transcript at the final width.
  - Computes a bitmap of user cells to apply user message styling.
  - Calls `App::render_lines_to_ansi` to turn those lines into ANSI strings.
  - Returns them as `session_lines` on `AppExitInfo`.
- The CLI (`codex-rs/cli/src/main.rs`) prints `session_lines` before the usual token‑usage and
  resume hints, so the exit transcript appears as a contiguous block in scrollback.

This is intentionally symmetric with the legacy TUI exit transcript, and the design document for the
original TUI still applies conceptually. TUI2 simply routes the behavior through `codex-tui2` and
the new feature flag.

Suspend‑time printing for TUI2 currently follows the same high‑level strategy as the legacy TUI
(`session_lines` on exit, optional printing on suspend), and continues to treat scrollback as an
append‑only log of logical transcript cells rather than an extra viewport to maintain.

---

## 5. Relationship to the Original Viewport Docs

The original TUI viewport and history notes live alongside this document under `docs/` and describe
the design in terms of the legacy crate. When reading this document, keep in mind:

- The **high‑level goals and tradeoffs** are shared between TUI and TUI2.
- The **implementation details** (module paths and type names) differ:
  - Where the original doc references `codex-rs/tui/src/app.rs`, TUI2 uses
    `codex-rs/tui2/src/app.rs`.
  - Where it references `tui/src/tui.rs`, TUI2 uses `tui2/src/tui.rs` and the same
    alt‑screen/suspend contracts.
  - The streaming markdown design notes are mirrored in
    `codex-rs/tui2/src/streaming_wrapping_design.md`.

If you are debugging viewport behavior, it is usually best to:

1. Skim the original `docs/tui_viewport_and_history.md` for motivation and historical context.
2. Use this document to map that context onto the TUI2 crate.
3. Jump into the code referenced here when you need to trace a specific behavior (e.g., scroll
   anchoring, selection, or exit transcript rendering).

---

## 6. Future Work and Open Questions

Most of the open questions from the original document still apply to TUI2. In particular:

- Whether we should make the “scroll vs live follow” state more explicit in the UI.
- Whether a lightweight scroll indicator (or mini‑map) would help long sessions without cluttering
  the limited vertical space.
- How much additional affordance we want for selection (beyond reversed text and footer hints).
- How configurable suspend‑time printing should be in TUI2, and whether we want different defaults
  when TUI2 becomes the primary frontend.

As we continue to evolve the TUI2 viewport, this document should be kept up to date alongside the
execplan in `docs/tui2_viewport_execplan.md` so that future contributors can trace both the “why”
and the “how” of viewport changes without having to rediscover the background.

