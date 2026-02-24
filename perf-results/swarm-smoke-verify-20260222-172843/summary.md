# Codex Local Perf Summary

- Generated: `2026-02-23T00:28:45.012375+00:00`
- Command: `/bin/echo ok`
- Iterations: `2`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-1fwzedo3/codex-home-worker-1/config.toml`

## Profile

- Name: `swarm-smoke`
- Phase: `measure`
- Concurrency: `2`
- Warmup: `0`
- Measured iterations: `2`

## Totals

- Successful runs: `4`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 74.521 | 74.521 | 92.441 | 54.610 | 94.432 |
| throughput_runs_per_sec | 28.9013 | 28.9013 | 35.8511 | n/a | n/a |
| max_rss_kb | 885248.0 | 885248.0 | 885248.0 | 885248.0 | 885248.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 2 | 2 | 0 | 94.432 | 885248 | 0 |
| 2 | 0 | 2 | 2 | 0 | 54.610 | 885248 | 0 |
