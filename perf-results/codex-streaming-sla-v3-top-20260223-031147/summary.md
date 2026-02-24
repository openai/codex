# Codex Local Perf Summary

- Generated: `2026-02-23T10:11:54.724851+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check 'write one short sentence'`
- Iterations: `3`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-wrybantg/codex-home/config.toml`

## Profile

- Name: `codex-streaming-sla-v3-top`
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
| latency_ms | 2347.740 | 2326.821 | 2591.711 | 2095.257 | 2621.143 |
| throughput_runs_per_sec | 0.4295 | 0.4298 | 0.4725 | n/a | n/a |
| max_rss_mb | 1.86 | 1.86 | 1.89 | 1.83 | 1.89 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.077 | 0.080 | 0.080 | n/a | n/a |
| system_cpu_sec | 0.120 | 0.120 | 0.129 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 65.7 | 64.0 | 72.1 | n/a | n/a |
| involuntary_ctx_switches | 4786.3 | 5091.0 | 5762.4 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 7.69 | 7.69 | 7.72 | 7.72 |
| sampled_peak_tree_cpu_pct | 3.93 | 3.60 | 4.68 | 4.80 |
| sampled_mean_tree_cpu_pct | 0.58 | 0.60 | 0.66 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.938 | 0.562 | 1.611 |
| spawn_proc | 6.376 | 5.669 | 8.140 |
| monitor_loop | 1795.505 | 1793.925 | 1961.981 |
| communicate | 0.457 | 0.511 | 0.638 |
| parse_stats | 1.485 | 0.963 | 2.631 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.04 |
| time | spawn_proc_share_pct | 0.27 |
| time | monitor_loop_share_pct | 76.48 |
| time | communicate_share_pct | 0.02 |
| time | parse_stats_share_pct | 0.06 |
| time | unaccounted_share_pct | 23.13 |
| cpu | cpu_core_utilization_pct | 8.38 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 7.69 |
| process | sampled_peak_tree_cpu_p95 | 4.68 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 10.3 | 10.0 | 11.8 | n/a |
| top_peak_rss_mb | 6.53 | 6.53 | 6.56 | 6.56 |
| top_mean_rss_mb | 6.53 | 6.53 | 6.56 | n/a |
| top_peak_cpu_pct | 2.30 | 1.60 | 3.67 | 3.90 |
| top_mean_cpu_pct | 0.37 | 0.27 | 0.63 | n/a |

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
| 1 | 0 | 1 | 1 | 0 | 2621.143 | 1.86 | 1 |
| 2 | 0 | 1 | 1 | 0 | 2095.257 | 1.89 | 1 |
| 3 | 0 | 1 | 1 | 0 | 2326.821 | 1.83 | 1 |
