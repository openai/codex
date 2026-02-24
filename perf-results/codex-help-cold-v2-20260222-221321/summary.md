# Codex Local Perf Summary

- Generated: `2026-02-23T05:13:28.231649+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `15`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-tnka2h2c/codex-home/config.toml`

## Profile

- Name: `swarm-cli-startup-cold`
- Phase: `startup`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `15`

## Totals

- Successful runs: `15`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 256.192 | 239.892 | 465.355 | 122.593 | 494.525 |
| throughput_runs_per_sec | 4.7512 | 4.1685 | 7.4718 | n/a | n/a |
| max_rss_kb | 1946222.9 | 1934208.0 | 1988275.2 | 1901440.0 | 1999744.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.3 | 1.0 | 2.5 | n/a | n/a |
| involuntary_ctx_switches | 140.4 | 124.0 | 206.8 | n/a | n/a |
| peak_open_fds | 4.9 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.3 | 0.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 362.779 | 1999744 | 0 |
| 2 | 0 | 1 | 1 | 0 | 413.497 | 1934208 | 0 |
| 3 | 0 | 1 | 1 | 0 | 263.567 | 1901440 | 0 |
| 4 | 0 | 1 | 1 | 0 | 146.707 | 1934208 | 0 |
| 5 | 0 | 1 | 1 | 0 | 166.805 | 1966976 | 0 |
| 6 | 0 | 1 | 1 | 0 | 239.892 | 1901440 | 0 |
| 7 | 0 | 1 | 1 | 0 | 153.416 | 1934208 | 0 |
| 8 | 0 | 1 | 1 | 0 | 122.593 | 1966976 | 0 |
| 9 | 0 | 1 | 1 | 0 | 209.903 | 1934208 | 0 |
| 10 | 0 | 1 | 1 | 0 | 494.525 | 1983360 | 0 |
| 11 | 0 | 1 | 1 | 0 | 253.955 | 1966976 | 0 |
| 12 | 0 | 1 | 1 | 0 | 143.826 | 1934208 | 0 |
| 13 | 0 | 1 | 1 | 0 | 139.312 | 1950592 | 0 |
| 14 | 0 | 1 | 1 | 0 | 452.854 | 1950592 | 0 |
| 15 | 0 | 1 | 1 | 0 | 279.247 | 1934208 | 0 |
