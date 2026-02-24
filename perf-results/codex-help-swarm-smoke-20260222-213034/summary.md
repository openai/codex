# Codex Local Perf Summary

- Generated: `2026-02-23T04:30:42.655919+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `5`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-h75395t5/codex-home-worker-1/config.toml`

## Profile

- Name: `swarm-cli-startup`
- Phase: `smoke`
- Concurrency: `6`
- Warmup: `2`
- Measured iterations: `5`

## Totals

- Successful runs: `30`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 860.871 | 731.393 | 1172.537 | 708.978 | 1239.190 |
| throughput_runs_per_sec | 7.2956 | 8.2035 | 8.4396 | n/a | n/a |
| max_rss_kb | 1983360.0 | 1983360.0 | 1996467.2 | 1966976.0 | 1999744.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 6 | 6 | 0 | 731.393 | 1983360 | 0 |
| 2 | 0 | 6 | 6 | 0 | 718.872 | 1999744 | 0 |
| 3 | 0 | 6 | 6 | 0 | 708.978 | 1966976 | 0 |
| 4 | 0 | 6 | 6 | 0 | 1239.190 | 1983360 | 0 |
| 5 | 0 | 6 | 6 | 0 | 905.925 | 1983360 | 0 |
