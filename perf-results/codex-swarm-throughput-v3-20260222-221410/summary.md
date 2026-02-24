# Codex Local Perf Summary

- Generated: `2026-02-23T05:14:16.980227+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `10`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-nviq82i9/codex-home-worker-1/config.toml`

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
| latency_ms | 273.560 | 263.666 | 365.968 | 210.982 | 399.068 |
| throughput_runs_per_sec | 22.6489 | 22.7921 | 28.1030 | n/a | n/a |
| max_rss_kb | 1945.0 | 1945.0 | 1969.0 | 1921.0 | 1969.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 3.5 | 3.0 | 7.8 | n/a | n/a |
| involuntary_ctx_switches | 292.5 | 289.0 | 389.6 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.7 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 6 | 6 | 0 | 253.186 | 1953 | 0 |
| 2 | 0 | 6 | 6 | 0 | 276.706 | 1969 | 0 |
| 3 | 0 | 6 | 6 | 0 | 210.982 | 1921 | 0 |
| 4 | 0 | 6 | 6 | 0 | 241.647 | 1969 | 0 |
| 5 | 0 | 6 | 6 | 0 | 216.662 | 1953 | 0 |
| 6 | 0 | 6 | 6 | 0 | 252.908 | 1921 | 0 |
| 7 | 0 | 6 | 6 | 0 | 284.779 | 1937 | 0 |
| 8 | 0 | 6 | 6 | 0 | 274.146 | 1937 | 0 |
| 9 | 0 | 6 | 6 | 0 | 399.068 | 1937 | 0 |
| 10 | 0 | 6 | 6 | 0 | 325.513 | 1953 | 0 |
