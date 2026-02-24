# Codex Local Perf Summary

- Generated: `2026-02-23T10:16:34.774767+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `6`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-ir2ivpd_/codex-home-worker-1/config.toml`

## Profile

- Name: `codex-sla-throughput-v4-top-opt`
- Phase: `measure`
- Concurrency: `6`
- Warmup: `1`
- Measured iterations: `6`

## Totals

- Successful runs: `36`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 1254.809 | 1501.804 | 1586.581 | 298.651 | 1590.050 |
| throughput_runs_per_sec | 6.8884 | 4.0007 | 16.4823 | n/a | n/a |
| max_rss_mb | 1.89 | 1.90 | 1.91 | 1.86 | 1.91 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.009 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.001 | 0.001 | 0.004 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.5 | 1.0 | 3.2 | n/a | n/a |
| involuntary_ctx_switches | 92.3 | 79.5 | 147.2 | n/a | n/a |
| peak_open_fds | n/a | n/a | n/a | n/a | n/a |
| peak_direct_children | 0.0 | 0.0 | 0.0 | n/a | 0.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 0.00 | 0.00 | 0.00 | 0.00 |
| sampled_peak_tree_cpu_pct | 0.00 | 0.00 | 0.00 | 0.00 |
| sampled_mean_tree_cpu_pct | 0.00 | 0.00 | 0.00 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 2.394 | 2.313 | 4.404 |
| spawn_proc | 7.254 | 7.143 | 9.305 |
| monitor_loop | 520.350 | 537.686 | 858.920 |
| communicate | 0.148 | 0.147 | 0.199 |
| parse_stats | 0.100 | 0.097 | 0.118 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.19 |
| time | spawn_proc_share_pct | 0.58 |
| time | monitor_loop_share_pct | 41.47 |
| time | communicate_share_pct | 0.01 |
| time | parse_stats_share_pct | 0.01 |
| time | unaccounted_share_pct | 57.74 |
| cpu | cpu_core_utilization_pct | 0.80 |
| process | peak_open_fds_mean | n/a |
| process | peak_direct_children_mean | 0.00 |
| process | sampled_peak_tree_rss_mean_mb | 0.00 |
| process | sampled_peak_tree_cpu_p95 | 0.00 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 1.0 | 1.0 | 1.0 | n/a |
| top_peak_rss_mb | 2.18 | 0.00 | 6.55 | 6.56 |
| top_mean_rss_mb | 0.82 | 0.00 | 2.87 | n/a |
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
| 1 | 0 | 6 | 6 | 0 | 1060.368 | 1.89 | 0 |
| 2 | 0 | 6 | 6 | 0 | 1557.755 | 1.91 | 0 |
| 3 | 0 | 6 | 6 | 0 | 1445.853 | 1.88 | 0 |
| 4 | 0 | 6 | 6 | 0 | 1590.050 | 1.91 | 0 |
| 5 | 0 | 6 | 6 | 0 | 298.651 | 1.86 | 0 |
| 6 | 0 | 6 | 6 | 0 | 1576.175 | 1.91 | 0 |
