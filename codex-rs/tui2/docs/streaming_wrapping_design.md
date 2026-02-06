# Streaming Markdown Wrapping & Animation – TUI2 Notes

This document mirrors the original `tui/streaming_wrapping_design.md` and
captures how the same concerns apply to the new `tui2` crate. It exists so that
future viewport and streaming work in TUI2 can rely on the same context without
having to cross‑reference the legacy TUI implementation.

At a high level, the design constraints are the same:

- Streaming agent responses are rendered incrementally, with an animation loop
  that reveals content over time.
- Non‑streaming history cells are rendered width‑agnostically and wrapped only
  at display time, so they reflow correctly when the terminal is resized.
- Streaming content should eventually follow the same “wrap on display” model so
  the transcript reflows consistently across width changes, without regressing
  animation or markdown semantics.

## 1. Where streaming is implemented in TUI2

TUI2 keeps the streaming pipeline conceptually aligned with the legacy TUI but
in a separate crate:

- `tui2/src/markdown_stream.rs` implements the markdown streaming collector and
  animation controller for agent deltas.
- `tui2/src/chatwidget.rs` integrates streamed content into the transcript via
  `HistoryCell` implementations.
- `tui2/src/history_cell.rs` provides the concrete history cell types used by
  the inline transcript and overlays.
- `tui2/src/wrapping.rs` contains the shared text wrapping utilities used by
  both streaming and non‑streaming render paths:
  - `RtOptions` describes viewport‑aware wrapping (width, indents, algorithm).
  - `word_wrap_line`, `word_wrap_lines`, and `word_wrap_lines_borrowed` provide
    span‑aware wrapping that preserves markdown styling and emoji width.

As in the original TUI, the key tension is between:

- **Pre‑wrapping streamed content at commit time** (simpler animation, but
  baked‑in splits that don’t reflow), and
- **Deferring wrapping to render time** (better reflow, but requires a more
  sophisticated streaming cell model or recomputation on each frame).

## 2. Current behavior and limitations

TUI2 is intentionally conservative for now:

- Streaming responses use the same markdown streaming and wrapping utilities as
  the legacy TUI, with width decisions made near the streaming collector.
- The transcript viewport (`App::render_transcript_cells` in
  `tui2/src/app.rs`) always uses `word_wrap_lines_borrowed` against the
  current `Rect` width, so:
  - Non‑streaming cells reflow naturally on resize.
  - Streamed cells respect whatever wrapping was applied when their lines were
    constructed, and may not fully “un‑wrap” if that work happened at a fixed
    width earlier in the pipeline.

This means TUI2 shares the same fundamental limitation documented in the
original design note: streamed paragraphs can retain historical wrap decisions
made at the time they were streamed, even if the viewport later grows wider.

## 3. Design directions (forward‑looking)

The options outlined in the legacy document apply here as well:

1. **Keep the current behavior but clarify tests and documentation.**
   - Ensure tests in `tui2/src/markdown_stream.rs`, `tui2/src/markdown_render.rs`,
     `tui2/src/history_cell.rs`, and `tui2/src/wrapping.rs` encode the current
     expectations around streaming, wrapping, and emoji / markdown styling.
2. **Move towards width‑agnostic streaming cells.**
   - Introduce a dedicated streaming history cell that stores the raw markdown
     buffer and lets `HistoryCell::display_lines(width)` perform both markdown
     rendering and wrapping based on the current viewport width.
   - Keep the commit animation logic expressed in terms of “logical” positions
     (e.g., number of tokens or lines committed) rather than pre‑wrapped visual
     lines at a fixed width.
3. **Hybrid “visual line count” model.**
   - Track committed visual lines as a scalar and re‑render the streamed prefix
     at the current width, revealing only the first `N` visual lines on each
     animation tick.

TUI2 does not yet implement these refactors; it intentionally stays close to
the legacy behavior while the viewport work (scrolling, selection, exit
transcripts) is being ported. This document exists to make that trade‑off
explicit for TUI2 and to provide a natural home for any TUI2‑specific streaming
wrapping notes as the design evolves.

