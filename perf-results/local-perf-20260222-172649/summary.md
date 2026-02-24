# Codex Local Perf Summary

- Generated: `2026-02-23T00:26:49.988458+00:00`
- Command: `/bin/echo ok`
- Iterations: `2`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-imgj3qlv/codex-home-worker-1/config.toml`

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
| latency_ms | 105.257 | 105.257 | 175.890 | 26.775 | 183.738 |
| throughput_runs_per_sec | 42.7903 | 42.7903 | 71.5051 | n/a | n/a |
| max_rss_kb | 885248.0 | 885248.0 | 885248.0 | 885248.0 | 885248.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 2 | 2 | 0 | 183.738 | 885248 | 0 |
| 2 | 0 | 2 | 2 | 0 | 26.775 | 885248 | 0 |
