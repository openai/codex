# Codex Local Perf Summary

- Generated: `2026-02-23T10:17:14.491560+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `6`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-vr0o4xfv/codex-home-worker-1/config.toml`

## Profile

- Name: `codex-sla-throughput-v4-top-opt2`
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
| latency_ms | 69.752 | 69.191 | 74.478 | 65.255 | 75.157 |
| throughput_runs_per_sec | 86.2133 | 86.7455 | 91.2567 | n/a | n/a |
| max_rss_mb | 1.89 | 1.89 | 1.89 | 1.88 | 1.89 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.007 | 0.007 | 0.008 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.0 | 1.0 | 1.0 | n/a | n/a |
| involuntary_ctx_switches | 67.2 | 65.0 | 84.0 | n/a | n/a |
| peak_open_fds | 4.3 | 6.0 | 6.0 | n/a | 6.0 |
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
| build_cmd | 1.795 | 1.916 | 2.340 |
| spawn_proc | 4.280 | 4.133 | 4.997 |
| monitor_loop | 0.000 | 0.000 | 0.000 |
| communicate | 0.067 | 0.068 | 0.071 |
| parse_stats | 0.078 | 0.078 | 0.081 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 2.57 |
| time | spawn_proc_share_pct | 6.14 |
| time | monitor_loop_share_pct | 0.00 |
| time | communicate_share_pct | 0.10 |
| time | parse_stats_share_pct | 0.11 |
| time | unaccounted_share_pct | 91.08 |
| cpu | cpu_core_utilization_pct | 10.75 |
| process | peak_open_fds_mean | 4.33 |
| process | peak_direct_children_mean | 0.00 |
| process | sampled_peak_tree_rss_mean_mb | n/a |
| process | sampled_peak_tree_cpu_p95 | n/a |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 0.0 | 0.0 | 0.0 | n/a |
| top_peak_rss_mb | n/a | n/a | n/a | n/a |
| top_mean_rss_mb | n/a | n/a | n/a | n/a |
| top_peak_cpu_pct | n/a | n/a | n/a | n/a |
| top_mean_cpu_pct | n/a | n/a | n/a | n/a |

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
| 1 | 0 | 6 | 6 | 0 | 75.157 | 1.89 | 0 |
| 2 | 0 | 6 | 6 | 0 | 65.255 | 1.89 | 0 |
| 3 | 0 | 6 | 6 | 0 | 70.469 | 1.89 | 0 |
| 4 | 0 | 6 | 6 | 0 | 72.444 | 1.88 | 0 |
| 5 | 0 | 6 | 6 | 0 | 67.914 | 1.89 | 0 |
| 6 | 0 | 6 | 6 | 0 | 67.276 | 1.89 | 0 |
