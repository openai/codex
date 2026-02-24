# Codex Local Perf Summary

- Generated: `2026-02-23T10:00:52.035898+00:00`
- Command: `sleep 0.6`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-t1e86ctu/codex-home/config.toml`

## Profile

- Name: `None`
- Phase: `measure`
- Concurrency: `1`
- Warmup: `0`
- Measured iterations: `1`

## Totals

- Successful runs: `1`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 1105.898 | 1105.898 | 1105.898 | 1105.898 | 1105.898 |
| throughput_runs_per_sec | 0.9042 | 0.9042 | 0.9042 | n/a | n/a |
| max_rss_mb | 0.84 | 0.84 | 0.84 | 0.84 | 0.84 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 0.0 | 0.0 | 0.0 | n/a | n/a |
| involuntary_ctx_switches | 34.0 | 34.0 | 34.0 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 2.28 | 2.28 | 2.28 | 2.28 |
| sampled_peak_tree_cpu_pct | 1.00 | 1.00 | 1.00 | 1.00 |
| sampled_mean_tree_cpu_pct | 1.00 | 1.00 | 1.00 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.448 | 0.448 | 0.448 |
| spawn_proc | 8.066 | 8.066 | 8.066 |
| monitor_loop | 1043.917 | 1043.917 | 1043.917 |
| communicate | 0.120 | 0.120 | 0.120 |
| parse_stats | 1.025 | 1.025 | 1.025 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.04 |
| time | spawn_proc_share_pct | 0.73 |
| time | monitor_loop_share_pct | 94.40 |
| time | communicate_share_pct | 0.01 |
| time | parse_stats_share_pct | 0.09 |
| time | unaccounted_share_pct | 4.73 |
| cpu | cpu_core_utilization_pct | 0.00 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 2.28 |
| process | sampled_peak_tree_cpu_p95 | 1.00 |
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
| 1 | 0 | 1 | 1 | 0 | 1105.898 | 0.84 | 0 |
