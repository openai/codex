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

## Notes

- Validation is source-agnostic and does not rely on naming any specific
  upstream provider.
- This process intentionally preserves existing baseline smooth behavior and
  focuses on burst/backlog handling behavior.
