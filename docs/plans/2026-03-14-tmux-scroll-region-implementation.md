# Tmux Scroll Region Handling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Avoid tmux pane boundary artifacts by disabling scroll-region ANSI sequences during history insertion, while keeping visible output consistent and disabling delayed frames in tmux.

**Architecture:** Introduce a scroll-region mode for history insertion. Tui selects the tmux-safe mode when a tmux multiplexer is detected. Tests assert scroll-region sequences are present only in the default path.

**Tech Stack:** Rust, crossterm, ratatui, codex-tui unit tests.

---

### Task 1: Add scroll-region mode + tests (TDD)

**Files:**
- Modify: `codex-rs/tui/src/insert_history.rs`
- Modify: `codex-rs/tui/src/test_backend.rs` (test-only backend)

**Step 1: Write the failing test**

Add a test backend that records ANSI output and a helper to detect `ESC[...r`.
Write a test that calls `insert_history_lines_with_mode(..., Disabled)` and
asserts no scroll-region sequences are emitted.

**Step 2: Run test to verify it fails**

Run: `cargo test -p codex-tui insert_history::tests::tmux_mode_avoids_scroll_region`  
Expected: FAIL (missing symbol or expected behavior not implemented).

**Step 3: Write minimal implementation**

Introduce:

```rust
pub enum ScrollRegionMode { Enabled, Disabled }
pub fn insert_history_lines_with_mode(..., mode: ScrollRegionMode) -> io::Result<()>
```

In `Disabled` mode:
- Clear the viewport area.
- Append history lines at the bottom (no scroll-region calls).
- Re-anchor viewport for redraw and record inserted rows.

**Step 4: Run test to verify it passes**

Run: `cargo test -p codex-tui insert_history::tests::tmux_mode_avoids_scroll_region`  
Expected: PASS.

**Step 5: Commit**

```bash
git add codex-rs/tui/src/insert_history.rs codex-rs/tui/src/test_backend.rs
git commit -m "tui: add tmux-safe history insertion without scroll regions"
```

---

### Task 2: Wire tmux detection to tmux-safe history insertion

**Files:**
- Modify: `codex-rs/tui/src/tui.rs`

**Step 1: Write the failing test**

Add a unit test for a small helper (e.g., `history_scroll_mode_for_terminal`)
that returns `Disabled` when `TerminalInfo.multiplexer` is tmux.

**Step 2: Run test to verify it fails**

Run: `cargo test -p codex-tui tui::tests::tmux_disables_scroll_region_mode`  
Expected: FAIL (helper not implemented).

**Step 3: Write minimal implementation**

Add a helper to compute scroll-region mode and use it in `Tui::insert_history_lines`.

**Step 4: Run test to verify it passes**

Run: `cargo test -p codex-tui tui::tests::tmux_disables_scroll_region_mode`  
Expected: PASS.

**Step 5: Commit**

```bash
git add codex-rs/tui/src/tui.rs
git commit -m "tui: disable scroll regions for history in tmux"
```

---

### Task 3: Update Termux safe build guidance

**Files:**
- Modify: `scripts/termux/build-safe.sh`
- Modify: `AGENTS.md`

**Step 1: Write the failing test**

No automated test; update scripts/docs directly.

**Step 2: Implement**

Ensure build-safe.sh clearly reuses shared safe env values and explicitly
documents `CARGO_BUILD_JOBS=1` + `CARGO_BUILD_PIPELINING=false`.
Add AGENTS.md text about serialized rustc and avoiding rustc+linker overlap.

**Step 3: Commit**

```bash
git add scripts/termux/build-safe.sh AGENTS.md
git commit -m "docs: clarify Termux safe build constraints"
```

---

### Task 4: Format and verify

**Step 1: just fmt**

Run: `cd codex-rs && just fmt`

**Step 2: Safety tests**

Run: `cd codex-rs && cargo test -p codex-tui`  
Note: If toolchain ICE occurs, capture logs and stop.

**Step 3: Safe debug build**

Run: `scripts/termux/build-safe.sh --debug`

**Step 4: Bash run verification**

Run:

```bash
~/A137442/codex-exomind/codex-rs/target/aarch64-linux-android/debug/codex --version
```

