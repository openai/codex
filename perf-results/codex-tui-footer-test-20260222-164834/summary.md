# Codex Local Perf Summary

- Generated: `2026-02-22T23:48:50.943438+00:00`
- Command: `cargo test --manifest-path codex-rs/Cargo.toml -p codex-tui bottom_pane::chat_composer::tests::footer_collapse_snapshots -- --nocapture`
- Iterations: `3`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-v2bhq5sk/codex-home/config.toml`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 2861.973 | 3020.511 | 3107.544 | 2448.195 | 3117.215 |
| throughput_runs_per_sec | 0.3534 | 0.3311 | 0.4007 | n/a | n/a |
| max_rss_kb | 170690112.0 | 170739264.0 | 170783443.2 | 170542720.0 | 170788352.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|
| 1 | 0 | 3117.215 | 170788352 | 0 |
| 2 | 0 | 3020.511 | 170542720 | 0 |
| 3 | 0 | 2448.195 | 170739264 | 0 |
