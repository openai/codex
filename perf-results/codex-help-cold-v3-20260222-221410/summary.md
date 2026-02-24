# Codex Local Perf Summary

- Generated: `2026-02-23T05:14:17.635774+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `15`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-86iljb5h/codex-home/config.toml`

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
| latency_ms | 203.632 | 165.260 | 337.051 | 123.184 | 344.716 |
| throughput_runs_per_sec | 5.5227 | 6.0511 | 8.0253 | n/a | n/a |
| max_rss_kb | 1905.0 | 1921.0 | 1941.8 | 1857.0 | 1953.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 2.2 | 2.0 | 3.3 | n/a | n/a |
| involuntary_ctx_switches | 219.0 | 240.0 | 348.7 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.8 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 204.283 | 1921 | 0 |
| 2 | 0 | 1 | 1 | 0 | 165.260 | 1921 | 0 |
| 3 | 0 | 1 | 1 | 0 | 161.518 | 1889 | 0 |
| 4 | 0 | 1 | 1 | 0 | 329.938 | 1953 | 0 |
| 5 | 0 | 1 | 1 | 0 | 167.859 | 1857 | 0 |
| 6 | 0 | 1 | 1 | 0 | 281.351 | 1905 | 0 |
| 7 | 0 | 1 | 1 | 0 | 136.829 | 1889 | 0 |
| 8 | 0 | 1 | 1 | 0 | 125.225 | 1921 | 0 |
| 9 | 0 | 1 | 1 | 0 | 123.184 | 1857 | 0 |
| 10 | 0 | 1 | 1 | 0 | 157.887 | 1921 | 0 |
| 11 | 0 | 1 | 1 | 0 | 207.830 | 1937 | 0 |
| 12 | 0 | 1 | 1 | 0 | 344.716 | 1921 | 0 |
| 13 | 0 | 1 | 1 | 0 | 333.766 | 1857 | 0 |
| 14 | 0 | 1 | 1 | 0 | 156.649 | 1921 | 0 |
| 15 | 0 | 1 | 1 | 0 | 158.182 | 1905 | 0 |
