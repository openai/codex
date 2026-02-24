# Codex Local Perf Summary

- Generated: `2026-02-23T10:11:46.291877+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check 'echo exec-smoke && sleep 0.05'`
- Iterations: `4`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-r73cva1x/codex-home/config.toml`

## Profile

- Name: `codex-sla-exec-v4-top`
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
| latency_ms | 5217.483 | 4953.748 | 6155.587 | 4619.947 | 6342.489 |
| throughput_runs_per_sec | 0.1945 | 0.2020 | 0.2152 | n/a | n/a |
| max_rss_mb | 1.88 | 1.88 | 1.92 | 1.83 | 1.92 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.100 | 0.100 | 0.100 | n/a | n/a |
| system_cpu_sec | 0.140 | 0.140 | 0.157 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 87.0 | 68.0 | 143.2 | n/a | n/a |
| involuntary_ctx_switches | 5536.8 | 5621.0 | 6286.0 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 7.71 | 7.71 | 7.75 | 7.75 |
| sampled_peak_tree_cpu_pct | 2.98 | 3.05 | 3.61 | 3.70 |
| sampled_mean_tree_cpu_pct | 0.20 | 0.21 | 0.24 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.300 | 0.241 | 0.496 |
| spawn_proc | 3.414 | 3.337 | 4.636 |
| monitor_loop | 4024.697 | 3686.450 | 4976.691 |
| communicate | 0.340 | 0.297 | 0.602 |
| parse_stats | 1.200 | 0.567 | 3.024 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.01 |
| time | spawn_proc_share_pct | 0.07 |
| time | monitor_loop_share_pct | 77.14 |
| time | communicate_share_pct | 0.01 |
| time | parse_stats_share_pct | 0.02 |
| time | unaccounted_share_pct | 22.76 |
| cpu | cpu_core_utilization_pct | 4.60 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 7.71 |
| process | sampled_peak_tree_cpu_p95 | 3.61 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 22.2 | 22.5 | 24.0 | n/a |
| top_peak_rss_mb | 6.55 | 6.55 | 6.59 | 6.59 |
| top_mean_rss_mb | 6.48 | 6.52 | 6.59 | n/a |
| top_peak_cpu_pct | 1.45 | 1.45 | 1.67 | 1.70 |
| top_mean_cpu_pct | 0.10 | 0.10 | 0.10 | n/a |

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
| turn | 1.00 | 1.00 | 1.00 | 4 | 3880.250 |
| action | 2.00 | 2.00 | 2.00 | 8 | 191.250 |
| stream | 0.00 | 0.00 | 0.00 | 0 | n/a |

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (MB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 5096.475 | 1.83 | 1 |
| 2 | 0 | 1 | 1 | 0 | 4811.022 | 1.89 | 1 |
| 3 | 0 | 1 | 1 | 0 | 4619.947 | 1.92 | 1 |
| 4 | 0 | 1 | 1 | 0 | 6342.489 | 1.88 | 1 |
