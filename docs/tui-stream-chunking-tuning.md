# TUI Stream Chunking Tuning Guide

This document explains how to tune adaptive stream chunking constants without
changing the underlying policy shape.

## Scope

Use this guide when adjusting thresholds or timing constants in
`codex-rs/tui/src/streaming/chunking.rs`.

This guide is about tuning behavior, not redesigning the policy.

## Before tuning

- Keep the baseline behavior intact:
  - `Smooth` mode drains one line per baseline tick.
  - `CatchUp` mode drains bounded batches.
- Capture trace logs with:
  - `codex_tui::streaming::commit_tick`
- Evaluate on sustained, bursty, and mixed-output prompts.

See `docs/tui-stream-chunking-validation.md` for the measurement process.

## Tuning goals

Tune for all three goals together:

- low visible lag under bursty output
- low mode flapping (`Smooth <-> CatchUp` chatter)
- smooth perceived motion during catch-up (not single-frame bursts)

## Constants and what they control

### Baseline cadence

- `BASELINE_COMMIT_TICK`
  - Controls smooth-mode drain cadence and tick quantization for paced
    catch-up.
  - Lower values increase visual update frequency and CPU/wakeups.
  - Higher values reduce update frequency and can increase visible lag.

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

### Catch-up pacing

- `CATCH_UP_TARGET`, `SEVERE_CATCH_UP_TARGET`
  - Lower target duration drains faster (less lag, choppier risk).
  - Higher target duration drains slower (smoother, more lag risk).
- `CATCH_UP_MIN_BATCH_LINES`
  - Raises minimum work per catch-up tick.
  - If too high, catch-up can look jumpy for small queues.
- `CATCH_UP_MAX_BATCH_LINES`
  - Caps worst-case per-tick burst size.
  - If too low, backlog may persist too long under heavy bursts.

### Severe-backlog gates

- `SEVERE_QUEUE_DEPTH_LINES`, `SEVERE_OLDEST_AGE`
  - Lower values engage severe pacing earlier.
  - Higher values reserve severe mode for only extreme pressure.

## Recommended tuning order

Tune in this order to keep cause/effect clear:

1. Entry/exit thresholds (`ENTER_*`, `EXIT_*`)
2. Hold windows (`EXIT_HOLD`, `REENTER_CATCH_UP_HOLD`)
3. Target durations (`CATCH_UP_TARGET`, `SEVERE_CATCH_UP_TARGET`)
4. Batch bounds (`CATCH_UP_MIN_BATCH_LINES`, `CATCH_UP_MAX_BATCH_LINES`)
5. Severe gates (`SEVERE_*`)

Change one logical group at a time and re-measure before the next group.

## Symptom-driven adjustments

- Too much lag before catch-up starts:
  - lower `ENTER_QUEUE_DEPTH_LINES` and/or `ENTER_OLDEST_AGE`
- Frequent `Smooth -> CatchUp -> Smooth` chatter:
  - increase `EXIT_HOLD`
  - increase `REENTER_CATCH_UP_HOLD`
  - tighten exit thresholds (lower `EXIT_*`)
- Catch-up feels too bursty:
  - increase `CATCH_UP_TARGET`
  - decrease `CATCH_UP_MIN_BATCH_LINES`
  - decrease `CATCH_UP_MAX_BATCH_LINES`
- Catch-up clears backlog too slowly:
  - decrease `CATCH_UP_TARGET`
  - increase `CATCH_UP_MAX_BATCH_LINES`
  - lower severe gates (`SEVERE_*`) to enter severe pacing sooner

## Validation checklist after each tuning pass

- `cargo test -p codex-tui` passes.
- Trace window shows bounded queue-age behavior.
- Mode transitions are not concentrated in repeated short-interval cycles.
- Catch-up drains backlogs without large one-frame jumps.
