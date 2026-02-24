# Codex Local Perf Summary

- Generated: `2026-02-23T00:58:28.053196+00:00`
- Command: `sleep 0.2`
- Iterations: `3`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-2c96hbtx/codex-home-worker-1/config.toml`

## Profile

- Name: `queue-cancel-stress`
- Phase: `stress`
- Concurrency: `8`
- Warmup: `0`
- Measured iterations: `3`

## Totals

- Successful runs: `0`
- Failed runs: `24`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 77.446 | 75.220 | 82.489 | 73.823 | 83.297 |
| throughput_runs_per_sec | 0.0000 | 0.0000 | 0.0000 | n/a | n/a |
| max_rss_kb | n/a | n/a | n/a | n/a | n/a |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 8 | 0 | 8 | 83.297 | n/a | 0 |
| 2 | 1 | 8 | 0 | 8 | 73.823 | n/a | 0 |
| 3 | 1 | 8 | 0 | 8 | 75.220 | n/a | 0 |
