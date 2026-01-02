# TUI2 Viewport, Transcript, and History – Design Notes

This document describes the viewport and history model we are implementing in the new
`codex-rs/tui2` crate. It builds on lessons from the legacy TUI and explains why we moved away
from directly writing history into terminal scrollback.

The target audience is Codex developers and curious contributors who want to understand or
critique how TUI2 owns its viewport, scrollback, and suspend behavior.

Unless stated otherwise, references to “the TUI” in this document mean the TUI2 implementation;
when we mean the legacy TUI specifically, we call it out explicitly.

---

## 1. Problem Overview

Historically, the legacy TUI tried to “cooperate” with the terminal’s own scrollback:

- The inline viewport sat somewhere above the bottom of the screen.
- When new history arrived, we tried to insert it directly into the terminal scrollback above the
  viewport.
- On certain transitions (e.g. switching sessions, overlays), we cleared and re‑wrote portions of
  the screen from scratch.

This had several failure modes:

- **Terminal‑dependent behavior.**

  - Different terminals handle scroll regions, clears, and resize semantics differently.
  - What looked correct in one terminal could drop or duplicate content in another.

- **Resizes and layout churn.**

  - The TUI reacts to resizes, focus changes, and overlay transitions.
  - When the viewport moved or its size changed, our attempts to keep scrollback “aligned” with the
    in‑memory history could go out of sync.
  - In practice this meant:
    - Some lines were lost or overwritten.
    - Others were duplicated or appeared in unexpected places.

- **“Clear and rewrite everything” didn’t save us.**
  - We briefly tried a strategy of clearing large regions (or the full screen) and re‑rendering
    history when the layout changed.
  - This ran into two issues:
    - Terminals treat full clears differently. For example, Terminal.app often leaves the cleared
      screen as a “page” at the top of scrollback, some terminals interpret only a subset of the
      ANSI clear/scrollback codes, and others (like iTerm2) gate “clear full scrollback” behind
      explicit user consent.
    - Replaying a long session is expensive and still subject to timing/race conditions with user
      output (e.g. shell prompts) when we weren’t in alt screen.

The net result: the legacy TUI could not reliably guarantee “the history you see on screen is complete, in
order, and appears exactly once” across terminals, resizes, suspend/resume, and overlay transitions.

---

## 2. Goals

The redesign is guided by a few explicit goals:

1. **Codex, not the terminal, owns the viewport.**

   - The in‑memory transcript (a list of history entries) is the single source of truth for what’s
     on screen.
   - The TUI decides how to map that transcript into the current viewport; scrollback becomes an
     output target, not an extra data structure we try to maintain.

2. **History must be correct, ordered, and never silently dropped.**

   - Every logical history cell should either:
     - Be visible in the TUI, or
     - Have been printed into scrollback as part of a suspend/exit flow.
   - We would rather (rarely) duplicate content than risk losing it.

3. **Avoid unnecessary duplication.**

   - When emitting history to scrollback (on suspend or exit), print each logical cell’s content at
     most once.
   - Streaming cells are allowed to be “re‑seen” as they grow, but finished cells should not keep
     reappearing.

4. **Behave sensibly under resizes.**

   - TUI rendering should reflow to the current width on every frame.
   - History printed to scrollback may have been wrapped at different widths over time; that is
     acceptable, but it must not cause missing content or unbounded duplication.

5. **Suspend/alt‑screen interaction is predictable.**
   - `Ctrl+Z` should:
     - Cleanly exit alt screen, if active.
     - Print a consistent transcript prefix into normal scrollback.
     - Resume with the TUI fully redrawn, without stale artifacts.

---

## 3. New Viewport & Transcript Model

### 3.1 Transcript as a logical sequence of cells

At a high level, the TUI transcript is a list of “cells”, each representing one logical thing in
the conversation:

- A user prompt (with padding and a distinct background).
- An agent response (which may arrive in multiple streaming chunks).
- System or info rows (session headers, migration banners, reasoning summaries, etc.).

Each cell knows how to draw itself for a given width: how many lines it needs, what prefixes to
use, how to style its content. The transcript itself is purely logical:

- It has no scrollback coordinates or terminal state baked into it.
- It can be re‑rendered for any viewport width.

The TUI’s job is to take this logical sequence and decide how much of it fits into the current
viewport, and how it should be wrapped and styled on screen.

### 3.2 Building viewport lines from the transcript

To render the main transcript area above the composer, the TUI:

1. Defines a “transcript region” as the full frame minus the height of the bottom input area.
2. Flattens all cells into a list of visual lines, remembering for each visual line which cell it
   came from and which line within that cell it corresponds to.
3. Uses this flattened list plus a scroll position to decide which visual line should appear at the
   top of the region.
4. Clears the transcript region and draws the visible slice of lines into it.
5. For user messages, paints the entire row background (including padding lines) so the user block
   stands out even when it does not fill the whole width.
6. Applies selection styling and other overlays on top of the rendered lines.

Scrolling (mouse wheel, PgUp/PgDn, Home/End) operates entirely in terms of these flattened lines
and the current scroll anchor. The terminal’s own scrollback is not part of this calculation; it
only ever sees fully rendered frames.

### 3.3 Alternate screen, overlays, and redraw guarantees

The TUI uses the terminal’s alternate screen for:

- The main interactive chat session (so the viewport can cover the full terminal).
- Full‑screen overlays such as the transcript pager, diff view, model migration screen, and
  onboarding.

Conceptually:

- Entering alt screen:

  - Switches the terminal into alt screen and expands the viewport to cover the full terminal.
  - Clears that alt‑screen buffer.

- Leaving alt screen:

  - Disables “alternate scroll” so mouse wheel events behave predictably.
  - Returns to the normal screen.

- On leaving overlays and on resuming from suspend, the TUI viewport is explicitly cleared and fully
  redrawn:
  - This prevents stale overlay content or shell output from lingering in the TUI area.
  - The next frame reconstructs the UI entirely from the in‑memory transcript and other state, not
    from whatever the terminal happened to remember.

Alt screen is therefore treated as a temporary render target. The only authoritative copy of the UI
is the in‑memory state.

---

## 4. Mouse, Selection, and Scrolling

Mouse interaction is a first‑class part of the new design:

- **Scrolling.**

  - Mouse wheel scrolls the transcript in fixed line increments.
  - Keyboard shortcuts (PgUp/PgDn/Home/End) use the same scroll model, so the footer can show
    consistent hints regardless of input device.

- **Selection.**

  - A click‑and‑drag gesture defines a linear text selection in terms of the flattened transcript
    lines (not raw buffer coordinates).
  - Selection tracks the _content_ rather than a fixed screen row. When the transcript scrolls, the
    selection moves along with the underlying lines instead of staying glued to a particular Y
    position.
  - The selection only covers the “transcript text” area; it intentionally skips the left gutter
    that we use for bullets/prefixes.

- **Copy.**
  - When the user triggers copy, the TUI reconstructs the wrapped transcript lines using the same
    flattening/wrapping rules as the visible view.
  - It then reconstructs a high‑fidelity clipboard string from the selected logical lines:
    - Preserves meaningful indentation (especially for code blocks).
    - Treats soft-wrapped prose as a single logical line by joining wrap continuations instead of
      inserting hard newlines.
    - Emits Markdown source markers (e.g. backticks and fences) for copy/paste, even if the UI
      chooses to render those constructs without showing the literal markers.
  - Copy operates on the full selection range, even if the selection extends outside the current
    viewport.
  - The resulting text is sent to the system clipboard and a status footer indicates success or
    failure.

Because scrolling, selection, and copy all operate on the same flattened transcript representation,
they remain consistent even as the viewport resizes or the chat composer grows/shrinks. Owning our
own scrolling also means we must own mouse interactions end‑to‑end: if we left scrolling entirely
to the terminal, we could not reliably line up selections with transcript content or avoid
accidentally copying gutter/margin characters instead of just the conversation text.

Scroll normalization details and the data behind it live in
`codex-rs/tui2/docs/scroll_input_model.md`.

---

## 5. Printing History to Scrollback

We still want the final session (and suspend points) to appear in the user’s normal scrollback, but
we no longer try to maintain scrollback in lock‑step with the TUI frame. Instead, we treat
scrollback as an **append‑only log** of logical transcript cells.

In practice this means:

- The TUI may print history both when you suspend (`Ctrl+Z`) and when you exit.
- Some users may prefer to only print on exit (for example to keep scrollback quieter during long
  sessions). The current design anticipates gating suspend‑time printing behind a config toggle so
  that this behavior can be made opt‑in or opt‑out without touching the core viewport logic, but
  that switch has not been implemented yet.

### 5.1 Cell‑based high‑water mark

Internally, the TUI keeps a simple “high‑water mark” for history printing:

- Think of this as “how many cells at the front of the transcript have already been sent to
  scrollback.”
- It is just a counter over the logical transcript, not over wrapped lines.
- It moves forward only when we have actually printed more history.

This means we never try to guess “how many terminal lines have already been printed”; we only
remember that “the first N logical entries are done.”

### 5.2 Rendering new cells for scrollback

When we need to print history (on suspend or exit), we:

1. Take the suffix of the transcript that lies beyond the high‑water mark.
2. Render just that suffix into styled lines at the **current** terminal width.
3. Write those lines to stdout.
4. Advance the high‑water mark to include all cells we just printed.

Older cells are never re‑rendered for scrollback; they remain in whatever wrapping they had when
they were first printed. This avoids the line‑count–based bugs we had before while still allowing
the on‑screen TUI to reflow freely.

### 5.3 Suspend (`Ctrl+Z`) flow

On suspend (typically `Ctrl+Z` on Unix):

- Before yielding control back to the shell, the TUI:
  - Leaves alt screen if it is active and restores normal terminal modes.
  - Determines which transcript cells have not yet been printed and renders them for the current
    width.
  - Prints those new lines once into normal scrollback.
  - Marks those cells as printed in the high‑water mark.
  - Finally, sends the process to the background.

On `fg`, the process resumes, re‑enters TUI modes, and redraws the viewport from the in‑memory
transcript. The history printed during suspend stays in scrollback and is not touched again.

### 5.4 Exit flow

When the TUI exits, we follow the same principle:

- We compute the suffix of the transcript that has not yet been printed (taking into account any
  prior suspends).
- We render just that suffix to styled lines at the current width.
- The outer `main` function leaves alt screen, restores the terminal, and prints those lines, plus a
  blank line and token usage summary.

If you never suspended, exit prints the entire transcript once. If you did suspend one or more
times, exit prints only the cells appended after the last suspend. In both cases, each logical
conversation entry reaches scrollback exactly once.

---

## 6. Streaming, Width Changes, and Tradeoffs

### 6.1 Streaming cells

Streaming agent responses are represented as a sequence of history entries:

- The first chunk produces a “first line” entry for the message.
- Subsequent chunks produce continuation entries that extend that message.

From the history/scrollback perspective:

- Each streaming chunk is just another entry in the logical transcript.
- The high‑water mark is a simple count of how many entries at the _front_ of the transcript have
  already been printed.
- As new streaming chunks arrive, they are appended as new entries and will be included the next
  time we print history on suspend or exit.

We do **not** attempt to reprint or retroactively merge older chunks. In scrollback you will see the
streaming response as a series of discrete blocks, matching the internal history structure.

Today, streaming rendering still “bakes in” some width at the time chunks are committed: line breaks
for the streaming path are computed using the width that was active at the time, and stored in the
intermediate representation. This is a known limitation and is called out in more detail in
`codex-rs/tui2/docs/streaming_wrapping_design.md`; a follow‑up change will make streaming behavior
match the rest of the transcript more closely (wrap only at display time, not at commit time).

### 6.2 Width changes over time

Because we now use a **cell‑level** high‑water mark instead of a visual line‑count, width changes
are handled gracefully:

- On every suspend/exit, we render the not‑yet‑printed suffix of the transcript at the **current**
  width and append those lines.
- Previously printed entries remain in scrollback with whatever wrapping they had at the time they
  were printed.
- We no longer rely on “N lines printed before, therefore skip N lines of the newly wrapped
  transcript,” which was the source of dropped and duplicated content when widths changed.

This does mean scrollback can contain older cells wrapped for narrower or wider widths than the
final terminal size, but:

- Each logical cell’s content appears exactly once.
- New cells are append‑only and never overwrite or implicitly “shrink” earlier content.
- The on‑screen TUI always reflows to the current width independently of scrollback.

If we later choose to also re‑emit the “currently streaming” cell when printing on suspend (to make
sure the latest chunk of a long answer is always visible in scrollback), that would intentionally
duplicate a small number of lines at the boundary of that cell. The design assumes any such behavior
would be controlled by configuration (for example, by disabling suspend‑time printing entirely for
users who prefer only exit‑time output).

### 6.3 Why not reflow scrollback?

In theory we could try to reflow already‑printed content when widths change by:

- Recomputing the entire transcript at the new width, and
- Printing diffs that “rewrite” old regions in scrollback.

In practice, this runs into the same issues that motivated the redesign:

- Terminals treat full clears and scroll regions differently.
- There is no portable way to “rewrite” arbitrary portions of scrollback above the visible buffer.
- Interleaving user output (e.g. shell prompts after suspend) makes it impossible to reliably
  reconstruct the original scrollback structure.

We therefore deliberately accept that scrollback is **append‑only** and not subject to reflow;
correctness is measured in terms of logical transcript content, not pixel‑perfect layout.

---

## 7. Backtrack and Overlays (Context)

While this document is focused on viewport and history, it’s worth mentioning a few related
behaviors that rely on the same model.

### 7.1 Transcript overlay and backtrack

The transcript overlay (pager) is a full‑screen view of the same logical transcript:

- When opened, it takes a snapshot of the current transcript and renders it in an alt‑screen
  overlay.
- Backtrack mode (`Esc` sequences) walks backwards through user messages in that snapshot and
  highlights the candidate “edit from here” point.
- Confirming a backtrack request forks the conversation on the server and trims the in‑memory
  transcript so that only history up to the chosen user message remains, then re‑renders that prefix
  in the main view.

The overlay is purely a different _view_ of the same transcript; it never infers anything from
scrollback.

---

## 8. Summary of Tradeoffs

**What we gain:**

- The TUI has a clear, single source of truth for history (the in‑memory transcript).
- Viewport rendering is deterministic and independent of scrollback.
- Suspend and exit flows:
  - Print each logical history cell exactly once.
  - Are robust to terminal width changes.
  - Interact cleanly with alt screen and raw‑mode toggling.
- Streaming, overlays, selection, and backtrack all share the same logical history model.
- Because cells are always re‑rendered live from the transcript, per‑cell interactions can become
  richer over time. Instead of treating the transcript as “dead text”, we can make individual
  entries interactive after they are rendered: expanding or contracting tool calls, diffs, or
  reasoning summaries in place, jum…truncated… \*\*\*

---

## 9. TUI2 Implementation Notes

This section maps the design above onto the `codex-rs/tui2` crate so future viewport work has
concrete code pointers.

### 9.1 Transcript state and layout

The main app struct (`codex-rs/tui2/src/app.rs`) tracks the transcript and viewport state with:

- `transcript_cells: Vec<Arc<dyn HistoryCell>>` – the logical history.
- `transcript_scroll: TranscriptScroll` – whether the viewport is pinned to the bottom or
  anchored at a specific cell/line pair.
- `transcript_selection: TranscriptSelection` – a selection expressed in content-relative
  coordinates over the flattened, wrapped transcript (line index + column).
- `transcript_view_top` / `transcript_total_lines` – the current viewport’s top line index and
  total number of wrapped lines for the inline transcript area.

### 9.2 Rendering, wrapping, and selection

`App::render_transcript_cells` defines the transcript region, builds flattened lines via
`App::build_transcript_lines`, wraps them with `word_wrap_lines_borrowed` from
`codex-rs/tui2/src/wrapping.rs`, and applies selection via `apply_transcript_selection` before
writing to the frame buffer.

Streaming wrapping details live in `codex-rs/tui2/docs/streaming_wrapping_design.md`.

### 9.3 Input, selection, and footer state

Mouse handling lives in `App::handle_mouse_event`, keyboard scrolling in
`App::handle_key_event`, selection rendering in `App::apply_transcript_selection`, and copy in
`App::copy_transcript_selection` plus `codex-rs/tui2/src/transcript_selection.rs` and
`codex-rs/tui2/src/clipboard_copy.rs`. Scroll/selection UI state is forwarded through
`ChatWidget::set_transcript_ui_state`,
`BottomPane::set_transcript_ui_state`, and `ChatComposer::footer_props`, with footer text
assembled in `codex-rs/tui2/src/bottom_pane/footer.rs`.

### 9.4 Exit transcript output

`App::run` returns `session_lines` on `AppExitInfo` after flattening with
`App::build_transcript_lines` and converting to ANSI via `App::render_lines_to_ansi`. The CLI
prints those lines before the token usage and resume hints.

## 10. Future Work and Open Questions

### 10.1 Current status

This design shipped behind the `tui2` feature flag (as a separate crate, duplicating the legacy
`tui` crate to enable rollout without breaking existing behavior). The following items from early
feedback are already implemented:

- Bottom pane positioning is pegged high with an empty transcript and moves down as the transcript
  fills (including on resume).
- Wheel-based transcript scrolling uses the stream-based normalization model derived from scroll
  probe data (see `codex-rs/tui2/docs/scroll_input_model.md`).
- While a selection is active, streaming stops “follow latest output” so the selection remains
  stable, and follow mode resumes after the selection is cleared.
- Copy operates on the full selection range (including offscreen lines), using the same wrapping as
  on-screen rendering.
- Copy selection uses `Ctrl+Shift+C` (VS Code uses `Ctrl+Y` because `Ctrl+Shift+C` is unavailable in
  the terminal) and shows an on-screen “copy” affordance near the selection.

### 10.2 Roadmap (prioritized)

This section captures a prioritized list of improvements we want to add to TUI2 based on early
feedback, with the goal of making scrolling/selection/copy feel as close to “native terminal” (and
Vim) behavior as we can while still owning the viewport.

**P0 — must-have (usability/correctness):**

- **Scrolling behavior.** Default to a classic multi-line wheel tick (3 lines, configurable) with
  acceleration/velocity for faster navigation, and ensure we stop scrolling when the user stops
  input (avoid redraw/event-loop backlog that makes scrolling feel “janky”).
- **Mouse event bounds.** Ignore mouse events outside the transcript region so clicks in the
  composer/footer don’t start or mutate transcript selection state.
- **Copy fidelity.** Preserve meaningful indentation (especially code blocks), treat soft-wrapped
  prose as a single logical line when copying, and copy markdown _source_ (including backticks and
  heading markers) even if we render it differently.

**P1 — should-have (UX polish and power user workflows):**

- **Streaming wrapping polish.** Ensure all streaming paths use display-time wrapping only, and add
  tests that cover resizing after streaming has started.
- **Selection semantics.** Define and implement selection behavior across multi-step output (and
  whether step boundaries should be copy boundaries), while continuing to exclude the left gutter
  from copied text.
- **Auto-scroll during drag.** While dragging a selection, auto-scroll when the cursor is at/near the
  top or bottom of the transcript viewport to allow selecting beyond the visible window.
- **Width-aware selection.** Ensure selection highlighting and copy reconstruction handle wide glyphs
  correctly (emoji, CJK), matching terminal display width rather than raw character count.
- **Multi-click selection.** Support double/triple/quad click selection (word/line/paragraph),
  implemented on top of the transcript/viewport model rather than terminal buffer coordinates.
- **Find in transcript.** Add text search over the transcript (and consider integrating match
  markers with any future scroll indicator work).
- **Cross-terminal behavior checks.** Validate copy/selection behavior across common terminals (incl.
  terminal-provided “override selection” modes like holding Shift) and document the tradeoffs.

**P2 — nice-to-have (polish, configuration, and interactivity):**

- **Suspend printing.** Decide whether printing history on suspend is desirable at all (it is not
  implemented yet). If we keep it, finalize the config shape/defaults, wire it through TUI startup,
  and document it in the appropriate config docs.
- **Terminal integration.** Consider guiding (or optionally managing) terminal-emulator-specific
  settings that affect TUI behavior (for example iTerm’s clipboard opt-in prompts or Ghostty
  keybinding quirks), so the “works well out of the box” path is consistent across terminals.
- **Interactive cells (unlocked by transcript ownership).** Because transcript entries are structured
  objects (not dead text in terminal scrollback), we can attach metadata to rendered regions and map
  mouse/keys back to the underlying cell reliably across resizes and reflow. Examples:
  - **Drill into a specific tool/command output.** Click (or press Enter) on a tool call / command
    cell to open a focused overlay that shows the command, exit status, timing, and stdout/stderr as
    separate sections, with dedicated “copy output” actions. This enables copying _just_ one command’s
    output even when multiple commands are interleaved in a turn.
  - **Copy an entire cell or entire turn.** Provide an action to copy a whole logical unit (one cell,
    or “user prompt + assistant response”), without gutters and with well-defined boundaries. This is
    hard to do with raw selection because step boundaries and padding aren’t reliably expressible in
    terminal coordinates once the viewport moves or reflows.
  - **Expand/collapse structured subregions with source-aware copy.** Tool calls, diffs, and
    markdown can render in a compact form by default and expand in place. Copy actions can choose
    between “copy rendered view” and “copy source” (e.g. raw markdown, raw JSON arguments, raw diff),
    since we retain the original source alongside the rendered lines.
  - **Cell-scoped actions.** Actions like “copy command”, “yank into composer”, “retry tool call”, or
    “open related view” (diff/pager) can be offered per cell and behave deterministically, because the
    UI can address cells by stable IDs rather than by fragile screen coordinates.
- **Additional affordances.** Consider an ephemeral scrollbar and/or a more explicit “selecting…”
  status if footer hints aren’t sufficient.
- **UX capture.** Maintain short “golden path” clips showing scrolling (mouse + keys), selection and
  copy, streaming under resize, and suspend/resume + exit printing.

### 10.3 Open questions

This section collects design questions that follow naturally from the current model and are worth
explicit discussion before we commit to further UI changes.

- **“Scroll mode” vs “live follow” UI.**

  - We already distinguish “scrolled away from bottom” vs “following the latest output” in the
    footer and scroll state. Do we need a more explicit “scroll mode vs live mode” affordance (e.g.,
    a dedicated indicator or toggle), or is the current behavior sufficient and adding more chrome
    would be noise?

- **Ephemeral scroll indicator.**

  - For long sessions, a more visible sense of “where am I?” could help. One option is a minimalist
    scrollbar that appears while the user is actively scrolling and fades out when idle. A full
    “mini‑map” is probably too heavy for a TUI given the limited vertical space, but we could
    imagine adding simple markers along the scrollbar to show where prior prompts occurred, or
    where text search matches are, without trying to render a full preview of the buffer.

- **Selection affordances.**

  - Today, the primary hint that selection is active is the reversed text plus the on-screen “copy”
    affordance (`Ctrl+Shift+C`) and the footer hint. Do we want an explicit “Selecting… (Esc to
    cancel)” status while a drag is in progress, or would that be redundant/clutter for most users?

- **Suspend banners in scrollback.**

  - When printing history on suspend, should we also emit a small banner such as
    `--- codex suspended; history up to here ---` to make those boundaries obvious in scrollback?
    This would slightly increase noise but could make multi‑suspend sessions easier to read.

- **Configuring suspend printing behavior.**

  - The design already assumes that suspend‑time printing can be gated by config. Questions to
    resolve:
    - Should printing on suspend be on or off by default?
    - Should we support multiple modes (e.g., “off”, “print all new cells”, “print streaming cell
      tail only”) or keep it binary?

- **Streaming duplication at the edges.**
  - If we later choose to always re‑emit the “currently streaming” message when printing on suspend,
    we would intentionally allow a small amount of duplication at the boundary of that message (for
    example, its last line appearing twice across suspends). Is that acceptable if it improves the
    readability of long streaming answers in scrollback, and should the ability to disable
    suspend‑time printing be our escape hatch for users who care about exact de‑duplication?\*\*\*

---
