# TUI2 viewport + history: architecture, change summary, and roadmap (DRAFT)

Status: **draft** (in progress)

This doc is meant to supersede (not delete) earlier design notes like:

- `tui2/docs/tui_viewport_and_history.md`
- `tui2/docs/scroll_input_model.md`
- `tui2/docs/streaming_wrapping_design.md`

It focuses on “what exists now”, “why we did it”, “what’s missing”, and how the
current modules fit together.

---

## What this doc is for (cleaned-up goals)

This document should make it fast to:

1. Understand the **before vs after** of the TUI2 viewport/history work (at the level of
   architecture and major implementation choices).
2. Get oriented in the codebase: **new modules**, **startup changes**, **`app.rs` changes**,
   **bottom pane widget changes**, and **history cell rendering changes**.
3. See the work broken down into the **PRs/commits that delivered it**, with links and short
   summaries.
4. Understand “done vs missing” and what remains for **completeness**.
5. Justify why this approach is better (correctness, UX, portability, maintainability, performance).

Non-goals:

- This is not intended to be a full tutorial for ratatui/crossterm.
- This is not intended to be a full protocol doc for codex-core/codex-protocol.

---

## How to read (progressive disclosure)

Skim-only:

- Start with **Executive summary**
- Then **PR index**
- Then **What’s missing / next steps**

Want the architecture:

- Read **Architecture overview** (data model → wrapping → caching → scroll/selection →
  render loop)

Want implementation detail:

- Jump to **Code map (entry points and modules)** and follow links.

Want a running log of findings:

- Use `tui2/docs/tui2_viewport_history_notes.md` (scratchpad + TODOs).

---

## Scope and prioritization (what’s in/out)

Baseline (“before”): **legacy TUI**.

Repository: `https://github.com/openai/codex`

In scope (high priority):

- PRs authored by **joshka-oai** or **joshka**.
- Changes under `tui2/**` (repo-root: `codex-rs/tui2/**`), especially viewport/history/transcript,
  selection/copy, and terminal behavior.

In scope (secondary, include when needed for a coherent story):

- Supporting changes outside `tui2/` required for the TUI2 viewport/history work to function
  (protocol/config/tooling glue).

Out of scope (or lowest priority):

- Generic TUI work by other authors that happens to touch `tui2/` as part of broader cleanup or
  “keep tui and tui2 in sync” changes, unless it materially affects viewport/history
  correctness.

---

## Methodology (how this doc is built and how to maintain it)

This section captures the constraints and goals that guided the work on these docs, plus the
concrete workflow used to gather evidence from the repo.

### Constraints (non-negotiables)

- This repository is a `jj` repo: investigate history with `jj`, not `git`.
- Always use `--no-pager` for commands whose output should be copy/paste-able.
- Prefer “read-only” inspection: do not change code while doing archaeology.
  - The only changes in this effort should be docs under `tui2/docs/`.
- Use `jj help <command>` or `jj <command> --help` when command usage is unclear.
- When reviewing doc edits, use `jj diff --git --no-pager` so diffs are easy to share.
- Keep docs markdownlint-friendly:
  - Wrap lines at ~100 columns.
  - Blank lines after headings, before lists, and around code blocks.

### Goals (what these docs must deliver)

- A “current state” doc for the TUI2 viewport/history approach:
  - What shipped and why.
  - What is missing and what should be done next.
- A skimmable top-down narrative with progressive disclosure:
  - Fast orientation (executive summary, PR table).
  - Deeper detail (architecture, implementation map, gaps, roadmap).
- A “before vs after” framing relative to the legacy TUI.
- An explicit PR index:
  - Table of PRs with dates and 1-sentence summaries.
  - Links to PRs, plus commit IDs where helpful.
- Clear justification:
  - Why transcript-owned viewport is worth it (correctness, portability, UX, performance).
- A compaction-safe workflow:
  - Keep the plan and the running notes doc updated so context survives long iterations.

### Broad-first workflow (how to investigate)

The workflow is intentionally “broad → narrow” so readers can confirm direction early:

1. Inventory PRs/commits touching `tui2/**` (repo-root: `codex-rs/tui2/**`), prioritizing
   Josh-authored PRs.
2. Summarize each PR at commit-message level into the running notes doc.
3. Extract the “themes” from those summaries (viewport, scroll, selection/copy, caching, etc.).
4. Only then do file-by-file deep dives for the high-impact modules.
5. Update the main doc sections as each theme becomes clear:
   timeline → architecture → gaps → roadmap → simplification ideas.

### Suggested `jj` commands (repeatable recipes)

List Josh-authored commits that touch `tui2/**` (repo-root: `codex-rs/tui2/**`):

```bash
jj --no-pager log -G -n 200 tui2 \
  -r '::main & (author(substring:"joshka") | committer(substring:"joshka"))' \
  -T 'committer.timestamp().local().format("%Y-%m-%d") ++ " " ++ commit_id.short() ++ " " ++ \
      description.first_line() ++ "\n"'
```

Focus on post-sync work by excluding the big baseline commit range:

```bash
jj --no-pager log -G tui2 \
  -r '90f262e9a46e..main & (author(substring:"joshka") | committer(substring:"joshka"))'
```

Inspect an individual PR merge commit and list touched files:

```bash
jj --no-pager show <commit_id_or_change_id> --name-only
```

Use `jj help -k revsets` to refine filters (by author, date, description, etc.).

---

## Plan for building/maintaining this doc (so compaction won’t lose context)

This is the plan the doc will follow (and keep updated as the work continues):

1. **Confirm scope**
   - Define the “viewport/history project boundary”: what changes count as part of this effort
     vs adjacent work.
2. **Collect PRs/commits**
   - Use `jj log --no-pager` filtered by relevant paths (`tui2/**`, repo-root:
     `codex-rs/tui2/**`) and any legacy tui references.
   - Extract PR numbers from commit subjects (e.g. `(... #1234)`), and capture dates.
   - Produce a PR table with: PR link, date, short summary, key files/modules, and “theme” tags
     (viewport/scroll/selection/copy/exit/suspend/etc).
   - Exclude the “massive tui → tui2 sync” commit from the per-commit summaries (but still
     reference it as the baseline for “after sync” work).
3. **Write the “before vs after” narrative**
   - Summarize what the earlier docs proposed vs what shipped.
   - Call out intentional deviations (tradeoffs taken to ship safely).
4. **Write the architecture overview**
   - Describe the actual pipeline and data flow:
     - history cells → transcript flattening → viewport wrapping → scroll model →
       selection/copy mapping → rendering/caching.
   - Add a single diagram (ASCII is fine) that shows major modules and data shapes.
5. **Write “what’s done” vs “what’s missing”**
   - Identify gaps or partial implementations (UX, correctness, edge cases, perf, tests).
   - Link each gap to a likely place in code and (when possible) the PR that introduced the
     current behavior.
6. **Add a roadmap**
   - Group remaining work into phases (must-have correctness, should-have UX, performance,
     cleanup, follow-ups).
7. **Maintenance rules**
   - The PR index must stay append-only (don’t rewrite history).
   - Prefer linking to PRs as the primary reference; commits are supporting links.
   - Keep “Executive summary” current even if deeper sections lag.

---

## Doc completeness checklist (living)

This checklist is about the documentation itself. For product completeness, see
**What’s missing / gaps**.

- [x] Capture scope, goals, and constraints
  - `jj`-only archaeology, docs-only changes, and markdownlint-ish formatting rules are recorded
    in **Methodology**.
- [x] PR table with PR + commit links
  - PRs are Josh-authored merges that touch `tui2/` on `main`.
  - Commit links point to the merge commit on `main` (GitHub `commit/<sha>`).
- [x] Skimmable executive summary and “how to read”
  - Includes a suggested reading order and a separate running notes doc for details.
- [x] Architecture overview + implementation map
  - Diagram + glossary + module-level map, with key files called out.
- [x] Gaps + roadmap + simplification ideas
  - Gaps are framed as Done/Partial/Missing; roadmap is grouped by priority.
- [x] Check for “secondary” Josh PRs outside `tui2/` (none found)
  - Revset search is recorded in `tui2/docs/tui2_viewport_history_notes.md`.
- [x] Deepen comparisons (optional)
  - [x] Legacy TUI: call out key modules/entry points for scrollback insertion
  - [x] TUI2 overlays vs inline viewport invariants (pager/backtrack)
- [x] Deepen component deep dives (optional)
  - [x] Bottom pane widget changes (composer/footer)
  - [x] Streaming markdown pipeline (`markdown_stream` / `markdown_render`)
  - [x] Frame timing/render helpers (`frames.rs`)

Evaluation:

- As of 2026-01-05, the doc meets the required methodology goals. The remaining unchecked items
  are explicitly tracked as follow-ups in **What’s missing / gaps** and **Roadmap**.
- Constraint check: only `tui2/docs/**` is modified in this working copy.

---

## Executive summary

TUI2’s viewport/history work replaces “cooperate with terminal scrollback” (legacy TUI) with
an app-owned transcript model.

In practice, this means the “scrollback” the user interacts with is rendered from in-memory
transcript state, not from terminal scrollback. The payoff is that scrolling, selection, and copy
become deterministic and consistent across terminals and resize/mode changes.

Current shape (high level, as shipped):

- Transcript-owned viewport: `HistoryCell` is the source of truth; each frame flattens cells into
  wrapped visual lines and renders a slice into the transcript region (no terminal scrollback
  coordination).
- Scroll model: scroll state is anchored to transcript metadata (`TranscriptScroll` +
  `TranscriptLineMeta`) so resizes and new output can reflow without “jumping”.
- Scroll input normalization: mouse wheel/trackpad scroll is normalized via a stream model
  (`tui2/src/tui/scrolling/mouse.rs`, [#8357]) with per-terminal defaults derived from
  `TerminalInfo` ([#8252]) and user overrides via config.
- Selection/copy: selection is content-relative (wrapped `line_index` + content `column`) and copy
  reconstructs text from the rendered transcript (soft-wrap joiners, code fences, inline backticks)
  via `transcript_copy` / `transcript_selection` / `transcript_copy_ui` / `transcript_copy_action`
  ([#8449], [#8462], [#8718]).
- Performance: redraw cadence is clamped and coalesced ([#8295], [#8499]); transcript rendering is
  cached via `TranscriptViewCache` (wrapped transcript cache + per-row raster cache) ([#8693]).
- Terminal integration: alt-screen transitions are made re-entrant and the first draw is forced to
  clear the viewport to avoid stale-cell artifacts ([#8463]); suspend/exit behavior is owned by the
  app rather than the terminal’s scrollback heuristics.

Remaining conservatism (intentional tradeoffs so far):

- Streaming wrapping/reflow has known constraints; some paths still trade perfect reflow for
  correctness and stability (see `tui2/docs/streaming_wrapping_design.md`).

Why this work was worth doing (justification):

The short version: we traded terminal-dependent behavior for app-owned behavior that we can test,
reason about, and evolve. The next sections expand this into concrete module boundaries and a PR
map you can follow.

- Cross-terminal correctness: stop relying on scroll regions/scrollback semantics that vary by
  terminal; render the transcript deterministically.
- A unified mental model: scroll/selection/copy are defined in transcript coordinates, not raw
  terminal rows.
- Testability and debuggability: the transcript can be rendered to ANSI deterministically for exit
  printing and for VT100-style tests.
- Performance control: the app can coalesce and cap redraws and cache expensive wrapping/raster
  work.
- Future extensibility: transcript-owned view state is the prerequisite for “interactive cells”
  and richer overlays.

Costs/tradeoffs:

- More complexity lives in-app (wrapping, caching, scroll anchors, selection/copy mapping).
- Some work remains conservative (notably streaming reflow) to avoid regressions while shipping.

---

## PR index

This table is the primary “map” for the project: if you only have time to click around, start
here. Later sections explain the architecture in a top-down way, but this table is the quickest
way to connect a concept (“copy pill UX”) to the code and the PR that introduced it.

Columns are intentionally short so it’s skimmable; details belong in later sections.

Initial seed list: PRs authored by joshka-oai/joshka where the merge commit touches
`tui2/**` (repo-root: `codex-rs/tui2/**`).

Commit links point at the merge commit on `main` (as shown by `jj log`). They are meant as stable
anchors; the PR links are the primary reference.

| PR      | Date       | Commit                | Summary (1 sentence)                       | Theme tags       |
| ------: | :--------- | :-------------------- | :----------------------------------------- | :--------------- |
| [#7793] | 2025-12-09 | [0c8828c5e298][c7793] | Add feature-flagged tui2 frontend          | bring-up         |
| [#7833] | 2025-12-10 | [90f262e9a46e][c7833] | Copy tui; normalize snapshots              | baseline         |
| [#7965] | 2025-12-12 | [6ec2831b91a3][c7965] | Sync tui2 with tui and keep dual-run glue  | sync             |
| [#7601] | 2025-12-15 | [b093565bfb5b][c7601] | WIP: viewport + selection/copy             | viewport         |
| [#8089] | 2025-12-15 | [f074e5706b1c][c8089] | Make transcript line meta explicit         | scroll           |
| [#8122] | 2025-12-16 | [3fbf379e02e0][c8122] | Docs: refine tui2 viewport roadmap         | docs             |
| [#8252] | 2025-12-18 | [df46ea48a230][c8252] | `TerminalInfo` for scroll scaling          | scroll           |
| [#8295] | 2025-12-19 | [1d4463ba8137][c8295] | Coalesce transcript scroll redraws         | scroll, perf     |
| [#8357] | 2025-12-20 | [63942b883c49][c8357] | Tune scrolling input model                 | scroll           |
| [#8423] | 2025-12-22 | [3c353a3acab9][c8423] | Re-enable ANSI for VT100 tests             | tests            |
| [#8419] | 2025-12-22 | [4e6d6cd7982d][c8419] | Constrain mouse selection bounds           | selection        |
| [#8449] | 2025-12-22 | [7d0c5c7bd5da][c8449] | Copy transcript selection outside viewport | copy             |
| [#8418] | 2025-12-22 | [f6275a51429c][c8418] | Include tracing targets in file logs       | logs             |
| [#8462] | 2025-12-22 | [414fbe0da95a][c8462] | Add copy shortcut + UI affordance          | copy, UX         |
| [#8463] | 2025-12-22 | [310f2114ae8f][c8463] | Fix screen corruption                      | render, terminal |
| [#8466] | 2025-12-22 | [282854932328][c8466] | Start transcript selection on drag         | selection        |
| [#8471] | 2025-12-23 | [0130a2fa405a][c8471] | Add multi-click transcript selection       | selection        |
| [#8499] | 2025-12-23 | [96a65ff0ed91][c8499] | Cap redraw scheduling to 60fps             | perf             |
| [#8681] | 2026-01-02 | [3cfa4bc8be78][c8681] | Reduce unnecessary redraws                 | perf             |
| [#8693] | 2026-01-03 | [90f37e854992][c8693] | Cache transcript view rendering            | perf, caching    |
| [#8697] | 2026-01-03 | [19525efb22ca][c8697] | Brighten transcript copy affordance        | UX               |
| [#8695] | 2026-01-03 | [279283fe02bf][c8695] | Fix scroll stickiness at cell boundaries   | scroll           |
| [#8716] | 2026-01-04 | [567821305831][c8716] | Render copy pill at viewport bottom        | copy, UX         |
| [#8718] | 2026-01-04 | [181ff89cbd33][c8718] | Copy selection dismisses highlight         | copy, UX         |

Open questions / TODOs for this section:

- Confirm whether we want to include “stacked PRs” and/or internal-only PRs.
- Add “secondary” PRs (outside `tui2/`) required for this work to function.

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

[c7601]: https://github.com/openai/codex/commit/b093565bfb5b2c016cb157127edb1ad62bfc7a27
[c7793]: https://github.com/openai/codex/commit/0c8828c5e298359ba50ba1e9c840400614afcd45
[c7833]: https://github.com/openai/codex/commit/90f262e9a46e592a58fe3e2cd6efc8717e448098
[c7965]: https://github.com/openai/codex/commit/6ec2831b91a35fe686945850e573ff810ea02b60
[c8089]: https://github.com/openai/codex/commit/f074e5706b1ce9158ca70d66df91760b46c1038e
[c8122]: https://github.com/openai/codex/commit/3fbf379e02e0f5701da3b01faa47712ba314b207
[c8252]: https://github.com/openai/codex/commit/df46ea48a2302ee677ce693ab588d7f41b01efc1
[c8295]: https://github.com/openai/codex/commit/1d4463ba8137b8ca3ea48ce08418ff2c4538d2c7
[c8357]: https://github.com/openai/codex/commit/63942b883c49849a7d6dfa5edcc6ae34014ffd78
[c8418]: https://github.com/openai/codex/commit/f6275a51429c2a072cdc90889f2c2b0712617abd
[c8419]: https://github.com/openai/codex/commit/4e6d6cd7982d2be3a0adb007585f93f06e23bc63
[c8423]: https://github.com/openai/codex/commit/3c353a3acab99f4cc3b7396fccc00e47384d7233
[c8449]: https://github.com/openai/codex/commit/7d0c5c7bd5da912b7a0bd826f4ebd41361391344
[c8462]: https://github.com/openai/codex/commit/414fbe0da95add9fc1075807edb44976f9304e64
[c8463]: https://github.com/openai/codex/commit/310f2114ae8fc519ba71e49c3e77e3f965f5c727
[c8466]: https://github.com/openai/codex/commit/2828549323284128ac1d2a862f866951aad41348
[c8471]: https://github.com/openai/codex/commit/0130a2fa405a73065e0f0d462b95fd0e09c51718
[c8499]: https://github.com/openai/codex/commit/96a65ff0ed918be41711ff031c1483b7068323c5
[c8681]: https://github.com/openai/codex/commit/3cfa4bc8be7893aff9a0304aa5212067c9aaa267
[c8693]: https://github.com/openai/codex/commit/90f37e854992c14d66147787f92b90f604246e42
[c8695]: https://github.com/openai/codex/commit/279283fe02bf0ce7f93a160db34dd8cf9c8f42c8
[c8697]: https://github.com/openai/codex/commit/19525efb22cabf968952cf542b7cac6738939efb
[c8716]: https://github.com/openai/codex/commit/567821305831fc506efdecb9c39cdc8ddbddd6fe
[c8718]: https://github.com/openai/codex/commit/181ff89cbd33ca5c571a7b7f210f42c827627c63

---

## Timeline (high level)

This is a narrative grouping, not a perfect chronological breakdown. Use it to understand the
sequence of “themes”:

- bring-up → scroll anchors → scroll normalization → selection/copy → perf

Then use the PR table above for exact ordering and links.

- Bring-up and baselining: feature-flagged `codex-tui2` frontend ([#7793]) and a large
  “sync/copy” baseline from legacy tui ([#7833], [#7965]).
- Viewport ownership rework: move toward transcript-as-source-of-truth, selection/copy, and
  suspend/exit printing ([#7601]).
- Make scroll anchors explicit: introduce `TranscriptLineMeta` and align scroll math around it
  ([#8089]), plus roadmap doc updates ([#8122]).
- Make scrolling consistent across terminals:
  - Terminal identification as structured metadata ([#8252]).
  - Stream-based scroll input normalization ([#8357]).
  - Redraw coalescing and 60fps clamping ([#8295], [#8499]).
  - Fix “stickiness” at cell boundaries by treating spacer rows as scroll anchors ([#8695]).
- Selection/copy correctness and UX:
  - Selection bounds + drag semantics ([#8419], [#8466]).
  - Copy beyond the viewport and rendered-text reconstruction ([#8449]).
  - Multi-click selection expansion (word/line/paragraph/cell) ([#8471]).
  - Copy pill UI, placement fixes, dismissal, and footer feedback ([#8462], [#8716], [#8718]).
- Performance and stability:
  - Reduce redraw loops under streaming and footer updates ([#8681]).
  - Cache transcript wrapping and rasterized rows ([#8693]).
  - Fix intermittent screen corruption via alt-screen nesting + first-draw clear ([#8463]).

---

## Change overview

This section describes what changed relative to the legacy TUI, organized by user-visible
behavior.

### Before vs after (viewport and history ownership)

Before (legacy TUI):

- The TUI attempted to “cooperate” with the terminal’s scrollback by inserting history above
  the viewport and relying on terminal-specific behavior for scroll regions, clears, resizes,
  and suspend.
- This led to terminal-dependent failure modes (dropped/duplicated lines, desynced scrollback,
  resize churn).

After (TUI2 viewport/history work):

- The in-memory transcript (history cells) is the single source of truth for what’s on screen.
- The app owns scrollback-like behavior by rendering a slice of wrapped transcript lines into a
  dedicated transcript region above the bottom pane.
- When the transcript is shorter than the available transcript region, the bottom pane is
  “pegged” just below the transcript and moves down as the transcript grows (matches legacy UX
  for early sessions).
- The terminal’s scrollback is treated as an output target for suspend/exit, not as an interactive
  “state store” the app tries to maintain.

### Transcript rendering and wrapping

Current pipeline (inline viewport):

- Logical transcript is a `Vec<Arc<dyn HistoryCell>>` (`tui2/src/history_cell.rs`).
- Each `HistoryCell` can render transcript lines with soft-wrap joiner metadata
  (`HistoryCell::transcript_lines_with_joiners`).
- `transcript_render` flattens cells into a single `TranscriptLines`:
  - `lines: Vec<Line<'static>>` (visual lines)
  - `meta: Vec<TranscriptLineMeta>` (maps lines back to `(cell_index, line_in_cell)` or `Spacer`)
  - `joiner_before: Vec<Option<String>>` (copy-only soft-wrap joiners)
- `transcript_view_cache` memoizes the wrapped transcript per width and can incrementally append
  new cells ([#8693]).

Important behavior:

- Viewport wrapping is applied to prose; preformatted content is intentionally not re-wrapped (so
  indentation is meaningful for copy/paste).
- Copy relies on joiners and styling-derived cues (e.g. cyan code blocks) to reconstruct Markdown
  source markers even if the UI renders styled spans instead of literal backticks.

### Scrolling

Scroll math (content anchors, not raw row indices):

- `TranscriptLineMeta` is the per-visual-line origin map (`CellLine` or `Spacer`) ([#8089]).
- `TranscriptScroll` represents:
  - `ToBottom` (follow latest)
  - `Scrolled { cell_index, line_in_cell }` (content anchor)
  - `ScrolledSpacerBeforeCell { cell_index }` (so 1-line scrolling can land on spacer rows)
    ([#8695])

Scroll input normalization (mouse wheel + trackpad):

- Scroll events are grouped into short “streams” and normalized using per-terminal
  `events_per_tick` plus mode heuristics ([#8357]).
- Defaults are keyed by detected terminal name (`TerminalInfo` / `TerminalName`) ([#8252]).
- Redraw scheduling is coalesced/clamped to avoid scroll lag during dense event bursts
  ([#8295], [#8499]).

### Selection and copy

Selection model (content-relative):

- Selection endpoints are `(line_index, column)` in the flattened wrapped transcript, with column
  measured in content space (excluding a fixed gutter) (`tui2/src/transcript_selection.rs`).
- Click sets an anchor; selection only becomes visible on drag (avoids “1-cell highlight” on
  click) ([#8466]).
- While dragging a selection during streaming, the selection can request a scroll lock so
  “follow latest output” does not move the viewport under the cursor.
- Selection is constrained to the transcript region (composer/footer interactions don’t start or
  mutate selection) ([#8419]).
- Multi-click selection expands selection in transcript coordinates:
  - double: word-ish token
  - triple: wrapped line
  - quad: paragraph
  - quint+: whole history cell
  (`tui2/src/transcript_multi_click.rs`, [#8471])

Copy behavior:

- Copy reconstructs text from the wrapped transcript lines (including off-screen selection)
  ([#8449]).
- Soft-wrap joiners are used to avoid inserting hard newlines for wrapped prose.
- Code blocks are detected (currently via styling) and copied using fenced Markdown with preserved
  indentation.

Copy UX:

- Copy shortcut is terminal-aware (VS Code fallback) and is centralized in `transcript_copy_ui`
  ([#8462]).
- A clickable “copy pill” is rendered near the visible end of the selection, with placement
  fixes at viewport boundaries ([#8716]).
- Copy action clears the selection and shows transient footer feedback
  (“Copied”/“Copy failed”) via `transcript_copy_action` ([#8718]).

### Rendering and performance

- `TranscriptViewCache` caches:
  - wrapped transcript lines per width (with incremental append)
  - rasterized per-row `Cell`s so redraws can copy pre-rendered rows instead of re-running grapheme
    segmentation ([#8693])
- Redraw requests are coalesced and capped at ~60fps ([#8295], [#8499]) and additional “feedback
  loops” are avoided ([#8681]).

### Terminal modes, overlays, suspend/exit

- `tui2/src/tui.rs` owns raw mode setup, bracketed paste, focus/mouse capture, and alt-screen
  transitions.
- Alt-screen nesting is guarded so nested overlay enter/leave doesn’t desync terminal state
  ([#8463]).
- Suspend/exit flows are app-owned; history printing is treated as an append-only operation rather
  than rewriting terminal scrollback (see [#7601] and `tui2/docs/tui_viewport_and_history.md`).

### Bottom pane (composer/footer integration)

- Transcript UI state (scrolled vs following, selection active, scroll position, copy shortcut,
  copy feedback) is computed during draw and passed into the bottom pane (`chat_widget`) so the
  footer can present correct hints and state.

---

## Architecture overview

This section describes the actual data flow and module boundaries.

### Mental model (module-level diagram)

```text
crossterm events
  ↓
`tui2/src/tui.rs` (Tui::event_stream)
  ↓
`tui2/src/app.rs` (App state + event handlers)
  - transcript_cells: Vec<Arc<dyn HistoryCell>>
  - transcript_scroll: TranscriptScroll
  - transcript_selection + transcript_multi_click
  - transcript_copy_ui + transcript_copy_action
  ↓ (Draw tick)
`App::render_transcript_cells`
  - TranscriptViewCache::ensure_wrapped(width)
  - TranscriptScroll::resolve_top(line_meta)
  - render visible rows (raster cache)
  - apply selection highlight + copy pill UI
  ↓
`tui2/src/bottom_pane/**` (composer/footer render using transcript UI state)
```

### Core data shapes (glossary)

These are the “terms of art” that show up repeatedly in the implementation:

- `HistoryCell` (logical unit of transcript)
  - Trait in `tui2/src/history_cell.rs`.
  - Renders its content into `Line`s, and can optionally supply soft-wrap joiners for copy via
    `transcript_lines_with_joiners`.
- Transcript “visual lines”
  - The flattened, wrapped lines that the viewport scrolls through.
  - Produced by `tui2/src/transcript_render.rs` and cached by
    `tui2/src/transcript_view_cache.rs`.
- `TranscriptLineMeta`
  - Per-visual-line origin mapping (`CellLine { cell_index, line_in_cell }` or `Spacer`).
  - Used for scroll anchoring and for determining row classification (e.g. user-row styling).
- `TranscriptScroll`
  - Scroll state as anchors, not absolute offsets (`ToBottom`, `Scrolled`,
    `ScrolledSpacerBeforeCell`).
  - Resolved each frame against current `line_meta` and viewport height.
- `TranscriptSelection` / `TranscriptSelectionPoint`
  - Selection endpoints in transcript coordinates (`line_index` + content `column`).
  - Columns exclude the gutter (`TRANSCRIPT_GUTTER_COLS`) so selection/copy operates on content.
- `TranscriptViewCache`
  - Two-layer cache: wrapped transcript lines + rasterized rows.
  - Used by the render hot path (reduces layout/grapheme work on redraw).
- `MouseScrollState` / `ScrollConfig`
  - Stream-based scroll normalization for mouse/trackpad.
  - Defaults derived from `TerminalInfo` plus user overrides.

### Code map (entry points and modules)

This list should become “the top-down reading order” once confirmed:

- Startup + terminal plumbing:
  - `tui2/src/main.rs`
  - `tui2/src/lib.rs` (bootstrapping/config)
  - `tui2/src/tui.rs` (modes, alt screen, event stream, suspend/exit)
- Main app loop and rendering:
  - `tui2/src/app.rs`
- Transcript pipeline:
  - `tui2/src/history_cell.rs`
  - `tui2/src/transcript_render.rs`
  - `tui2/src/transcript_view_cache.rs`
  - `tui2/src/transcript_selection.rs`
  - `tui2/src/transcript_copy.rs`
  - `tui2/src/transcript_copy_ui.rs`
  - `tui2/src/transcript_copy_action.rs`
  - `tui2/src/transcript_multi_click.rs`
- Scrolling:
  - `tui2/src/tui/scrolling/**`
  - `tui2/docs/scroll_input_model.md` (existing rationale)
- Bottom pane:
  - `tui2/src/bottom_pane/**`

Critical path modules (viewport/history work):

- `tui2/src/app.rs` (viewport layout + scroll/selection/copy handling)
- `tui2/src/history_cell.rs`, `tui2/src/transcript_render.rs`,
  `tui2/src/transcript_view_cache.rs` (transcript flatten/wrap/cache)
- `tui2/src/tui/scrolling/**` (scroll anchors + input normalization)
- `tui2/src/transcript_selection.rs`, `tui2/src/transcript_multi_click.rs`,
  `tui2/src/transcript_copy*.rs` (selection expansion + copy fidelity + UI)
- `tui2/src/chatwidget.rs` + `tui2/src/bottom_pane/**` (footer hints + copy feedback)

Adjacent (influences the viewport but not the core of this project):

- Streaming markdown pipeline: `tui2/src/markdown_stream.rs`, `tui2/src/markdown_render.rs`,
  `tui2/src/wrapping.rs`
- Overlays and UI modes: `tui2/src/pager_overlay.rs`, `tui2/src/tui/alt_screen_nesting.rs`
- Legacy scrollback insertion path (candidate cleanup): `tui2/src/insert_history.rs`

---

## Implementation map (what changed where)

This section is a code-oriented index of the work, grouped by responsibility. It’s intended to
help orient quickly before doing a deep read.

### Startup and frontend selection

- `tui2/src/main.rs`: `codex-tui2` binary entry point.
- `tui2/src/lib.rs`: TUI2 bootstrapping and top-level flow:
  - Initialize terminal + TUI wrapper (`tui2/src/tui.rs`).
  - Potentially run onboarding/update prompts in inline mode.
  - Enter alt screen for the main chat session, run `App::run`, then leave alt screen and print the
    exit transcript (`AppExitInfo.session_lines`).
- Origin: [#7793] bring-up, then large sync baselines ([#7833], [#7965]).

### App render loop and viewport composition

- `tui2/src/app.rs`: event loop + draw tick + viewport composition.
  - `App::handle_tui_event` triggers `handle_scroll_tick` on draw ticks and routes key/mouse input.
  - `App::render_transcript_cells`:
    - Defines the transcript region as “terminal minus composer height”.
    - Ensures the wrapped transcript cache is up to date (`TranscriptViewCache::ensure_wrapped`).
    - Resolves `TranscriptScroll` into a concrete `top_offset`.
    - Renders visible rows via `TranscriptViewCache::render_row_index_into`.
    - Applies selection highlight and renders the copy pill UI.
  - After drawing, transcript UI state is forwarded to the bottom pane
    (`chat_widget.set_transcript_ui_state`).

### Transcript rendering pipeline

- `tui2/src/history_cell.rs`: defines `HistoryCell` and concrete cell types.
  - `transcript_lines_with_joiners` is the key copy-fidelity hook.
- `tui2/src/transcript_render.rs`: flattening and viewport wrapping.
  - Produces `lines` + `line_meta` + `joiner_before`.
  - Handles spacer rows between non-continuation cells.
  - Produces ANSI exit transcript output via `render_lines_to_ansi`.
- `tui2/src/transcript_view_cache.rs`: caching layer for wrapped transcript + rasterized rows
  ([#8693]).

### Scrolling (state + input)

- `tui2/src/tui/scrolling.rs`: scroll anchoring and deltas in visual lines (`TranscriptScroll`).
- `tui2/src/tui/scrolling/mouse.rs`: stream-based wheel/trackpad normalization and per-terminal
  defaults ([#8357]).
- `core/src/terminal.rs`: terminal detection metadata (`TerminalInfo` / `TerminalName`) used for
  scroll defaults ([#8252]).

### Selection and copy

- `tui2/src/transcript_selection.rs`: selection model and drag semantics (anchor/head in transcript
  coordinates).
- `tui2/src/transcript_copy.rs`: reconstruct selection to clipboard text with joiners and
  markdown-ish markers (including off-screen selection) ([#8449]).
- `tui2/src/transcript_copy_ui.rs`: keybinding detection and the on-screen copy pill (terminal-aware
  shortcut, buffer-derived placement) ([#8462], [#8716]).
- `tui2/src/transcript_copy_action.rs`: side effects (clipboard write) + transient footer feedback
  ([#8718]).
- `tui2/src/transcript_multi_click.rs`: multi-click selection expansion (word/line/paragraph/cell)
  ([#8471]).

### Bottom pane widgets (footer integration)

- `tui2/src/chatwidget.rs`: forwards transcript UI state into the bottom pane.
- `tui2/src/bottom_pane/chat_composer.rs`: stores transcript UI state for footer rendering and
  avoids redraw feedback loops by only requesting redraws on changes ([#8681]).
- `tui2/src/bottom_pane/footer.rs`: renders scroll/selection/copy hints and copy feedback.

---

## Deep dives

This section fills in the “optional deep dives” from the methodology checklist. The goal is to
explain the non-obvious invariants and the “why” behind specific module boundaries, without
turning the whole doc into a line-by-line tour of the code.

### Bottom pane widgets (composer/footer integration)

The bottom pane isn’t just UI chrome; it is the other half of the viewport layout. The composer
height determines how many rows are available for the transcript region, and the footer is where
we surface transcript state (scrolled vs following, selection active, copy hints) without
polluting the transcript rendering pipeline.

The design constraint is subtle: the transcript draw computes the footer state, but the footer is
part of the overall layout, so it’s easy to accidentally create a redraw feedback loop. The
current solution keeps the dependency directional: the transcript computes a small “UI state”
struct each draw, and the bottom pane consumes it and requests redraw only when the state changes.

Data flow (render-time):

- `App::render_transcript_cells` computes the transcript region as “terminal minus composer
  height”, renders the transcript slice, and computes:
  - `scrolled` (are we away from bottom?)
  - `selection_active`
  - `scroll_position` (visible top / total, when meaningful)
  - `copy_selection_key` (terminal-aware key hint)
  - `copy_feedback` (Copied / Failed)
- That state is forwarded via:
  - `tui2/src/app.rs` → `tui2/src/chatwidget.rs` (`set_transcript_ui_state`) →
    `tui2/src/bottom_pane/mod.rs` → `tui2/src/bottom_pane/chat_composer.rs`
- The bottom pane requests redraw only if the values actually changed ([#8681]), which prevents a
  “footer update triggers redraw triggers footer update” loop.

Footer behavior:

- When the transcript is scrolled, the footer surfaces scroll/jump hints (PageUp/PageDown and
  Home/End) and can show `(current/total)` scroll position.
- When a selection is active, the footer shows the copy shortcut (from `transcript_copy_ui`) and
  appends copy feedback (from `transcript_copy_action`) after a copy attempt.

Primary PRs in this area:

- Footer/transcript hint integration: [#8462], [#8718]
- Redraw loop fixes via “only redraw on change”: [#8681]

### Streaming markdown pipeline (collector → controller → history cell)

Streaming is where the viewport/history model gets the most pressure: the agent emits arbitrary
chunks, Markdown semantics depend on surrounding context, and the UI still needs to feel alive
without “rewriting the past” every frame.

The current approach is intentionally conservative (and largely inherited from the legacy TUI):

- Deltas are accumulated in a newline-gated collector. We only “commit” fully-terminated logical
  lines, which avoids intermediate states where list markers/indentation arrive and re-shape the
  visible structure.
- Committed lines are queued and emitted via a controller that can animate commits one line at a
  time. This yields the “typing” feel without requiring the transcript viewport to repaint the
  entire growing Markdown buffer on every delta.
- Finalization forces any trailing partial line to render (by appending a temporary newline) so
  the last line is not lost when the agent stops streaming.

Where this lives:

- `tui2/src/chatwidget.rs`: owns `StreamController` lifecycle.
  - Seeds stream width from `last_rendered_width` (currently `width - 2` for indentation).
  - Sends `AppEvent::StartCommitAnimation` when new committed lines arrive.
- `tui2/src/streaming/controller.rs`: converts committed `Line`s into `AgentMessageCell`s and
  drains at most one queued line per commit tick.
- `tui2/src/markdown_stream.rs`: `MarkdownStreamCollector` renders the current buffer and returns
  only newly completed logical lines since the last commit.
- `tui2/src/markdown.rs` + `tui2/src/markdown_render.rs`: pulldown-cmark based renderer producing
  styled `Line`s. Some downstream behaviors (notably copy) still infer Markdown semantics from
  styling choices (e.g. cyan code spans).

Known limitations / future work:

- Resize-after-stream-start reflow is still imperfect
  (see `tui2/docs/streaming_wrapping_design.md`). Width is chosen when the stream starts, so some
  historical wrap decisions can persist.
- The animation/commit model is line-oriented rather than token-oriented; this keeps semantics
  stable but can feel “chunky” for highly structured Markdown.

### Frame scheduling and redraw control

The viewport/history approach assumes we can redraw often (streaming, mouse scrolling, selection
updates) without turning redraws into the bottleneck. The solution is to treat “draw” as a
scheduled event, not as a direct side effect of every input handler.

The key idea: any part of the TUI can request a redraw by cloning a `FrameRequester`, but the
implementation coalesces bursts and clamps emission to 60fps.

How it works:

- `tui2/src/tui/frame_requester.rs`: `FrameRequester` is a cloneable handle used across the UI.
  Calls to `schedule_frame()` and `schedule_frame_in(...)` enqueue a desired draw deadline.
- A `FrameScheduler` task coalesces many requests into one draw notification and uses
  `tui2/src/tui/frame_rate_limiter.rs` (`MIN_FRAME_INTERVAL`) to clamp to ~60fps ([#8499]).
- `tui2/src/tui.rs` turns those notifications into `TuiEvent::Draw` via a broadcast channel. The
  app event loop treats draw ticks like any other input event (key/mouse/resize).
- `Tui::draw` renders inside `stdout().sync_update(...)` and includes a first-draw clear to avoid
  stale terminal content leaking through diff-based rendering ([#8463]).

How this relates to viewport/history work:

- Terminals can emit very dense scroll/mouse streams; coalescing + fps clamping prevents the
  viewport from falling behind on input while still feeling responsive ([#8295], [#8499]).
- Several “redraw feedback loop” fixes (footer updates, status indicators, streaming) rely on
  being able to request redraws freely while keeping actual draws bounded ([#8681]).

### Overlays vs inline viewport (pager + backtrack)

TUI2 has two “ways of looking” at the transcript:

- The inline viewport: always present in the main chat UI, shares the screen with the bottom pane,
  and supports transcript scrolling + selection + copy.
- Pager overlays: modal views (full-screen within the TUI area) used for “show transcript” and
  backtrack preview/selection.

These paths intentionally don’t share all the same machinery. Overlays optimize for simplicity
and predictable navigation; the inline viewport is where we invested in anchor-based scrolling,
mouse normalization, and selection/copy correctness.

Key differences to keep in mind:

- Scroll model:
  - Inline viewport uses `TranscriptScroll` anchored to `TranscriptLineMeta` so resizes and new
    output don’t “jump” ([#8089], [#8695]).
  - Pager overlay uses a simple `scroll_offset` in rendered rows and a fixed wheel step of 3 rows
    per scroll event (`tui2/src/pager_overlay.rs`).
- Rendering pipeline:
  - Inline viewport uses `transcript_render` + `TranscriptViewCache` (wrapping + raster caching)
    and then applies selection/copy UI in the hot path.
  - Pager overlay re-renders cells into a list of `Renderable` chunks and scrolls through their
    desired heights.
- Alt-screen behavior:
  - Overlays call `tui.enter_alt_screen()`/`leave_alt_screen()` even when already in alt screen.
    This relies on alt-screen nesting guards to avoid terminal state corruption ([#8463]).

Backtrack preview is layered on top of the transcript overlay:

- `tui2/src/app_backtrack.rs` manages `BacktrackState` and routes Esc/Enter in overlay mode.
- In overlay preview mode, Esc steps through older user messages by setting a highlight cell in
  `TranscriptOverlay`, and Enter confirms (forking from that point).

Follow-up opportunity:

- Pager overlay scrolling does not currently use the same scroll normalization model as the inline
  viewport; unifying them would reduce surprise but may not be necessary for correctness.

### Legacy TUI scrollback insertion (what we moved away from)

The legacy TUI tried to “cooperate with terminal scrollback”: keep an inline viewport somewhere
above the bottom of the terminal, and insert new transcript lines *above* that viewport so they
became part of the terminal’s normal scrollback.

In code, the core mechanism is “scroll region insertion”:

- `tui/src/insert_history.rs` (and its copied counterpart `tui2/src/insert_history.rs`) sets a
  scroll region, uses Reverse Index (`ESC M`) to shift the lower region down, and then prints the
  wrapped transcript lines into the region above the viewport.
- `tui/src/tui.rs` maintains a notion of `viewport_area` and uses cursor-position heuristics on
  resize to keep the viewport aligned with the terminal state.

Why this was brittle:

- Terminals vary in how they implement scroll regions, clears, and resize semantics.
- The approach has many “moving parts” that can desynchronize under resize, overlays, and
  suspend/resume, causing dropped/duplicated history.

What we kept (and why it still matters):

- The ANSI writer path in `insert_history.rs` (`write_spans`) is still valuable for deterministic
  output: `transcript_render::render_lines_to_ansi` uses it for exit transcript printing and
  VT100-style tests. Even if we eventually delete inline scrollback insertion, the “render to
  ANSI” part is a useful, testable primitive.

---

## What’s missing / gaps

This section is the “completeness checklist” for the viewport/history effort.

Status legend:

- Done: shipped and reasonably stable
- Partial: shipped but known limitations remain
- Missing: not implemented yet

### At-a-glance checklist (living)

- [x] Transcript-owned viewport rendering ([#7601])
- [x] Anchor-based scroll state + spacer anchors ([#8089], [#8695])
- [x] Stream-based scroll input normalization ([#8357]) + per-terminal defaults ([#8252])
- [x] Selection bounds + drag semantics ([#8419], [#8466]) + multi-click expansion ([#8471])
- [x] Off-screen copy reconstruction ([#8449]) + copy shortcut/pill UX ([#8462], [#8716], [#8718])
- [x] Redraw coalescing/caps + transcript view caching ([#8295], [#8499], [#8693], [#8681])
- [ ] Suspend printing to scrollback (missing)
- [ ] Drag selection auto-scroll near viewport edges (missing)
- [ ] Streaming reflow polish + resize-after-streaming tests (partial)
- [ ] Cleanup: remove or wire unused history insertion paths (partial)

### Viewport + scroll correctness

This is the “can I trust what I’m seeing?” bucket. The main correctness bar is that the same
transcript content should render exactly once, in order, across resizes, streaming growth, and
mode transitions (inline viewport vs overlays).

- Done: transcript-owned viewport (flattened visual lines rendered into a transcript region)
  ([#7601]).
- Done: anchor-based scroll state using `TranscriptLineMeta` ([#8089]) with spacer anchors to avoid
  boundary “stickiness” ([#8695]).
- Partial: invariants across overlays vs inline viewport (pager/backtrack overlay behavior vs main
  transcript viewport) need to be documented and tested as a unified system.
- Partial: backtracking/forking and transcript replacement are handled in several places (e.g.
  cache rebuild rules in `tui2/src/transcript_view_cache.rs`), but we don’t yet have a single
  documented set of invariants.

### Selection/copy UX and correctness

This bucket covers two related concerns: mapping mouse gestures onto transcript coordinates, and
reconstructing text for copy/paste in a way that matches the logical content rather than the
wrapped pixels.

- Done: drag selection anchored to transcript coordinates, not terminal rows ([#8466]).
- Done: selection bounds ignore mouse events outside the transcript region ([#8419]).
- Done: off-screen selection copy reconstructs text from wrapped transcript lines ([#8449]).
- Done: multi-click selection expansion (word/line/paragraph/cell) ([#8471]).
- Partial: wide-glyph behavior (emoji/CJK) is handled in some copy paths, but needs broader
  cross-terminal validation and explicit invariants.
- Missing: auto-scroll while dragging selection near the viewport edges (called out in
  `tui2/docs/tui_viewport_and_history.md`).
- Partial: selection/copy boundaries across multi-step output (whether step boundaries are copy
  boundaries) are not yet explicitly defined.

### Streaming + wrapping

Streaming is intentionally treated as “partially solved”: the baseline behavior is stable and
matches the legacy TUI, but perfect resize/reflow semantics are not yet guaranteed mid-stream.

- Partial: streaming wrapping/reflow is still conservative (see
  `tui2/docs/streaming_wrapping_design.md`).
- Partial: tests that cover “resize after streaming has started” for all streaming paths.

### Suspend/exit printing

The contract here is about what ends up in the user’s normal scrollback outside the TUI. Exit
printing is the “must-have” path.
Suspend printing is still an open design/implementation choice.

- Done: exit transcript is rendered to ANSI and printed after leaving alt screen
  (`tui2/src/app.rs` → `AppExitInfo.session_lines`).
- Missing: “print transcript on suspend” is not implemented yet (see
  `tui2/docs/tui_viewport_and_history.md` roadmap).
- Partial: audit/document “exactly once” semantics for any scrollback printing paths.
  (Today, most “history printing” behavior is tied to the exit transcript output.)

### Performance and scaling

This bucket tracks “does it stay fast as sessions get long?” concerns.
The work so far focuses on bounding redraw rate and caching expensive rendering work.
Long-session memory/eviction is still a design space.

- Done: scroll redraw coalescing + 60fps cap ([#8295], [#8499]).
- Done: transcript view caching (wrapped transcript + rasterized rows) ([#8693]).
- Partial: copy and multi-click selection can rebuild the wrapped transcript view (`O(total
  transcript text)`), which may be acceptable but should be explicitly treated as a performance
  tradeoff.
- Partial: long-session memory/caching strategy (history capping, cache eviction tuning) is not
  documented as a “designed” story yet.

### Cleanup / simplification

Some parts of the codebase still reflect the legacy “insert into scrollback” architecture. The
goal of this bucket is to remove dead paths and tighten module boundaries so the current
architecture is easier to understand and maintain.

- Partial: `Tui::insert_history_lines` / `pending_history_lines` look unused in the current
  alt-screen default flow; either wire them up for inline mode or remove them to reduce confusion.

---

## Roadmap

This is the working roadmap for making the viewport/history work feel “complete”.

Canonical source for the original prioritized roadmap is:

- `tui2/docs/tui_viewport_and_history.md` section “10.2 Roadmap (prioritized)”

This document’s roadmap is intended to stay aligned, but phrased in terms of “what’s left
given what’s already shipped”.

### P0 (must-have)

- Keep scroll behavior “native-feeling” across terminals and avoid event-loop backlog (mostly
  shipped via [#8357], [#8295], [#8499], but should be continuously validated).
- Preserve copy fidelity for wrapped prose and preformatted blocks (shipped; maintain invariants as
  styling evolves).

### P1 (should-have)

- Streaming wrapping polish + resize-after-streaming tests (see
  `tui2/docs/streaming_wrapping_design.md`).
- Auto-scroll during drag selection near viewport edges.
- Define and document selection/copy boundaries for multi-step output.
- Cross-terminal behavior checks for selection/copy and “terminal override selection” modes.

### P2 (nice-to-have)

- Decide whether suspend printing is desirable; if yes, implement it and document config/behavior.
- Move toward “interactive cells” unlocked by transcript ownership (cell-scoped copy actions,
  drill-down overlays, expand/collapse rendered regions).
- Reduce complexity and duplication between legacy tui and tui2 code paths where feasible.

---

## Appendix: related docs and terminology

Related / older design docs:

- `tui2/docs/tui_viewport_and_history.md` (early viewport/history design notes)
- `tui2/docs/scroll_input_model.md` (mouse/trackpad scroll model and reasoning)
- `tui2/docs/streaming_wrapping_design.md` (streaming wrapping constraints and direction)

Working notes:

- `tui2/docs/tui2_viewport_history_notes.md`

---

## Appendix: simplification ideas

This section captures “complexity reduction” ideas discovered during the investigation.

### Consolidate transcript view sources

- `transcript_copy` and `transcript_multi_click` rebuild a wrapped transcript view from cells.
  Consider reusing `TranscriptViewCache` as the single “current wrapped view” source where
  possible (to reduce duplicate wrapping logic and `O(total transcript)` rebuilds).

### Clarify scrollback vs ANSI writing responsibilities

- `insert_history.rs` contains both “insert above viewport using scroll regions” and
  “write spans to ANSI for deterministic output”.
  Consider splitting into:
  - an ANSI writer module (pure, reusable for exit transcript and tests)
  - a scrollback insertion module (only used for inline viewport mode, if kept)

### Reduce cross-module coupling

- `TranscriptLineMeta` lives under `tui/scrolling`, but it is also a transcript-rendering concern.
  Consider moving it to a transcript-centric module to make dependencies clearer.

### Shrink `App` surface area

- `App` owns a lot of responsibilities (input handling, viewport state, selection, copy UX,
  overlay flows, rendering). The recent extraction of `TranscriptCopyAction` is a good pattern;
  consider continuing this by introducing a dedicated “transcript viewport controller” that
  owns:
  scroll state, selection state, multi-click tracking, copy UI state, and render helpers.

### Remove or wire up dead paths

- `Tui::pending_history_lines` / `Tui::insert_history_lines` currently appear unused in the
  alt-screen-default flow; either implement the drain/flush path or remove them to avoid confusion.

### Reduce style-coupled logic

- Copy currently infers Markdown code fences and inline code from render-time styling
  (e.g. cyan spans/lines). Consider exposing semantic markers from markdown rendering (or history
  cells) so copy logic doesn’t depend on a particular color choice.
