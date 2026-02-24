# Codex Local Perf Summary

- Generated: `2026-02-23T10:18:23.898904+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check 'write one short sentence'`
- Iterations: `3`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-erfa79q1/codex-home/config.toml`

## Profile

- Name: `codex-streaming-sla-v4-top-opt`
- Phase: `measure`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `3`

## Totals

- Successful runs: `3`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 2200.624 | 2343.571 | 2556.473 | 1678.171 | 2580.128 |
| throughput_runs_per_sec | 0.4701 | 0.4267 | 0.5790 | n/a | n/a |
| max_rss_mb | 1.88 | 1.89 | 1.91 | 1.84 | 1.91 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.067 | 0.070 | 0.070 | n/a | n/a |
| system_cpu_sec | 0.083 | 0.080 | 0.089 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 53.7 | 53.0 | 62.0 | n/a | n/a |
| involuntary_ctx_switches | 1315.0 | 1221.0 | 1497.3 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 7.70 | 7.72 | 7.72 | 7.72 |
| sampled_peak_tree_cpu_pct | 2.50 | 2.50 | 2.68 | 2.70 |
| sampled_mean_tree_cpu_pct | 0.37 | 0.34 | 0.43 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.147 | 0.145 | 0.163 |
| spawn_proc | 2.172 | 2.069 | 2.593 |
| monitor_loop | 755.985 | 799.003 | 858.982 |
| communicate | 0.125 | 0.116 | 0.140 |
| parse_stats | 0.420 | 0.244 | 0.808 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.01 |
| time | spawn_proc_share_pct | 0.10 |
| time | monitor_loop_share_pct | 34.35 |
| time | communicate_share_pct | 0.01 |
| time | parse_stats_share_pct | 0.02 |
| time | unaccounted_share_pct | 65.52 |
| cpu | cpu_core_utilization_pct | 6.82 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 7.70 |
| process | sampled_peak_tree_cpu_p95 | 2.68 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 12.0 | 13.0 | 13.9 | n/a |
| top_peak_rss_mb | 7.70 | 7.72 | 7.72 | 7.72 |
| top_mean_rss_mb | 7.17 | 7.17 | 7.32 | n/a |
| top_peak_cpu_pct | 2.80 | 2.70 | 3.15 | 3.20 |
| top_mean_cpu_pct | 0.57 | 0.49 | 0.72 | n/a |

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
| turn | 1.00 | 1.00 | 1.00 | 3 | 1555.667 |
| action | 0.00 | 0.00 | 0.00 | 0 | n/a |
| stream | 0.00 | 0.00 | 0.00 | 0 | n/a |

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (MB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 2343.571 | 1.84 | 1 |
| 2 | 0 | 1 | 1 | 0 | 2580.128 | 1.89 | 1 |
| 3 | 0 | 1 | 1 | 0 | 1678.171 | 1.91 | 1 |
