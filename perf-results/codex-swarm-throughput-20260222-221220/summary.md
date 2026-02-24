# Codex Local Perf Summary

- Generated: `2026-02-23T05:12:26.433015+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `10`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-wesozsb4/codex-home-worker-1/config.toml`

## Profile

- Name: `swarm-throughput`
- Phase: `measure`
- Concurrency: `6`
- Warmup: `2`
- Measured iterations: `10`

## Totals

- Successful runs: `60`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 251.831 | 234.619 | 336.292 | 206.897 | 373.427 |
| throughput_runs_per_sec | 24.5695 | 25.5750 | 28.7579 | n/a | n/a |
| max_rss_kb | 1983360.0 | 1983360.0 | 1999744.0 | 1966976.0 | 1999744.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| system_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 3.0 | 3.0 | 5.5 | n/a | n/a |
| involuntary_ctx_switches | 277.4 | 267.5 | 336.7 | n/a | n/a |
| peak_open_fds | n/a | n/a | n/a | n/a | n/a |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 6 | 6 | 0 | 265.975 | 1966976 | 0 |
| 2 | 0 | 6 | 6 | 0 | 210.807 | 1966976 | 0 |
| 3 | 0 | 6 | 6 | 0 | 268.178 | 1966976 | 0 |
| 4 | 0 | 6 | 6 | 0 | 212.706 | 1999744 | 0 |
| 5 | 0 | 6 | 6 | 0 | 236.495 | 1983360 | 0 |
| 6 | 0 | 6 | 6 | 0 | 206.897 | 1983360 | 0 |
| 7 | 0 | 6 | 6 | 0 | 290.904 | 1983360 | 0 |
| 8 | 0 | 6 | 6 | 0 | 232.742 | 1999744 | 0 |
| 9 | 0 | 6 | 6 | 0 | 220.183 | 1983360 | 0 |
| 10 | 0 | 6 | 6 | 0 | 373.427 | 1999744 | 0 |
