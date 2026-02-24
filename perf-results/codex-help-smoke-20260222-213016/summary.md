# Codex Local Perf Summary

- Generated: `2026-02-23T04:30:23.547070+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `5`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-vgc4e4eb/codex-home/config.toml`

## Profile

- Name: `cli-startup`
- Phase: `smoke`
- Concurrency: `1`
- Warmup: `2`
- Measured iterations: `5`

## Totals

- Successful runs: `5`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 522.295 | 542.509 | 739.352 | 281.985 | 763.804 |
| throughput_runs_per_sec | 2.1756 | 1.8433 | 3.3611 | n/a | n/a |
| max_rss_kb | 1960422.4 | 1966976.0 | 1993190.4 | 1934208.0 | 1999744.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 542.509 | 1966976 | 0 |
| 2 | 0 | 1 | 1 | 0 | 381.629 | 1966976 | 0 |
| 3 | 0 | 1 | 1 | 0 | 281.985 | 1934208 | 0 |
| 4 | 0 | 1 | 1 | 0 | 641.545 | 1934208 | 0 |
| 5 | 0 | 1 | 1 | 0 | 763.804 | 1999744 | 0 |
