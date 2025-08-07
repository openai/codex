Plan to restore streaming Markdown in the TUI

Goal
- Prioritize Markdown correctness for assistant reasoning and answers in the TUI.
- Buffer text until an explicit newline is observed, then render and commit complete logical lines as styled Markdown.

Why this revision?
- We prefer correctness over responsiveness: many outputs are long paragraphs or multi-line markdown constructs. Waiting for a newline ensures complete context and avoids streaming artifacts, duplication, and mid-construct styling glitches.
- We render only completed logical lines.

Current state (reference)
- Live streaming uses RowBuilder (tui/src/live_wrap.rs) for soft wrapping and paints a live overlay ring in the bottom pane.
- History insertion uses insert_history::insert_history_lines(..) to write styled ratatui::text::Line<'static> above the viewport.
- Markdown rendering lives in tui/src/markdown.rs::append_markdown(..).
- We have a streaming accumulator in tui/src/markdown_stream.rs that currently focuses on committing on newline boundaries.

High-level approach (newline-gated, with rendered streaming)
- Collect deltas in memory until an explicit newline ("\n").
- On each newline:
  1) Re-render the entire buffer via append_markdown(..) to a Vec<Line<'static>>.
  2) Consider only fully completed logical lines: all rendered lines except the last one when the buffer does not end with a newline.
  3) Compute the delta since the last commit and enqueue the new completed lines into a RenderedLineStreamer.
  4) The streamer incrementally inserts rows into history while keeping the last K rows in the live ring, producing a smooth streaming feel from already-correct markdown.
  5) We do NOT display interrupt events such as tool call outputs or approval interrupts until we've fully committed ALL streaming text. We'll need to queue writes and tool outputs in the same data structure.
- On finalize, if the buffer does not end with a newline, append a temporary newline, render, enqueue remaining lines, drain the streamer, and then insert a trailing blank spacer line.

Key design choices
- Commit only at explicit newline boundaries (or finalize). This maximizes correctness for block-level markdown and avoids optimistic closure.
- Previously committed lines never change; history is append-only.
- The live ring shows only correctly rendered rows; it begins streaming once a newline makes rows available.

Alignment with the existing codebase
- insert_history works with full Lines; in the newline-gated path, we commit whole logical lines only, so no span-aware splitting is needed during streaming.
- ChatWidget currently uses RowBuilder to stream live plain text; in the newline-gated mode we stop pushing partial content to the live ring. Instead, on newline we commit markdown-rendered lines to history and mirror those lines to the live ring so the UI reflects the newly added content.
- Finalize semantics and ```markdown unwrap: Keep unwrap_markdown_language_fence applied only during finalize (and skipped under cfg(test)), ensuring we never rewrite prior history during the stream.

Technical design
1) MarkdownNewlineCollector
   - Fields:
     - buffer: String — concatenation of all deltas for the active stream so far.
     - committed_line_count: usize — how many rendered markdown lines have been committed to history.
   - Methods:
     - push_delta(&mut self, delta: &str): append raw text to buffer.
     - commit_complete_lines(&mut self, config: &Config) -> Vec<Line<'static>>:
       - Render the full buffer via markdown::append_markdown(..) into a Vec<Line<'static>>.
       - Compute complete_line_count: if buffer ends with '\n', all rendered lines; else all except the final line.
       - Emit only rendered[committed_line_count..complete_line_count], then set committed_line_count = complete_line_count.
     - finalize_and_drain(&mut self, config: &Config) -> Vec<Line<'static>>:
       - If buffer does not end with '\n', temporarily append one for rendering. Optionally unwrap ```markdown in non-test builds. Emit all lines beyond committed_line_count.

2) RenderedLineStreamer
   - Purpose: provide the streaming feel using already-correct, rendered Lines.
   - Fields:
     - queue: VecDeque<Line<'static>> — pending rendered rows to stream.
   - Methods:
     - enqueue(Vec<Line<'static>>): push new rows to the queue.
     - step(live_max_rows: usize) -> { history: Vec<Line<'static>>, live: Vec<Line<'static>> }:
       - Move some rows from queue to history (at least 1 per step, or more for bursts), return them for InsertHistory.
       - Compute the last K rows (history tail + queue head) to display in the live ring.
     - drain_all(live_max_rows) -> same return type, but empty the queue.

3) ChatWidget integration
   - Maintain one collector and one streamer per active stream (reasoning/answer).
   - On AgentReasoningDelta/AgentMessageDelta: push delta into the collector only. If the delta contains '\n', call collector.commit_complete_lines(..) and streamer.enqueue(those lines). Trigger a streamer step (timer or on-next-delta) to insert rows into history and update the live ring.
   - On AgentReasoning/AgentMessage (final): call finalize_and_drain(..), enqueue the remainder, then drain the streamer and append a trailing blank spacer line.
   - Emit the stream header once per stream before the first history insertion the streamer produces.

4) Files to touch
   - tui/src/chatwidget.rs: swap streaming to the collector; only insert to history (and mirror to live ring) when newline or finalize occurs; ensure headers are emitted once.
   - tui/src/markdown_stream.rs: implement MarkdownNewlineCollector as described; keep existing accumulator helpers for potential optional modes.
   - tui/src/markdown.rs: unchanged.
   - (tests) Add a small public test-only simulate_stream_markdown_for_tests helper that feeds deltas through the collector and returns Lines for insert_history.

Testing strategy
- Principles
  - Tests should validate both absence of duplication and correct flush-on-wrap behaviour.
  - Prefer end-to-end vt100 screen assertions (via TestBackend + insert_history) over fragile unit assumptions.
  - Keep widths small and deterministic so wrap points are predictable.

- Unit tests (markdown_stream.rs)
  - Optimistic closer: feed incomplete inline constructs and ensure the output either closes or removes dangling tokens on the final line.
  - Visual row accounting: given rendered Lines and a width, ensure we count visual rows predictably and do not over-commit the trailing row.

- Integration tests (tui/tests, feature = "vt100-tests")
  1) No commit until newline
     - Stream a long paragraph without newlines; assert no history insertions until a newline or finalize occurs.
  2) Inline formatting correctness
     - Stream sentences with bold/italic/link constructs and ensure committed lines render with correct SGRs and without duplication; since commits are newline-gated, we do not need optimistic closure.
  3) Fenced code block streamed slowly
     - Stream "```\ncode line\n```\n" in token-sized deltas; assert no duplication, and only the lines completed by newline are committed; finalize flushes the remainder.
  4) Rendered streaming feel
     - After a newline introduces multiple completed rows, assert rows trickle into history and the live ring shows the newest K rows and a partially revealed head. No duplication and stable ordering.
  8) Thinking visibility and finalize trickle
     - Thinking header visible before first newline (reasoning stream) and added to history at first content commit
     - Finalize does not flush; characters trickle into history after finalize until complete
     - Working status remains visible while trickling characters (including after finalize)
  9) Live ring wrapping while streaming
     - With a narrow viewport, ensure the live ring shows wrapped rows for both committed tail and the partially revealed head; verify wide characters (emoji/CJK) and combining marks render without truncation or duplication
  5) Header once when newline and overflow coincide
     - Maintain existing test to ensure headers are emitted a single time.
  6) Reasoning then answer ordering
     - Maintain existing ordering test for headers and content.
  7) Markdown language fence unwrap (future)
     - Keep an ignored test that asserts that ```markdown wrappers are unwrapped on finalize and not shown to the user.

Acceptance criteria
- As the model streams, no content is committed until a newline is observed; at newline, correctly rendered rows begin streaming into history while the live ring displays the newest K rows.
- Already-committed history lines never change after they are inserted.
- The final transcript is well-formed Markdown rendering for headers, lists, links, inline code, fenced code blocks, and blockquotes.
- Integration tests above pass reliably across platforms and terminal widths used in tests.
 - Live ring respects wrapping: partially revealed head and committed tail are wrapped according to viewport width, including correct handling of wide graphemes (emoji/CJK) and combining characters.

Notes and open questions
- Tables: tui_markdown supports a subset; we treat each new wrapped row as a commit opportunity and rely on the renderer for formatting.
- Syntax highlighting: Out of scope for now.

Thinking/Reasoning UX acceptance
- The “thinking” block is visible immediately when the reasoning stream begins (live overlay), even before the first newline produces content.
- The “thinking” header is inserted into history at the first history insertion for the reasoning stream (or an equivalent policy we adopt), and reasoning content follows the same newline-gated, character‑trickle pipeline as the answer.
- The Working status bar remains visible until the last trickled character is printed (including post‑finalize trickle).


Progress checklists

Core streaming model
- [x] Implement newline-gated accumulator that re-renders the full buffer and commits only fully completed logical lines (tui/src/markdown_stream.rs)
- [x] Finalize path renders with a temporary trailing newline when needed
- [x] Unwrap ```markdown fences only at finalize and skip in tests via cfg(test)
- [ ] Add helper to expose “current partial line” if we need it for diagnostics or optional modes
- [x] Add helper to expose “current partial line” if we need it for diagnostics or optional modes

Rendered streaming pipeline
- [x] Introduce RenderedLineStreamer (queue of already-rendered Lines)
- [x] Streamer step() inserts a batch into history and computes live ring rows (last K)
- [x] Streamer drain_all() drains queue and updates live ring once

ChatWidget integration
- [x] Maintain one accumulator per active stream (reasoning, answer)
- [x] On delta push: append to accumulator; if delta contains a newline, commit_complete_lines and enqueue via streamer
- [x] On finalize: finalize_and_drain, enqueue remainder, then drain streamer and append a trailing blank spacer line
- [x] Emit stream header exactly once per stream, right before the first history insertion produced by the streamer
- [x] Stop committing plain RowBuilder rows to history; rely on rendered markdown lines
- [x] Keep live ring in sync with newest K rendered rows (history tail + queue head)
- [x] Remove or repurpose unused buffers (content_buffer, answer_buffer) in ChatWidget
- [x] Remove the TODO in finalize_stream and route through markdown rendering

Thinking/Reasoning streaming UX
- [x] Thinking block participates in the same pipeline as answer content (newline‑gated commits, character trickle in the live ring)
- [x] Show “thinking” immediately in the live overlay on stream start; insert header into history at first content commit (or chosen equivalent policy)
- [ ] Keep Working status visible until last trickled character prints (including post‑finalize)
- [x] Keep Working status visible until last trickled character prints (including post‑finalize)

Testing – unit (markdown_stream.rs)
- [ ] No commit until newline; commit only fully completed lines
- [ ] Finalize commits partial last line
- [x] No commit until newline; commit only fully completed lines
- [x] Finalize commits partial last line
- [ ] Lists and fences commit as logical lines without duplication
- [ ] UTF‑8 boundary safety when consuming prefixes
- [ ] Add explicit tests for visual row accounting with small widths
- [ ] Add test for finalize‑time ```markdown unwrap path (skipped in cfg(test))

Testing – integration (tui/tests, vt100 based)
- [x] Add a public, test-only helper simulate_stream_markdown_for_tests that returns Lines for insert_history
- [ ] No commit until newline (long paragraph): verify no history insertions until newline/finalize
- [ ] Inline formatting correctness: bold/italic/link constructs render correctly without duplication
- [ ] Fenced code block streamed slowly: no duplication; only newline-complete rows commit; finalize flushes remainder
- [ ] Rendered streaming feel: rows trickle into history; live ring shows newest K rows
- [ ] Header emitted once when newline and overflow coincide
- [ ] Reasoning then answer ordering as expected
- [ ] Future: ignored test asserting ```markdown unwrap on finalize (not shown to the user)
- [x] No duplicates between committed history and live ring content in ANSI history stream

UX and semantics
- [x] History is append-only; previously committed lines never change
- [x] Trailing blank spacer line appended after each finalized stream
- [x] Reasoning and answer headers styled as before ("thinking" italic magenta, "codex" bold magenta)
- [ ] Increase streaming print speed (smooth but quick) so we never get overtaken by a tool call returning before we finish displaying any output

Performance and resilience
- [ ] Keep rendering cost acceptable for long streams (profile worst-case O(n^2); consider incremental strategies later)
- [ ] Ensure wide-character handling and wrapping are stable across platforms/terminals
- [ ] Validate behaviour with very small live_max_rows and very narrow widths

Documentation and cleanup
- [ ] Align naming in code and docs (MarkdownNewlineCollector vs MarkdownStreamAccumulator) or add a note
- [ ] Update inline comments to reflect newline-gated rendering once wired
- [ ] Remove dead code and unused fields after integration
- [ ] Add brief developer notes on how to simulate the streamer in tests

Live ring wrapping
- [ ] Live ring now wraps styled lines to viewport width while streaming
- [ ] Live ring desired_height accounts for wrapped visual rows (not logical lines)
- [ ] Live ring renders the newest K visual rows, preserving styles
- [ ] Add vt100 integration test to assert wrapped rows (including emoji/CJK) are shown correctly

Notes on the wrapping fix
- Implemented width-aware wrapping for rendered, styled Lines in tui/src/bottom_pane/live_ring_widget.rs.
- desired_height(width) now measures wrapped visual rows and caps to max_rows.
- Rendering pre-wraps lines and displays only the newest K visual rows, ensuring stability as characters trickle and as items are committed out of the ring.
- The wrapper preserves span styles and alignment, and uses Unicode cell widths so wide graphemes and combining marks are handled correctly.
