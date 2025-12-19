# TUI2 Scroll Feel Update (Goal-Focused)

## TL;DR

- The north-star is to make TUI2 scrolling feel native across terminals as we move toward a
  go/no-go decision on the TUI2 rollout.
- The highest-risk roadmap item right now is **P0: Scrolling behavior** in
  `codex-rs/tui2/docs/tui_viewport_and_history.md` (section 10.2). That work is **in progress**.
- We have a cadence-based model with per-terminal defaults, traces for measurement, and tests, but
  we are not yet fully satisfied with cross-terminal trackpad feel (notably iTerm2 and WezTerm).

## Where We Are Relative to the Roadmap

From the roadmap in `codex-rs/tui2/docs/tui_viewport_and_history.md`:

- **P0: Scrolling behavior (must-have)** — **In progress**
  - We now treat single events as 1-line trackpad scrolls, detect bursts with a rolling window,
    and apply per-terminal overrides where measurements show consistent patterns.
  - The goal is “native” feel (no sticky single events, no splashy over-acceleration).
  - Still tuning thresholds and per-terminal behavior based on real measurements.
- **Other P0 items (mouse bounds, copy offscreen, copy fidelity)** — **Not started / unchanged**

## Goal and Rationale (High Level)

TUI2’s core promise is that Codex owns the viewport and scrollback behavior instead of relying on
terminal-specific quirks. That only works if scrolling feels natural on real hardware and common
terminals. The work here is about translating real mouse and trackpad event timing into stable,
predictable line deltas so users can navigate long transcripts without friction.

## What We Tried and What We Learned

- A generalized velocity/acceleration approach didn’t yield good results; terminal quirks dominate.
- We built a small diagnostic tool to profile event timing for mouse wheel and trackpad across
  terminals. The data showed large differences in burst cadence and batching.
- The practical path is per-terminal defaults + cadence heuristics, with the ability to tune as we
  learn more.

Key terminal observations (summarized near the tuning code):
- Ghostty: fast bursts (many events in milliseconds) → needs burst division.
- VS Code: wheel is clamped to frame cadence → needs frame-scale boost.
- iTerm2: single events for wheel/trackpad; trackpad cadence clusters near 16–17ms → needs careful
  inference to avoid fast trackpad scrolling.
- WezTerm: wheel/trackpad cadence overlaps → likely needs a config override or upstream option.

## Current State

We now:

- Use a short rolling window (3 intervals) to classify bursts so a single event never suppresses
  scrolling.
- Keep single events at 1 line (trackpad-like) unless we observe a short sequence that indicates
  wheel behavior.
- Apply measured per-terminal overrides where data is strong.
- Emit trace logs to support repeatable testing and normalization.

Even with those changes, iTerm2 trackpad still feels too fast in some cases, so the heuristic is
being refined to require a short sequence before applying 3-line wheel steps.

## Next Steps to Reach the Goal

- Run a consistent manual scroll script with tracing enabled across terminals.
- Use the trace logs to adjust cadence thresholds and per-terminal defaults.
- Decide whether WezTerm needs a config knob / upstream option for reliable trackpad vs wheel
  differentiation.
- Keep tui and tui2 in sync until the go/no-go decision is made (per team direction).

## Summary

We are making progress toward the P0 scrolling goal, but we are not “done” yet. The model is now
measurable, explainable, and testable, which should let us converge faster. The remaining work is
mostly empirical validation and calibration so the default behavior feels native across terminals.
