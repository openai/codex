# Codex Local Perf Summary

- Generated: `2026-02-23T10:11:11.849604+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `8`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-tzlv5grk/codex-home/config.toml`

## Profile

- Name: `codex-sla-cold-v4-top`
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
| latency_ms | 193.428 | 200.519 | 209.719 | 166.089 | 210.094 |
| throughput_runs_per_sec | 5.2094 | 4.9872 | 5.9615 | n/a | n/a |
| max_rss_mb | 1.86 | 1.87 | 1.89 | 1.83 | 1.89 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.009 | 0.010 | 0.010 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 2.5 | 1.5 | 6.9 | n/a | n/a |
| involuntary_ctx_switches | 372.8 | 311.5 | 605.5 | n/a | n/a |
| peak_open_fds | 3.5 | 3.5 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.2 | 0.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 0.14 | 0.00 | 0.75 | 1.16 |
| sampled_peak_tree_cpu_pct | 0.09 | 0.00 | 0.45 | 0.70 |
| sampled_mean_tree_cpu_pct | 0.09 | 0.00 | 0.45 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.527 | 0.365 | 1.097 |
| spawn_proc | 4.955 | 4.155 | 9.556 |
| monitor_loop | 134.389 | 140.073 | 151.978 |
| communicate | 0.498 | 0.450 | 0.959 |
| parse_stats | 0.494 | 0.261 | 1.158 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.27 |
| time | spawn_proc_share_pct | 2.56 |
| time | monitor_loop_share_pct | 69.48 |
| time | communicate_share_pct | 0.26 |
| time | parse_stats_share_pct | 0.26 |
| time | unaccounted_share_pct | 27.18 |
| cpu | cpu_core_utilization_pct | 9.69 |
| process | peak_open_fds_mean | 3.50 |
| process | peak_direct_children_mean | 0.25 |
| process | sampled_peak_tree_rss_mean_mb | 0.14 |
| process | sampled_peak_tree_cpu_p95 | 0.45 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 1.0 | 1.0 | 1.0 | n/a |
| top_peak_rss_mb | 5.71 | 6.53 | 6.54 | 6.55 |
| top_mean_rss_mb | 5.71 | 6.53 | 6.54 | n/a |
| top_peak_cpu_pct | 0.56 | 0.00 | 2.26 | 2.30 |
| top_mean_cpu_pct | 0.56 | 0.00 | 2.26 | n/a |

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
| 1 | 0 | 1 | 1 | 0 | 183.623 | 1.88 | 0 |
| 2 | 0 | 1 | 1 | 0 | 170.900 | 1.86 | 0 |
| 3 | 0 | 1 | 1 | 0 | 201.499 | 1.84 | 0 |
| 4 | 0 | 1 | 1 | 0 | 199.538 | 1.88 | 0 |
| 5 | 0 | 1 | 1 | 0 | 210.094 | 1.89 | 0 |
| 6 | 0 | 1 | 1 | 0 | 166.089 | 1.86 | 0 |
| 7 | 0 | 1 | 1 | 0 | 206.657 | 1.83 | 0 |
| 8 | 0 | 1 | 1 | 0 | 209.023 | 1.88 | 0 |
