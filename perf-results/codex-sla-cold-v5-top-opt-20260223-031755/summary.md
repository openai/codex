# Codex Local Perf Summary

- Generated: `2026-02-23T10:17:56.026957+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `8`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-b1s4omoh/codex-home/config.toml`

## Profile

- Name: `codex-sla-cold-v5-top-opt`
- Phase: `startup`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `8`

## Totals

- Successful runs: `8`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 62.103 | 61.276 | 70.366 | 55.967 | 72.421 |
| throughput_runs_per_sec | 16.2021 | 16.3303 | 17.6423 | n/a | n/a |
| max_rss_mb | 1.86 | 1.84 | 1.89 | 1.83 | 1.89 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.1 | 1.0 | 1.6 | n/a | n/a |
| involuntary_ctx_switches | 59.0 | 56.0 | 79.2 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.0 | 0.0 | 0.0 | n/a | 0.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | n/a | n/a | n/a | n/a |
| sampled_peak_tree_cpu_pct | n/a | n/a | n/a | n/a |
| sampled_mean_tree_cpu_pct | n/a | n/a | n/a | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.137 | 0.134 | 0.166 |
| spawn_proc | 2.012 | 1.929 | 2.313 |
| monitor_loop | 0.000 | 0.000 | 0.000 |
| communicate | 0.117 | 0.090 | 0.232 |
| parse_stats | 0.231 | 0.139 | 0.612 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.22 |
| time | spawn_proc_share_pct | 3.24 |
| time | monitor_loop_share_pct | 0.00 |
| time | communicate_share_pct | 0.19 |
| time | parse_stats_share_pct | 0.37 |
| time | unaccounted_share_pct | 95.98 |
| cpu | cpu_core_utilization_pct | 16.10 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 0.00 |
| process | sampled_peak_tree_rss_mean_mb | n/a |
| process | sampled_peak_tree_cpu_p95 | n/a |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 1.0 | 1.0 | 1.0 | n/a |
| top_peak_rss_mb | 0.00 | 0.00 | 0.00 | 0.00 |
| top_mean_rss_mb | 0.00 | 0.00 | 0.00 | n/a |
| top_peak_cpu_pct | 0.00 | 0.00 | 0.00 | 0.00 |
| top_mean_cpu_pct | 0.00 | 0.00 | 0.00 | n/a |

## vmmap Snapshots

| Metric | Mean | P50 | P95 |
|---|---:|---:|---:|
| vmmap_start_physical_footprint_mb | n/a | n/a | n/a |
| vmmap_mid_physical_footprint_mb | n/a | n/a | n/a |
| vmmap_end_physical_footprint_mb | n/a | n/a | n/a |

## xctrace Hotspots

- Trace captures: `0`
- No hotspots captured.

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Turn / Action / Streaming OTEL Signals

| Signal | Mean points | P50 points | P95 points | Total points | Mean value-sum |
|---|---:|---:|---:|---:|---:|
| turn | 0.00 | 0.00 | 0.00 | 0 | n/a |
| action | 0.00 | 0.00 | 0.00 | 0 | n/a |
| stream | 0.00 | 0.00 | 0.00 | 0 | n/a |

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (MB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 66.548 | 1.89 | 0 |
| 2 | 0 | 1 | 1 | 0 | 55.967 | 1.89 | 0 |
| 3 | 0 | 1 | 1 | 0 | 58.342 | 1.83 | 0 |
| 4 | 0 | 1 | 1 | 0 | 72.421 | 1.84 | 0 |
| 5 | 0 | 1 | 1 | 0 | 59.706 | 1.84 | 0 |
| 6 | 0 | 1 | 1 | 0 | 62.847 | 1.89 | 0 |
| 7 | 0 | 1 | 1 | 0 | 62.933 | 1.83 | 0 |
| 8 | 0 | 1 | 1 | 0 | 58.059 | 1.83 | 0 |
