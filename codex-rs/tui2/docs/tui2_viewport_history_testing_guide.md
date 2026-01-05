# TUI2 viewport/history: tester-facing overview (DRAFT)

Status: **draft**

This is a short, team-facing guide for testing and triaging the recent TUI2
viewport/history changes.

For the full architecture and PR index, start with:

- `tui2/docs/tui2_viewport_history_architecture.md`

---

## What changed (high level)

TUI2 now treats “scrollback” as an app-owned transcript, not the terminal’s native
scrollback buffer. The transcript is wrapped, cached, and rendered into an on-screen
viewport each frame.

That shift enables deterministic scrolling, selection, and copy across terminal
implementations and mode changes, at the cost of moving more complexity into the app
(wrapping/caching/scroll anchors/selection mapping).

---

## Why it matters (what to look for as a tester)

The main promise of the work is that the transcript behaves consistently:

- Scrolling should feel “native” (wheel and trackpad) and should not jump, duplicate,
  or lose lines across resizes or mode changes.
- Selection/copy should match logical content (not pixels), should ignore non-content
  UI regions (like the gutter), and should allow copying beyond what’s currently
  visible.

When bugs show up, they often look like correctness failures (“the transcript view is
out of sync”) or UX failures (“it feels wrong compared to my terminal”).

---

## How to enable TUI2 (and confirm you’re in it)

Codex selects the TUI implementation at runtime based on the `tui2` feature flag.

### Enable via config

In `config.toml`:

```toml
[features]
tui2 = true
```

### Enable via one-off CLI override

When launching `codex`:

```bash
codex -c features.tui2=true
```

### Quick confirmation cues

These are not “official”, but they tend to be fast signals you’re in the new flow:

- The footer advertises a “copy selection” shortcut (usually `Ctrl+Shift+C`, but
  `Ctrl+Y` in VS Code’s integrated terminal).
- A small on-screen “⧉ copy …” pill appears near an active transcript selection.
- Scrolling behavior feels normalized across wheel densities (1 vs 3 vs 9+ events per
  notch).

---

## Test focus areas (prioritized)

These sections are written to be runnable as manual test checklists. Each starts with
the “why” and then lists concrete things to try.

### Scroll + viewport stability (P0)

This is the core correctness bar: transcript content should render exactly once, in
order, and scrolling should be stable at history cell boundaries.

Try:

- Produce enough output to require scrolling (a few screens).
- Scroll with a wheel and with a trackpad (if available).
- Repeatedly scroll across “cell boundaries” (where a history entry changes from one
  cell to the next) and look for “stickiness”, jumps, or repeated lines.
- Use keyboard scrolling if you rely on it (PageUp/PageDown/Home/End).
- Resize the terminal while scrolled up:
  - Verify the same content remains visible (no sudden jumps to bottom).
  - Verify wrapping reflows without duplicating or dropping lines.

If you can, repeat the same checks in multiple terminals (especially one with dense
wheel events, and one with sparse wheel events). See `tui2/docs/scroll_input_model.md`
for the motivation and the knobs that can help diagnose “scroll feels wrong”.

### Selection + copy (P0)

Selection is transcript-relative, not terminal-row-relative. That means the selection
can extend beyond the visible viewport and can be reconstructed for copy.

Try:

- Click-drag to select transcript text; verify selection does not start outside the
  transcript region (e.g. the left gutter).
- Multi-click selection expansion (word → line → larger scopes) and confirm it does
  not accidentally include UI chrome.
- Extend selection beyond the viewport (drag, then scroll) and copy it:
  - Use the shortcut shown in the footer (`Ctrl+Shift+C` or `Ctrl+Y`).
  - Click the on-screen “⧉ copy …” pill.
- Verify copied text matches logical content:
  - Soft-wrapped prose should copy as a paragraph, not with hard line breaks.
  - Preformatted blocks should preserve indentation.

Notes:

- VS Code’s integrated terminal often intercepts `Ctrl+Shift+C`, so TUI2 falls back to
  advertising/accepting `Ctrl+Y` there.

### Streaming output + wrapping (P1)

Streaming is intentionally “partially solved”: we favor conservative behavior to avoid
regressions, but resize/reflow mid-stream is still a risk area.

Try:

- Stream long markdown content (lists, headings, code blocks).
- Resize the terminal while streaming is still producing output.
- Watch for:
  - Duplicate lines during reflow.
  - Dropped lines (missing chunks).
  - Over-aggressive rewrapping that makes the transcript feel unstable while output is
    still arriving.

For deeper context, see `tui2/docs/streaming_wrapping_design.md`.

### Overlays / backtrack / pager interactions (P1)

TUI2 uses overlays (pager/backtrack-style modes) that interact with alt-screen and
render state. Bugs here often present as “screen corruption” when entering/exiting an
overlay.

Try:

- Enter/exit overlays repeatedly (whatever your workflow uses: pager, backtrack, etc.).
- Resize while an overlay is active, then exit it.
- Verify you return to a correct transcript viewport with no persistent corruption.

### Exit + suspend semantics (P2)

Exit printing is the “must-have” contract: leaving TUI2 should print the session’s
transcript to the normal terminal scrollback.

Try:

- Exit normally and confirm the transcript appears in your terminal scrollback.
- Suspend/resume (if you use it) and note current behavior:
  - “Print transcript on suspend” is not implemented yet (known gap).

### Performance + redraw behavior (P2)

Recent work adds redraw coalescing, a 60fps cap, and transcript view caching. The goal
is “no jank” during scroll/selection and no event-loop backlog under high input rate.

Try:

- Rapid scrolling (wheel bursts and fast trackpad swipes).
- Rapid resize (dragging the window size back and forth).
- Long sessions (lots of transcript content) and then repeated selection/copy.
- Watch for:
  - Input lag (scroll events applying late).
  - Excessive CPU usage when idle.
  - Flicker or repeated redraw of unchanged content.

---

## Diagnostics (what to capture in a bug report)

The fastest bug reports for this area include both “what happened” and the context
that impacts terminal behavior.

Capture:

- Terminal + OS (e.g. WezTerm on macOS, iTerm2, VS Code integrated terminal).
- Whether the footer shows `Ctrl+Shift+C` or `Ctrl+Y` for copy selection.
- Whether the issue involves wheel vs trackpad scrolling (or `scroll_mode` overrides).
- A short description of the content type involved (prose vs code block vs mixed markdown).
- Whether a resize or overlay transition occurred right before the issue.

Optional: record a JSONL session log for replay/analysis:

```bash
CODEX_TUI_RECORD_SESSION=1 \
CODEX_TUI_SESSION_LOG_PATH=/tmp/codex-session.jsonl \
codex -c features.tui2=true
```

---

## Key PRs to skim (by testing area)

This is a small, tester-oriented subset. For the full PR map, see the “PR index”
section in `tui2/docs/tui2_viewport_history_architecture.md`.

| Area                         | PRs                           | Notes                         |
| ---------------------------- | ----------------------------- | ----------------------------- |
| Enable/dispatch              | [#7793]                       | Flag-gated TUI2 from `codex`  |
| Scroll normalization         | [#8252], [#8357], [#8695]     | Stream model + stickiness fix |
| Selection gestures           | [#8419], [#8466], [#8471]     | Bounds + drag + multi-click   |
| Copy UX                      | [#8449], [#8462], [#8716]     | Off-screen copy + pill hint   |
| Perf/redraw                  | [#8295], [#8499], [#8693]     | Coalesce + 60fps + caching    |
| Overlay stability            | [#8463]                       | Nested alt-screen corruption  |

[#7793]: https://github.com/openai/codex/pull/7793
[#8252]: https://github.com/openai/codex/pull/8252
[#8295]: https://github.com/openai/codex/pull/8295
[#8357]: https://github.com/openai/codex/pull/8357
[#8419]: https://github.com/openai/codex/pull/8419
[#8449]: https://github.com/openai/codex/pull/8449
[#8462]: https://github.com/openai/codex/pull/8462
[#8463]: https://github.com/openai/codex/pull/8463
[#8466]: https://github.com/openai/codex/pull/8466
[#8471]: https://github.com/openai/codex/pull/8471
[#8499]: https://github.com/openai/codex/pull/8499
[#8693]: https://github.com/openai/codex/pull/8693
[#8695]: https://github.com/openai/codex/pull/8695
[#8716]: https://github.com/openai/codex/pull/8716

---

## Known gaps / current limitations

These are called out in more detail (with context and references) in the architecture
doc’s “What’s missing / gaps” section. They’re repeated here because they affect what
“complete” testing looks like.

- “Print transcript on suspend” is not implemented yet.
- Auto-scroll while dragging selection near viewport edges is not implemented yet.
- Streaming resize/reflow behavior is conservative and still needs broader test coverage.

---

## Further reading

- `tui2/docs/tui2_viewport_history_architecture.md` (architecture + PR index + roadmap)
- `tui2/docs/tui2_viewport_history_notes.md` (running notes + archaeology commands)
- `tui2/docs/scroll_input_model.md` (wheel/trackpad model and per-terminal defaults)
- `tui2/docs/streaming_wrapping_design.md` (streaming wrapping constraints)
