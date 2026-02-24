# Codex Local Perf Summary

- Generated: `2026-02-23T05:09:32.467479+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `15`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-455922_a/codex-home/config.toml`

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
| latency_ms | 155.904 | 132.732 | 271.446 | 104.066 | 379.254 |
| throughput_runs_per_sec | 7.1805 | 7.5340 | 9.5009 | n/a | n/a |
| max_rss_kb | 1942946.1 | 1934208.0 | 1983360.0 | 1901440.0 | 1983360.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| system_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.2 | 1.0 | 2.3 | n/a | n/a |
| involuntary_ctx_switches | 159.1 | 158.0 | 224.5 | n/a | n/a |
| peak_open_fds | n/a | n/a | n/a | n/a | n/a |
| peak_direct_children | 0.7 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 379.254 | 1934208 | 0 |
| 2 | 0 | 1 | 1 | 0 | 133.774 | 1950592 | 0 |
| 3 | 0 | 1 | 1 | 0 | 142.384 | 1901440 | 0 |
| 4 | 0 | 1 | 1 | 0 | 177.351 | 1983360 | 0 |
| 5 | 0 | 1 | 1 | 0 | 104.066 | 1901440 | 0 |
| 6 | 0 | 1 | 1 | 0 | 105.771 | 1966976 | 0 |
| 7 | 0 | 1 | 1 | 0 | 195.614 | 1934208 | 0 |
| 8 | 0 | 1 | 1 | 0 | 131.458 | 1934208 | 0 |
| 9 | 0 | 1 | 1 | 0 | 127.531 | 1934208 | 0 |
| 10 | 0 | 1 | 1 | 0 | 108.395 | 1966976 | 0 |
| 11 | 0 | 1 | 1 | 0 | 122.323 | 1934208 | 0 |
| 12 | 0 | 1 | 1 | 0 | 132.732 | 1983360 | 0 |
| 13 | 0 | 1 | 1 | 0 | 225.243 | 1934208 | 0 |
| 14 | 0 | 1 | 1 | 0 | 136.523 | 1950592 | 0 |
| 15 | 0 | 1 | 1 | 0 | 116.142 | 1934208 | 0 |
