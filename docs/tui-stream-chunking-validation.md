# TUI Stream Chunking Validation Process

This document records the process used to validate adaptive stream chunking
and anti-flap behavior.

## Scope

The goal is to verify two properties from runtime traces:

- display lag is reduced when queue pressure rises
- mode transitions remain stable instead of rapidly flapping

## Trace targets

Chunking observability is emitted by:

- `codex_tui::streaming::commit_tick`

Two trace messages are used:

- `stream chunking commit tick`
- `stream chunking mode transition`

## Runtime command

Run Codex with chunking traces enabled:

```bash
RUST_LOG='codex_tui::streaming::commit_tick=trace,codex_tui=info,codex_core=info,codex_rmcp_client=info' \
  just codex --enable=responses_websockets
```

## Log capture process

Tip: for one-off measurements, run with `-c log_dir=...` to direct logs to a fresh directory and avoid mixing sessions.

1. Record the current size of `~/.codex/log/codex-tui.log` as a start offset.
2. Run an interactive prompt that produces sustained streamed output.
3. Stop the run.
4. Parse only log bytes written after the recorded offset.

This avoids mixing earlier sessions with the current measurement window.

## Metrics reviewed

For each measured window:

- `commit_ticks`
- `mode_transitions`
- `smooth_ticks`
- `catchup_ticks`
- drain-plan distribution (`Single`, `Batch(n)`)
- queue depth (`max`, `p95`, `p99`)
- oldest queued age (`max`, `p95`, `p99`)
- rapid re-entry count:
  - number of `Smooth -> CatchUp` transitions within 1 second of a
    `CatchUp -> Smooth` transition

## Interpretation

- Healthy behavior:
  - queue age remains bounded while backlog is drained
  - transition count is low relative to total ticks
  - rapid re-entry events are infrequent and localized to burst boundaries
- Regressed behavior:
  - repeated short-interval mode toggles across an extended window
  - persistent queue-age growth while in smooth mode
  - long catch-up runs without backlog reduction

## Experiment history

This section captures the major tuning passes so future work can build on
what has already been tried.

- Baseline
  - One-line smooth draining with a 50ms commit tick.
  - This preserved familiar pacing but could feel laggy under sustained
    backlog.
- Pass 1: instant catch-up, baseline tick unchanged
  - Kept smooth-mode semantics but made catch-up drain the full queued
    backlog each catch-up tick.
  - Result: queue lag dropped faster, but perceived motion could still feel
    stepped because smooth-mode cadence remained coarse.
- Pass 2: faster baseline tick (25ms)
  - Improved smooth-mode cadence and reduced visible stepping.
  - Result: better, but still not aligned with draw cadence.
- Pass 3: frame-aligned baseline tick (~16.7ms)
  - Set baseline commit cadence to approximately 60fps.
  - Result: smoother perceived progression while retaining hysteresis and
    fast backlog convergence.
- Pass 4: higher frame-aligned baseline tick (~8.3ms)
  - Set baseline commit cadence to approximately 120fps.
  - Result: further reduced smooth-mode stepping while preserving the same
    adaptive catch-up policy shape.

Current state combines:

- instant catch-up draining in `CatchUp`
- hysteresis for mode-entry/exit stability
- frame-aligned smooth-mode commit cadence (~8.3ms)

## Notes

- Validation is source-agnostic and does not rely on naming any specific
  upstream provider.
- This process intentionally preserves existing baseline smooth behavior and
  focuses on burst/backlog handling behavior.
