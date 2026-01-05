# TUI2 viewport + history: running notes (DRAFT)

This file is intentionally a scratchpad. It exists so we can iterate, capture raw findings,
and survive compaction without losing breadcrumbs.

If you’re new, start with `tui2/docs/tui2_viewport_history_architecture.md`.

---

## Work log (what changed in these docs)

- 2026-01-05: Created `tui2_viewport_history_architecture.md` + this notes doc, and added
  “historical” pointers to the older viewport/scroll/streaming design docs, and indexed
  Josh-authored PRs that
  touch `tui2/` (with links + a first-pass architecture/gaps/roadmap).
- 2026-01-05: Added commit links to the PR table, added a doc completeness checklist +
  justification section, clarified the “critical path” modules, and verified there are no
  extra Josh “tui2” PRs outside `tui2/` (revset search).
- 2026-01-05: Expanded the embedded TODO/checklist sections (doc + implementation), and added an
  at-a-glance completeness checklist under “What’s missing / gaps”.
- 2026-01-05: Filled the optional deep dives in the main doc (bottom pane integration, streaming
  markdown pipeline, frame scheduling/redraw control, overlays/backtrack, and legacy scrollback
  insertion).

---

## Working agreement for this doc

- Prefer short bullets over essays.
- Keep command lines and key outputs (trimmed) so later readers can reproduce.
- When a finding seems “important”, migrate it into the main doc and leave a link here.
- Keep TODOs grouped and checked off as they’re resolved.

---

## Scope (decisions so far)

- Repo: `https://github.com/openai/codex`
- Baseline (“before”): legacy TUI
- Priority: `tui2/**` changes (repo-root: `codex-rs/tui2/**`)
- Secondary: supporting changes outside `tui2/` required for the work
- Focus: PRs authored by joshka-oai / joshka
- Keep older docs as-is, but link them to the new doc

Note on paths:

- `jj show --name-only` prints repo-root-relative paths (often `codex-rs/...`).
- When navigating from within `codex-rs/`, drop the `codex-rs/` prefix (e.g. `tui2/src/app.rs`).

Open questions:

- Are there specific “must-include” PRs you want to seed the table with?
- Should we treat “stacked PRs” specially in the PR index?

---

## TODO (high level)

- [x] Confirm scope boundaries (what counts as viewport/history work)
  - Repo: `https://github.com/openai/codex`
  - Baseline: legacy TUI
- [x] Build PR index table from `jj log` (paths + PR numbers + dates)
  - Captured Josh-authored PRs touching `tui2/` on `main`
  - Added PR + merge-commit links in the main doc
- [x] Draft “before vs after” framing (high level)
  - Captured the “why app-owned transcript” story and major behavior changes
- [x] Deep legacy TUI compare (optional)
  - Added a “legacy scrollback insertion” deep dive in the main doc.
- [x] Map architecture and data flow with a diagram
  - Added diagram + data-shape glossary in the main doc
- [x] Identify missing pieces / gaps (correctness, UX, perf, tests)
  - Recorded confirmed gaps (suspend printing, drag auto-scroll, streaming reflow/tests, cleanup)
- [x] Draft roadmap with phases and owners (if applicable)
  - Grouped into P0/P1/P2 for completeness work
- [x] Add “historical” pointers to earlier design docs
- [x] Optional deep dives (streaming, frames, overlays)
  - Added a dedicated “Deep dives” section in the main doc.

---

## Investigation checklist (where to look)

### Entry points / startup

- [x] `tui2/src/main.rs` (binary entry)
- [x] `tui2/src/lib.rs` (bootstrap/config)
- [x] `tui2/src/tui.rs` (terminal modes, alt screen, suspend/exit)

### App render loop + viewport

- [x] `tui2/src/app.rs` (render loop and layout)
- [x] `tui2/src/frames.rs` (frame timing/render helpers)
- [x] `tui2/src/tui/scrolling/**` (scroll state, mouse model, anchors)

### Transcript pipeline

- [x] `tui2/src/history_cell.rs` (cell types and transcript lines)
- [x] `tui2/src/transcript_render.rs` (flatten/wrap/meta)
- [x] `tui2/src/transcript_view_cache.rs` (wrapped + raster cache)
- [x] `tui2/src/transcript_selection.rs` (selection model)
- [x] `tui2/src/transcript_copy.rs` / `tui2/src/transcript_copy_ui.rs` (copy behavior)

### Bottom pane and widgets

- [x] Identify composer/footer integration points + key PRs
- [ ] Deep dive: composer, popups, approvals, footer (optional)

### Streaming and wrapping

- [x] `tui2/docs/streaming_wrapping_design.md` (design constraints)
- [x] `tui2/src/markdown_stream.rs` / `tui2/src/markdown_render.rs`

---

## Commands and outputs (fill as we go)

### PR discovery

Planned commands (examples; adapt as needed):

```bash
jj --no-pager log tui2 -r '::main'
jj --no-pager log tui2 -r '::main & (author(joshka) | committer(joshka))'
jj --no-pager log -r '::main & (author(joshka) | committer(joshka))' -n 200
```

Notes:

- Prefer `jj log <paths...>` to restrict to viewport/history work.
- Capture PR numbers from subjects like `(... #1234)` and convert to links once the base URL is
  known.

### Secondary PR search (Josh; outside `tui2/`)

This checks whether there are Josh-authored commits that mention “tui2” but do not touch
`tui2/`.

```bash
jj --no-pager log -n 20 \
  -r '((::main) & (author(substring:"joshka") | committer(substring:"joshka")) & \
      description(substring:"tui2")) ~ files(tui2)' \
  -T 'committer.timestamp().local().format("%Y-%m-%d") ++ " " ++ commit_id.short() ++ " " ++ \
      description.first_line() ++ "\n"'
```

Result: no matches.

---

## Findings (raw; to migrate into main doc)

## Broad commit inventory (Joshka; touches `tui2/**`)

Command:

```bash
jj --no-pager log -G -n 60 tui2 \
  -r '::main & (author(substring:"joshka") | committer(substring:"joshka"))' \
  -T 'committer.timestamp().local().format("%Y-%m-%d") ++ " " ++ commit_id.short() ++ " " ++ \
      description.first_line() ++ "\n"'
```

Raw list (to refine into the PR index and architecture narrative):

- 2026-01-04 `181ff89cbd33` [#8718] copy selection dismisses highlight
- 2026-01-04 `567821305831` [#8716] render copy pill at viewport bottom
- 2026-01-03 `279283fe02bf` [#8695] avoid scroll stickiness at cell boundaries
- 2026-01-03 `19525efb22ca` [#8697] brighten transcript copy affordance
- 2026-01-03 `90f37e854992` [#8693] cache transcript view rendering
- 2026-01-02 `3cfa4bc8be78` [#8681] reduce unnecessary redraws
- 2025-12-23 `96a65ff0ed91` [#8499] cap redraw scheduling to 60fps
- 2025-12-23 `0130a2fa405a` [#8471] add multi-click transcript selection
- 2025-12-22 `282854932328` [#8466] start transcript selection on drag
- 2025-12-22 `310f2114ae8f` [#8463] fix screen corruption
- 2025-12-22 `414fbe0da95a` [#8462] add copy shortcut + UI affordance
- 2025-12-22 `f6275a51429c` [#8418] include tracing targets in file logs
- 2025-12-22 `7d0c5c7bd5da` [#8449] copy transcript selection outside viewport
- 2025-12-22 `4e6d6cd7982d` [#8419] constrain transcript mouse selection bounds
- 2025-12-22 `3c353a3acab9` [#8423] re-enable ANSI for VT100 tests
- 2025-12-20 `63942b883c49` [#8357] tune scrolling input (commit subject truncated)
- 2025-12-19 `1d4463ba8137` [#8295] coalesce transcript scroll redraws
- 2025-12-18 `df46ea48a230` [#8252] terminal detection metadata for scroll scaling
- 2025-12-16 `3fbf379e02e0` [#8122] docs: refine tui2 viewport roadmap
- 2025-12-15 `f074e5706b1c` [#8089] make transcript line metadata explicit
- 2025-12-15 `b093565bfb5b` [#7601] WIP: rework viewport, history printing, selection/copy
- 2025-12-12 `6ec2831b91a3` [#7965] sync tui2 with tui and keep dual-run glue
- 2025-12-10 `90f262e9a46e` [#7833] copy tui crate and normalize snapshots (massive sync)
- 2025-12-09 `0c8828c5e298` [#7793] add feature-flagged tui2 frontend

Baseline note:

- [#7833] is the “massive sync” baseline:
  - `jj --no-pager diff --stat -r 90f262e9a46e` reports `742 files changed` and ~`53k` insertions.

## PR notes (broad; key files touched)

These are quick “what changed where” notes, based on `jj show --name-only`.

- [#7793] bring-up (feature-flagged tui2 frontend)
  - Touches: `codex-rs/tui2/src/lib.rs`, `codex-rs/tui2/src/main.rs`,
    `codex-rs/core/src/features.rs`, `codex-rs/cli/src/main.rs`, `docs/config.md`.

- [#7965] sync tui2 with tui + dual-run glue
  - Touches many files under `codex-rs/tui2/src/**` plus `codex-rs/tui2/tests/**`.
  - Appears to keep some `codex-tui` interop conversions in `codex-rs/tui2/src/lib.rs`.

- [#7601] WIP: rework viewport/history printing/selection-copy
  - Touches: `codex-rs/tui2/src/app.rs`, `codex-rs/tui2/src/tui.rs`,
    `codex-rs/tui2/src/tui/job_control.rs`, `codex-rs/tui2/src/insert_history.rs`,
    `codex-rs/tui2/src/clipboard_copy.rs`, `codex-rs/tui2/src/bottom_pane/footer.rs`,
    `codex-rs/tui2/src/chatwidget.rs`, `codex-rs/tui2/src/pager_overlay.rs`.
  - Adds/updates docs: `tui2/docs/tui_viewport_and_history.md`,
    `tui2/docs/streaming_wrapping_design.md`.

- [#8089] transcript line metadata refactor
  - Touches: `codex-rs/tui2/src/tui/scrolling.rs`, `codex-rs/tui2/src/app.rs`,
    `codex-rs/tui2/src/tui.rs`.

- [#8693] transcript view caching (wrapped transcript + row raster cache)
  - Touches: `codex-rs/tui2/src/transcript_view_cache.rs`,
    `codex-rs/tui2/src/transcript_render.rs`, `codex-rs/tui2/src/app.rs`.
  - Also touches `codex-rs/tui2/src/terminal_palette.rs` and `codex-rs/tui/src/terminal_palette.rs`.
  - Adds: `docs/tui2/performance-testing.md`.

- [#8122] docs: refine tui2 viewport roadmap
  - Touches: `codex-rs/tui2/docs/tui_viewport_and_history.md`.

- [#8252] terminal detection metadata (per-terminal scroll scaling)
  - Touches: `codex-rs/core/src/terminal.rs`, `codex-rs/tui2/src/lib.rs`.

- [#8295] coalesce transcript scroll redraws
  - Touches: `codex-rs/tui2/src/app.rs`.

- [#8357] scroll input model: stream-based wheel/trackpad normalization
  - Touches: `codex-rs/tui2/src/tui/scrolling/mouse.rs`,
    `codex-rs/tui2/src/tui/scrolling.rs`, `codex-rs/tui2/src/app.rs`,
    `codex-rs/tui2/docs/scroll_input_model.md`.
  - Also touches config/docs: `codex-rs/core/src/config/types.rs`, `docs/config.md`.

- [#8423] VT100 tests: force ANSI on under NO_COLOR
  - Touches: `codex-rs/tui2/src/test_backend.rs`.

- [#8419] selection bounds: ignore mouse outside transcript region
  - Touches: `codex-rs/tui2/src/app.rs`.

- [#8449] copy selection outside viewport (full logical selection range)
  - Touches: `codex-rs/tui2/src/transcript_selection.rs`,
    `codex-rs/tui2/src/app.rs`, `codex-rs/tui2/src/lib.rs`.

- [#8418] include tracing targets in file logs
  - Touches: `codex-rs/tui/src/lib.rs`, `codex-rs/tui2/src/lib.rs`.

- [#8462] copy shortcut + “copy pill” UI affordance
  - Touches: `codex-rs/tui2/src/transcript_copy.rs`, `codex-rs/tui2/src/app.rs`,
    `codex-rs/tui2/src/bottom_pane/footer.rs`, `codex-rs/tui2/src/key_hint.rs`.

- [#8463] fix screen corruption (alt-screen nesting + first-draw clear)
  - Touches: `codex-rs/tui2/src/tui/alt_screen_nesting.rs`, `codex-rs/tui2/src/tui.rs`.

- [#8466] start transcript selection on drag
  - Touches: `codex-rs/tui2/src/transcript_selection.rs`, `codex-rs/tui2/src/app.rs`.

- [#8471] multi-click transcript selection (word/line/paragraph/cell)
  - Touches: `codex-rs/tui2/src/transcript_multi_click.rs`,
    `codex-rs/tui2/src/transcript_selection.rs`, `codex-rs/tui2/src/transcript_render.rs`.

- [#8499] cap redraw scheduling to 60fps
  - Touches: `codex-rs/tui2/src/tui/frame_requester.rs`,
    `codex-rs/tui2/src/tui/frame_rate_limiter.rs`, `codex-rs/tui2/src/tui.rs`.

- [#8681] reduce unnecessary redraws
  - Touches: `codex-rs/tui2/src/chatwidget.rs`, `codex-rs/tui2/src/bottom_pane/chat_composer.rs`.

- [#8697] brighten transcript copy affordance
  - Touches: `codex-rs/tui2/src/transcript_copy_ui.rs`.

- [#8695] scroll anchoring: make spacer rows first-class
  - Touches: `codex-rs/tui2/src/tui/scrolling.rs`, `codex-rs/tui2/src/app.rs`.

- [#8716] render copy pill at viewport bottom (edge case)
  - Touches: `codex-rs/tui2/src/transcript_copy_ui.rs`.

- [#8718] copy action clears highlight + shows footer feedback
  - Touches: `codex-rs/tui2/src/transcript_copy_action.rs`, `codex-rs/tui2/src/app.rs`,
    `codex-rs/tui2/src/bottom_pane/footer.rs`.

TODO:

- [x] Verify the list above includes all relevant PRs authored by joshka-oai/joshka.
- [x] Treat [#7833] as the pre-work baseline in the narrative and PR index.
- [ ] Add any “secondary” Josh PRs outside `tui2/` if needed for coherence.

## Legacy TUI precursor PRs (Josh; may be relevant background)

Found via:

```bash
jj --no-pager log -G -n 200 \
  -r '::main & (author(substring:"joshka") | committer(substring:"joshka")) & \
      (description(substring:"tui2") | description(substring:"tui"))' \
  -T 'committer.timestamp().local().format("%Y-%m-%d") ++ " " ++ commit_id.short() ++ " " ++ \
      description.first_line() ++ "\n"'
```

- 2025-12-08 `a9f566af7bfb` [#7660] restore status header after stream recovery
- 2025-12-02 `58e1e570faf0` [#7461] tui.rs extract several pieces
- 2025-11-21 `3ea33a061650` [#6382] fail when stdin is not a terminal
- 2025-11-10 `60deb6773a35` [#6477] job-control for Ctrl-Z handling
- 2025-11-07 `9fba811764c7` [#6373] cleanup deprecated flush logic
- 2025-10-27 `66a4b8982268` [#5568] clarify Windows auto mode requirements
- 2025-10-23 `e258f0f0441c` [#5582] use Option symbol for mac key hints
- 2025-10-15 `18d00e36b9b8` [#5035] warn high effort rate use

[#5035]: https://github.com/openai/codex/pull/5035
[#5568]: https://github.com/openai/codex/pull/5568
[#5582]: https://github.com/openai/codex/pull/5582
[#6373]: https://github.com/openai/codex/pull/6373
[#6382]: https://github.com/openai/codex/pull/6382
[#6477]: https://github.com/openai/codex/pull/6477
[#7461]: https://github.com/openai/codex/pull/7461
[#7660]: https://github.com/openai/codex/pull/7660

---

## Gaps / follow-ups (confirmed so far)

- Suspend printing:
  - `tui2/src/app.rs` prints an ANSI transcript on exit via `AppExitInfo.session_lines`.
  - `tui2/src/tui/job_control.rs` handles Ctrl-Z by leaving alt screen and restoring modes, but
    does not print transcript/history to scrollback.
- Drag selection auto-scroll (near viewport edges) does not appear to be implemented yet; it is
  still listed as P1 in `tui2/docs/tui_viewport_and_history.md`.
- `tui2/src/tui.rs` has `pending_history_lines` and `Tui::insert_history_lines`, but they appear
  unused in the current alt-screen-default flow (no drain/flush path found).
- Streaming wrapping/reflow remains conservative per `tui2/docs/streaming_wrapping_design.md`.
- Some UX actions (copy and multi-click expansion) rebuild the wrapped transcript view; treat as a
  known `O(total transcript text)` tradeoff unless optimized later.

[#7601]: https://github.com/openai/codex/pull/7601
[#7793]: https://github.com/openai/codex/pull/7793
[#7833]: https://github.com/openai/codex/pull/7833
[#7965]: https://github.com/openai/codex/pull/7965
[#8089]: https://github.com/openai/codex/pull/8089
[#8122]: https://github.com/openai/codex/pull/8122
[#8252]: https://github.com/openai/codex/pull/8252
[#8295]: https://github.com/openai/codex/pull/8295
[#8357]: https://github.com/openai/codex/pull/8357
[#8418]: https://github.com/openai/codex/pull/8418
[#8419]: https://github.com/openai/codex/pull/8419
[#8423]: https://github.com/openai/codex/pull/8423
[#8449]: https://github.com/openai/codex/pull/8449
[#8462]: https://github.com/openai/codex/pull/8462
[#8463]: https://github.com/openai/codex/pull/8463
[#8466]: https://github.com/openai/codex/pull/8466
[#8471]: https://github.com/openai/codex/pull/8471
[#8499]: https://github.com/openai/codex/pull/8499
[#8681]: https://github.com/openai/codex/pull/8681
[#8693]: https://github.com/openai/codex/pull/8693
[#8695]: https://github.com/openai/codex/pull/8695
[#8697]: https://github.com/openai/codex/pull/8697
[#8716]: https://github.com/openai/codex/pull/8716
[#8718]: https://github.com/openai/codex/pull/8718
