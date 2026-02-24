# Codex Local Perf Summary

- Generated: `2026-02-23T10:12:23.162086+00:00`
- Command: `python3 -c 'import math,time; t=time.time()+2.0; x=0.0\nwhile time.time()<t: x+=math.sqrt(12345.6789)\nprint(x)'`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-ll_kztr7/codex-home/config.toml`

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
| latency_ms | 17266.891 | 17266.891 | 17266.891 | 17266.891 | 17266.891 |
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
| sampled_peak_tree_rss_mb | 270.25 | 270.25 | 270.25 | 270.25 |
| sampled_peak_tree_cpu_pct | 150.60 | 150.60 | 150.60 | 150.60 |
| sampled_mean_tree_cpu_pct | 36.01 | 36.01 | 36.01 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 6.106 | 6.106 | 6.106 |
| spawn_proc | 4.517 | 4.517 | 4.517 |
| monitor_loop | 9644.861 | 9644.861 | 9644.861 |
| communicate | 0.074 | 0.074 | 0.074 |
| parse_stats | 1.050 | 1.050 | 1.050 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.04 |
| time | spawn_proc_share_pct | 0.03 |
| time | monitor_loop_share_pct | 55.86 |
| time | communicate_share_pct | 0.00 |
| time | parse_stats_share_pct | 0.01 |
| time | unaccounted_share_pct | 44.07 |
| cpu | cpu_core_utilization_pct | n/a |
| process | peak_open_fds_mean | 117.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 270.25 |
| process | sampled_peak_tree_cpu_p95 | 150.60 |
| stability | failure_rate_pct | 100.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 24.0 | 24.0 | 24.0 | n/a |
| top_peak_rss_mb | 90.55 | 90.55 | 90.55 | 90.55 |
| top_mean_rss_mb | 37.72 | 37.72 | 37.72 | n/a |
| top_peak_cpu_pct | 51.60 | 51.60 | 51.60 | 51.60 |
| top_mean_cpu_pct | 15.82 | 15.82 | 15.82 | n/a |

## vmmap Snapshots

| Metric | Mean | P50 | P95 |
|---|---:|---:|---:|
| vmmap_start_physical_footprint_mb | 0.27 | 0.27 | 0.27 |
| vmmap_mid_physical_footprint_mb | 17.10 | 17.10 | 17.10 |
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
| 1 | 1 | 1 | 0 | 1 | 17266.891 | n/a | 0 |
