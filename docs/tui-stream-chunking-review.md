# TUI Stream Chunking

This document explains how stream chunking in the TUI works and why it is
implemented this way.

## Problem

Streaming output can arrive faster than a one-line-per-tick animation can show
it. If commit speed stays fixed while arrival speed spikes, queued lines grow
and visible output lags behind received output.

## Design goals

- Preserve existing baseline behavior under normal load.
- Reduce display lag when backlog builds.
- Keep output order stable.
- Avoid abrupt single-frame flushes that look jumpy.
- Keep policy transport-agnostic and based only on queue state.

## Non-goals

- The policy does not schedule animation ticks.
- The policy does not depend on upstream source identity.
- The policy does not reorder queued output.

## Where the logic lives

- `codex-rs/tui/src/streaming/chunking.rs`
  - Adaptive policy, mode transitions, and drain-plan selection.
- `codex-rs/tui/src/streaming/commit_tick.rs`
  - Orchestration for each commit tick: snapshot, decide, drain, trace.
- `codex-rs/tui/src/streaming/controller.rs`
  - Queue/drain primitives used by commit-tick orchestration.
- `codex-rs/tui/src/chatwidget.rs`
  - Integration point that invokes commit-tick orchestration and handles UI
    lifecycle events.

## Runtime flow

On each commit tick:

1. Build a queue snapshot across active controllers.
   - `queued_lines`: total queued lines.
   - `oldest_age`: max age of the oldest queued line across controllers.
2. Ask adaptive policy for a decision.
   - Output: current mode and a drain plan.
3. Apply drain plan to each controller.
4. Emit drained `HistoryCell`s for insertion by the caller.
5. Emit trace logs for observability.

In `CatchUpOnly` scope, policy state still advances, but draining is skipped
unless mode is currently `CatchUp`.

## Modes and transitions

Two modes are used:

- `Smooth`
  - Baseline behavior: one line drained per baseline commit tick.
  - Baseline tick interval currently comes from
    `tui/src/app.rs:COMMIT_ANIMATION_TICK` (~8.3ms, ~120fps).
- `CatchUp`
  - Drain current queued backlog per tick via `Batch(queued_lines)`.

Entry and exit use hysteresis:

- Enter `CatchUp` when queue depth or queue age exceeds enter thresholds.
- Exit requires both depth and age to be below exit thresholds for a hold
  window (`EXIT_HOLD`).

This prevents oscillation when load hovers near thresholds.

## Current experimental tuning values

These are the current values in `streaming/chunking.rs` plus the baseline
commit tick in `tui/src/app.rs`. They are
experimental and may change as we gather more trace data.

- Baseline commit tick: `~8.3ms` (`COMMIT_ANIMATION_TICK` in `app.rs`)
- Enter catch-up:
  - `queued_lines >= 8` OR `oldest_age >= 120ms`
- Exit catch-up eligibility:
  - `queued_lines <= 2` AND `oldest_age <= 40ms`
- Exit hold (`CatchUp -> Smooth`): `250ms`
- Re-entry hold after catch-up exit: `250ms`
- Severe backlog thresholds:
  - `queued_lines >= 64` OR `oldest_age >= 300ms`

## Drain planning

In `Smooth`, plan is always `Single`.

In `CatchUp`, plan is `Batch(queued_lines)`, which drains the currently queued
backlog for immediate convergence.

## Why this design

This keeps normal animation semantics intact, while making backlog behavior
adaptive:

- Under normal load, behavior stays familiar and stable.
- Under pressure, queue age is reduced quickly without sacrificing ordering.
- Hysteresis avoids rapid mode flapping.

## Invariants

- Queue order is preserved.
- Empty queue resets policy back to `Smooth`.
- `CatchUp` exits only after sustained low pressure.
- Catch-up drains are immediate while in `CatchUp`.

## Observability

Trace events are emitted from commit-tick orchestration:

- `stream chunking commit tick`
  - `mode`, `queued_lines`, `oldest_queued_age_ms`, `drain_plan`,
    `has_controller`, `all_idle`
- `stream chunking mode transition`
  - `prior_mode`, `new_mode`, `queued_lines`, `oldest_queued_age_ms`,
    `entered_catch_up`

These events are intended to explain display lag by showing queue pressure,
selected drain behavior, and mode transitions over time.
