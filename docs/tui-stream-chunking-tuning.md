# TUI Stream Chunking Tuning Guide

This document explains how to tune adaptive stream chunking constants without
changing the underlying policy shape.

## Scope

Use this guide when adjusting queue-pressure thresholds and hysteresis windows in
`codex-rs/tui/src/streaming/chunking.rs`, and baseline commit cadence in
`codex-rs/tui/src/app.rs`.

This guide is about tuning behavior, not redesigning the policy.

## Before tuning

- Keep the baseline behavior intact:
  - `Smooth` mode drains one line per baseline tick.
  - `CatchUp` mode drains queued backlog immediately.
- Capture trace logs with:
  - `codex_tui::streaming::commit_tick`
- Evaluate on sustained, bursty, and mixed-output prompts.

See `docs/tui-stream-chunking-validation.md` for the measurement process.

## Tuning goals

Tune for all three goals together:

- low visible lag under bursty output
- low mode flapping (`Smooth <-> CatchUp` chatter)
- stable catch-up entry/exit behavior under mixed workloads

## Constants and what they control

### Baseline commit cadence

- `COMMIT_ANIMATION_TICK` (`tui/src/app.rs`)
  - Lower values increase smooth-mode update cadence and reduce steady-state lag.
  - Higher values increase smoothing and can increase perceived lag.
  - This should usually move after chunking thresholds/holds are in a good range.

### Enter/exit thresholds

- `ENTER_QUEUE_DEPTH_LINES`, `ENTER_OLDEST_AGE`
  - Lower values enter catch-up earlier (less lag, more mode switching risk).
  - Higher values enter later (more lag tolerance, fewer mode switches).
- `EXIT_QUEUE_DEPTH_LINES`, `EXIT_OLDEST_AGE`
  - Lower values keep catch-up active longer.
  - Higher values allow earlier exit and may increase re-entry churn.

### Hysteresis holds

- `EXIT_HOLD`
  - Longer hold reduces flip-flop exits when pressure is noisy.
  - Too long can keep catch-up active after pressure has cleared.
- `REENTER_CATCH_UP_HOLD`
  - Longer hold suppresses rapid re-entry after exit.
  - Too long can delay needed catch-up for near-term bursts.
  - Severe backlog bypasses this hold by design.

### Severe-backlog gates

- `SEVERE_QUEUE_DEPTH_LINES`, `SEVERE_OLDEST_AGE`
  - Lower values bypass re-entry hold earlier.
  - Higher values reserve hold bypass for only extreme pressure.

## Recommended tuning order

Tune in this order to keep cause/effect clear:

1. Entry/exit thresholds (`ENTER_*`, `EXIT_*`)
2. Hold windows (`EXIT_HOLD`, `REENTER_CATCH_UP_HOLD`)
3. Severe gates (`SEVERE_*`)
4. Baseline cadence (`COMMIT_ANIMATION_TICK`)

Change one logical group at a time and re-measure before the next group.

## Symptom-driven adjustments

- Too much lag before catch-up starts:
  - lower `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
- Frequent `Smooth -> CatchUp -> Smooth` chatter:
  - increase `EXIT_HOLD`
  - increase `REENTER_CATCH_UP_HOLD`
  - tighten exit thresholds (lower `EXIT_*`)
- Catch-up engages too often for short bursts:
  - increase `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
  - increase `REENTER_CATCH_UP_HOLD`
- Catch-up engages too late:
  - lower `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
  - lower severe gates (`SEVERE_*`) to bypass re-entry hold sooner

## Validation checklist after each tuning pass

- `cargo test -p codex-tui` passes.
- Trace window shows bounded queue-age behavior.
- Mode transitions are not concentrated in repeated short-interval cycles.
- Catch-up clears backlog quickly once mode enters `CatchUp`.
