# Codex Local Perf Summary

- Generated: `2026-02-23T10:10:15.754940+00:00`
- Command: `sleep 1`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-qz_8j5ss/codex-home/config.toml`

## Profile

- Name: `None`
- Phase: `measure`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `1`

## Totals

- Successful runs: `0`
- Failed runs: `1`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 8017.724 | 8017.724 | 8017.724 | 8017.724 | 8017.724 |
| throughput_runs_per_sec | 0.0000 | 0.0000 | 0.0000 | n/a | n/a |
| max_rss_mb | n/a | n/a | n/a | n/a | n/a |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| system_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| involuntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| peak_open_fds | 117.0 | 117.0 | 117.0 | n/a | 117.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 303.59 | 303.59 | 303.59 | 303.59 |
| sampled_peak_tree_cpu_pct | 148.40 | 148.40 | 148.40 | 148.40 |
| sampled_mean_tree_cpu_pct | 63.46 | 63.46 | 63.46 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 1.497 | 1.497 | 1.497 |
| spawn_proc | 2.685 | 2.685 | 2.685 |
| monitor_loop | 4107.648 | 4107.648 | 4107.648 |
| communicate | 0.057 | 0.057 | 0.057 |
| parse_stats | 0.855 | 0.855 | 0.855 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.02 |
| time | spawn_proc_share_pct | 0.03 |
| time | monitor_loop_share_pct | 51.23 |
| time | communicate_share_pct | 0.00 |
| time | parse_stats_share_pct | 0.01 |
| time | unaccounted_share_pct | 48.70 |
| cpu | cpu_core_utilization_pct | n/a |
| process | peak_open_fds_mean | 117.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 303.59 |
| process | sampled_peak_tree_cpu_p95 | 148.40 |
| stability | failure_rate_pct | 100.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 15.0 | 15.0 | 15.0 | n/a |
| top_peak_rss_mb | 87.78 | 87.78 | 87.78 | 87.78 |
| top_mean_rss_mb | 30.34 | 30.34 | 30.34 | n/a |
| top_peak_cpu_pct | 86.30 | 86.30 | 86.30 | 86.30 |
| top_mean_cpu_pct | 31.93 | 31.93 | 31.93 | n/a |

## vmmap Snapshots

| Metric | Mean | P50 | P95 |
|---|---:|---:|---:|
| vmmap_start_physical_footprint_mb | 0.27 | 0.27 | 0.27 |
| vmmap_mid_physical_footprint_mb | 1.45 | 1.45 | 1.45 |
| vmmap_end_physical_footprint_mb | n/a | n/a | n/a |

## xctrace Hotspots

- Trace captures: `1`
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
| 1 | 1 | 1 | 0 | 1 | 8017.724 | n/a | 0 |
