# Performance testing (`codex-tui2`)

This doc captures a repeatable workflow for investigating `codex-tui2` performance issues
(especially high idle CPU and high CPU while streaming) and validating optimizations to the draw
hot path.

## Scope (this round)

The current focus is the transcript draw hot path, specifically the cost of repeatedly rendering
the same visible transcript lines via Ratatui’s `Line::render_ref` (notably grapheme segmentation
and span layout).

The intended mitigation is a **rasterization cache**: render a wrapped transcript `Line` into a
row of `Cell`s once, cache it, and on subsequent redraws copy cached cells into the frame buffer.

Key invariants:

- The cache is width-scoped (invalidate on terminal width changes).
- The cache stores **base content** only; selection highlight and copy affordances are applied
  after rendering, so they don’t pollute cached rows.

## Roles

- Human: runs `codex-tui2` in an interactive terminal (e.g. Ghostty), triggers “idle” and
  “streaming” scenarios, and captures profiles.
- Assistant (or a script): reads profile output and extracts hotspots and deltas.

## Baseline setup

Build from a clean checkout:

```sh
cd codex-rs
cargo build -p codex-tui2
```

Run `codex-tui2` in a terminal and get a PID (macOS):

```sh
pgrep -n codex-tui2
```

Track CPU quickly while reproducing:

```sh
top -pid "$(pgrep -n codex-tui2)"
```

## Capture profiles (macOS)

Capture both an “idle” and a “streaming” profile so hotspots are not conflated:

```sh
sample "$(pgrep -n codex-tui2)" 1 -file /tmp/tui2.idle.sample.txt
sample "$(pgrep -n codex-tui2)" 1 -file /tmp/tui2.streaming.sample.txt
```

For the streaming sample, trigger a response that emits many deltas (e.g. “Tell me a story”) so
the stream runs long enough to sample.

## Quick hotspot extraction

These `rg` patterns keep the investigation grounded in the data:

```sh
# Buffer diff hot path (idle)
rg -n "custom_terminal::diff_buffers|diff_buffers" /tmp/tui2.*.sample.txt | head -n 80

# Transcript rendering hot path (streaming)
rg -n "App::render_transcript_cells|Line::render|render_spans|styled_graphemes|GraphemeCursor::next_boundary" /tmp/tui2.*.sample.txt | head -n 120
```

## Rasterization-cache validation checklist

After implementing a transcript rasterization cache, re-run the same scenarios and confirm:

- Streaming sample shifts away from `unicode_segmentation::grapheme::GraphemeCursor::next_boundary`
  stacks dominating the main thread.
- CPU during streaming drops materially vs baseline for the same streaming load.
- Idle CPU does not regress (redraw gating changes can mask rendering improvements; always measure
  both idle and streaming).

## Notes to record per run

- Terminal size: width × height
- Scenario: idle vs streaming (prompt + approximate response length)
- CPU snapshot: `top` (directional)
- Profile excerpt: 20–50 relevant lines for the dominant stacks

## Code pointers

- `codex-rs/tui2/src/transcript_view_cache.rs`: wrapped transcript memoization + per-line
  rasterization cache (cached `Cell` rows).
- `codex-rs/tui2/src/transcript_render.rs`: incremental helper used by the wrapped-line cache
  (`append_wrapped_transcript_cell`).
- `codex-rs/tui2/src/app.rs`: wiring in `App::render_transcript_cells` (uses cached rows instead of
  calling `Line::render_ref` every frame).
