# Codex Local Perf Summary

- Generated: `2026-02-23T10:17:57.525112+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `6`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-mgxdlj4k/codex-home-worker-1/config.toml`

## Profile

- Name: `codex-sla-throughput-v5-top-opt`
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
| latency_ms | 130.320 | 128.988 | 149.797 | 116.147 | 154.638 |
| throughput_runs_per_sec | 46.4824 | 46.5918 | 51.4682 | n/a | n/a |
| max_rss_mb | 1.90 | 1.89 | 1.92 | 1.88 | 1.92 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.009 | 0.008 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.0 | 1.0 | 1.0 | n/a | n/a |
| involuntary_ctx_switches | 80.0 | 79.0 | 96.0 | n/a | n/a |
| peak_open_fds | 5.0 | 6.0 | 6.0 | n/a | 6.0 |
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
| build_cmd | 1.605 | 1.385 | 3.207 |
| spawn_proc | 6.849 | 6.533 | 9.126 |
| monitor_loop | 0.000 | 0.000 | 0.000 |
| communicate | 0.108 | 0.082 | 0.182 |
| parse_stats | 0.089 | 0.086 | 0.107 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 1.23 |
| time | spawn_proc_share_pct | 5.26 |
| time | monitor_loop_share_pct | 0.00 |
| time | communicate_share_pct | 0.08 |
| time | parse_stats_share_pct | 0.07 |
| time | unaccounted_share_pct | 93.36 |
| cpu | cpu_core_utilization_pct | 6.61 |
| process | peak_open_fds_mean | 5.00 |
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
| 1 | 0 | 6 | 6 | 0 | 135.273 | 1.92 | 0 |
| 2 | 0 | 6 | 6 | 0 | 123.785 | 1.89 | 0 |
| 3 | 0 | 6 | 6 | 0 | 134.191 | 1.88 | 0 |
| 4 | 0 | 6 | 6 | 0 | 154.638 | 1.89 | 0 |
| 5 | 0 | 6 | 6 | 0 | 117.885 | 1.91 | 0 |
| 6 | 0 | 6 | 6 | 0 | 116.147 | 1.89 | 0 |
