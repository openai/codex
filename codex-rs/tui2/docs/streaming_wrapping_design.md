# Streaming Wrapping Reflow (tui2)

This document describes a correctness bug in `codex-rs/tui2` and the chosen fix:
while streaming assistant markdown, soft-wrap decisions were effectively persisted as hard line
breaks, so resizing the viewport could not reflow prose.

## Goal

- Resizing the viewport reflows transcript prose (including streaming assistant output).
- Width-derived breaks are always treated as *soft wraps* (not logical newlines).
- Copy/paste continues to treat soft wraps as joinable (via joiners), and hard breaks as newlines.

Non-goals:

- Reflowing terminal scrollback that has already been printed.
- Reflowing content that is intentionally treated as preformatted (e.g., code blocks, raw stdout).

## Background: where reflow happens in tui2

TUI2 renders the transcript as a list of `HistoryCell`s:

1. A cell stores width-agnostic content (string, diff, logical lines, etc.).
2. At draw time (and on resize), `transcript_render` asks each cell for lines at the *current*
   width (ideally via `HistoryCell::transcript_lines_with_joiners(width)`).
3. `TranscriptViewCache` caches the wrapped visual lines keyed by width; a width change triggers a
   rebuild.

This only works if cells do *not* persist width-derived wrapping inside their stored state.

## The bug: soft wraps became hard breaks during streaming

Ratatui represents multi-line content as `Vec<Line>`. If we split a paragraph into multiple `Line`s
because the viewport is narrow, that split is indistinguishable from an explicit newline unless we
also carry metadata describing which breaks were “soft”.

Streaming assistant output used to generate already-wrapped `Line`s and store them inside the
history cell. Later, when the viewport became wider, the transcript renderer could not “un-split”
those baked lines — they looked like hard breaks.

## Chosen solution (A, F1): stream logical markdown lines; wrap in the cell at render-time

User choice recap:

- **A**: Keep append-only streaming (new history cell per commit tick), but make the streamed data
  width-agnostic.
- **F1**: Make the agent message cell responsible for wrapping-to-width so transcript-level wrapping
  can be a no-op for it.

### Key idea: separate markdown parsing from wrapping

We introduce a width-agnostic “logical markdown line” representation that preserves the metadata
needed to wrap correctly later:

- `codex-rs/tui2/src/markdown_render.rs`
  - `MarkdownLogicalLine { content, initial_indent, subsequent_indent, line_style, is_preformatted }`
  - `render_markdown_logical_lines(input: &str) -> Vec<MarkdownLogicalLine>`

This keeps:

- hard breaks (paragraph/list boundaries, explicit newlines),
- markdown indentation rules for wraps (list markers, nested lists, blockquotes),
- preformatted runs (code blocks) stable.

### Updated streaming pipeline

- `codex-rs/tui2/src/markdown_stream.rs`
  - `MarkdownStreamCollector` is newline-gated (no change), but now commits
    `Vec<MarkdownLogicalLine>` instead of already-wrapped `Vec<Line>`.
  - Width is removed from the collector; wrapping is not performed during streaming.

- `codex-rs/tui2/src/streaming/controller.rs`
  - Emits `AgentMessageCell::new_logical(...)` containing logical lines.

- `codex-rs/tui2/src/history_cell.rs`
  - `AgentMessageCell` stores `Vec<MarkdownLogicalLine>`.
  - `HistoryCell::transcript_lines_with_joiners(width)` wraps each logical line at the current
    width using `word_wrap_line_with_joiners` and composes indents as:
    - transcript gutter prefix (`• ` / `  `), plus
    - markdown-provided initial/subsequent indents.
  - Preformatted logical lines are rendered without wrapping.

Result: on resize, the transcript cache rebuilds against the new width and the agent output reflows
correctly because the stored content contains no baked soft wraps.

## Overlay deferral fix (D): defer cells, not rendered lines

When an overlay (transcript/static) is active, TUI2 is in alt screen and the normal terminal buffer
is not visible. Historically, `tui2` attempted to queue “history to print” for the normal buffer by
deferring *rendered lines*, which baked the then-current width.

User choice recap:

- **D**: Store deferred *cells* and render them at overlay close time.

Implementation:

- `codex-rs/tui2/src/app.rs`
  - `deferred_history_cells: Vec<Arc<dyn HistoryCell>>` (replaces `deferred_history_lines`).
  - `AppEvent::InsertHistoryCell` pushes cells into the deferral list when `overlay.is_some()`.

- `codex-rs/tui2/src/app_backtrack.rs`
  - `close_transcript_overlay` renders deferred cells at the *current* width when closing the
    overlay, then queues the resulting lines for the normal terminal buffer.

Note: as of today, `Tui::insert_history_lines` queues lines but `Tui::draw` does not flush them into
the terminal (see `codex-rs/tui2/src/tui.rs`). This section is therefore best read as “behavior we
want when/if scrollback printing is re-enabled”, not a guarantee that content is printed during the
main TUI loop. For the current intended behavior around printing, see
`codex-rs/tui2/docs/tui_viewport_and_history.md`.

## Tests (G2)

User choice recap:

- **G2**: Add resize reflow tests + snapshot coverage.

Added coverage:

- `codex-rs/tui2/src/history_cell.rs`
  - `agent_message_cell_reflows_streamed_prose_on_resize`
  - `agent_message_cell_reflows_streamed_prose_vt100_snapshot`

These assert that a streamed agent cell produces fewer visual lines at wider widths and provide
snapshots showing reflow for list items and blockquotes.

## Audit: other `HistoryCell`s and width-baked paths

This section answers “what else might behave like this?” up front.

### History cells

- `AgentMessageCell` (`codex-rs/tui2/src/history_cell.rs`): **was affected**; now stores logical
  markdown lines and wraps at render time.
- `UserHistoryCell` (`codex-rs/tui2/src/history_cell.rs`): wraps at render time from stored `String`
  using `word_wrap_lines_with_joiners` (reflowable).
- `ReasoningSummaryCell` (`codex-rs/tui2/src/history_cell.rs`): renders from stored `String` on each
  call; it does call `append_markdown(..., Some(width))`, but that wrapping is recomputed per width
  (reflowable).
- `PrefixedWrappedHistoryCell` (`codex-rs/tui2/src/history_cell.rs`): wraps at render time and
  returns joiners (reflowable).
- `PlainHistoryCell` (`codex-rs/tui2/src/history_cell.rs`): stores `Vec<Line>` and returns it
  unchanged (not reflowable by design; used for already-structured/preformatted output).

Rule of thumb: any cell that stores already-wrapped `Vec<Line>` for prose is a candidate for the
same bug; cells that store source text or logical lines and compute wrapping inside
`display_lines(width)` are safe.

### Width-baked output outside the transcript model

Even with the streaming fix, some paths are inherently width-baked:

- Printed transcript after exit (`codex-rs/tui2/src/app.rs`): `AppExitInfo.session_lines` is rendered
  once using the final width and then printed; it cannot reflow afterward.
- Optional scrollback insertion helper (`codex-rs/tui2/src/insert_history.rs`): once ANSI is written
  to the terminal, that output cannot be reflowed later. This helper is currently used for
  deterministic ANSI emission (`write_spans`) and tests; it is not wired into the main TUI draw
  loop.
- Static overlays (`codex-rs/tui2/src/pager_overlay.rs`): reflow depends on whether callers provided
  width-agnostic input; pre-split `Vec<Line>` cannot be “un-split” within the overlay.

## Deferred / follow-ups

The fix above is sufficient to unblock correct reflow on resize. Remaining choices can be deferred:

- Streaming granularity: one logical line can wrap into multiple visual lines, so “commit tick”
  updates can appear in larger chunks than before. If this becomes a UX issue, we can add a render-
  time “progressive reveal” layer without reintroducing width baking.
- Expand logical-line rendering to other markdown-ish cells if needed (e.g., unify `append_markdown`
  usage), but only if we find a concrete reflow bug beyond `AgentMessageCell`.
