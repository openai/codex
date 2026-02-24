# Codex Local Perf Summary

- Generated: `2026-02-23T10:18:16.942669+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check 'echo exec-smoke && sleep 0.05'`
- Iterations: `4`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-6fop1gb0/codex-home/config.toml`

## Profile

- Name: `codex-sla-exec-v5-top-opt`
- Phase: `measure`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `4`

## Totals

- Successful runs: `4`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 4708.160 | 4477.908 | 5998.180 | 3713.615 | 6163.207 |
| throughput_runs_per_sec | 0.2215 | 0.2272 | 0.2674 | n/a | n/a |
| max_rss_mb | 1.88 | 1.89 | 1.90 | 1.84 | 1.91 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.085 | 0.085 | 0.090 | n/a | n/a |
| system_cpu_sec | 0.087 | 0.085 | 0.098 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 72.8 | 64.5 | 96.9 | n/a | n/a |
| involuntary_ctx_switches | 1513.5 | 1415.0 | 1928.8 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 7.71 | 7.72 | 7.73 | 7.73 |
| sampled_peak_tree_cpu_pct | 3.20 | 3.20 | 3.84 | 3.90 |
| sampled_mean_tree_cpu_pct | 0.25 | 0.26 | 0.36 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 1.053 | 0.187 | 3.188 |
| spawn_proc | 6.449 | 4.738 | 12.595 |
| monitor_loop | 1660.306 | 1544.907 | 2186.209 |
| communicate | 0.150 | 0.143 | 0.181 |
| parse_stats | 0.466 | 0.288 | 0.969 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.02 |
| time | spawn_proc_share_pct | 0.14 |
| time | monitor_loop_share_pct | 35.26 |
| time | communicate_share_pct | 0.00 |
| time | parse_stats_share_pct | 0.01 |
| time | unaccounted_share_pct | 64.56 |
| cpu | cpu_core_utilization_pct | 3.66 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 7.71 |
| process | sampled_peak_tree_cpu_p95 | 3.84 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 25.0 | 24.0 | 31.2 | n/a |
| top_peak_rss_mb | 7.71 | 7.72 | 7.73 | 7.73 |
| top_mean_rss_mb | 7.24 | 7.58 | 7.73 | n/a |
| top_peak_cpu_pct | 3.20 | 3.20 | 3.84 | 3.90 |
| top_mean_cpu_pct | 0.31 | 0.33 | 0.39 | n/a |

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
| turn | 1.00 | 1.00 | 1.00 | 4 | 4040.750 |
| action | 2.00 | 2.00 | 2.00 | 8 | 151.500 |
| stream | 0.00 | 0.00 | 0.00 | 0 | n/a |

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (MB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 6163.207 | 1.84 | 1 |
| 2 | 0 | 1 | 1 | 0 | 3713.615 | 1.91 | 1 |
| 3 | 0 | 1 | 1 | 0 | 5063.022 | 1.89 | 1 |
| 4 | 0 | 1 | 1 | 0 | 3892.794 | 1.89 | 1 |
