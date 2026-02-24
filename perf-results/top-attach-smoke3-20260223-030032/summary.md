# Codex Local Perf Summary

- Generated: `2026-02-23T10:00:34.512829+00:00`
- Command: `sleep 0.6`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-til99cd5/codex-home/config.toml`

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
| latency_ms | 2096.850 | 2096.850 | 2096.850 | 2096.850 | 2096.850 |
| throughput_runs_per_sec | 0.4769 | 0.4769 | 0.4769 | n/a | n/a |
| max_rss_mb | 0.86 | 0.86 | 0.86 | 0.86 | 0.86 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 0.0 | 0.0 | 0.0 | n/a | n/a |
| involuntary_ctx_switches | 57.0 | 57.0 | 57.0 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 2.30 | 2.30 | 2.30 | 2.30 |
| sampled_peak_tree_cpu_pct | 0.60 | 0.60 | 0.60 | 0.60 |
| sampled_mean_tree_cpu_pct | 0.60 | 0.60 | 0.60 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.710 | 0.710 | 0.710 |
| spawn_proc | 7.381 | 7.381 | 7.381 |
| monitor_loop | 2033.261 | 2033.261 | 2033.261 |
| communicate | 0.438 | 0.438 | 0.438 |
| parse_stats | 1.038 | 1.038 | 1.038 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.03 |
| time | spawn_proc_share_pct | 0.35 |
| time | monitor_loop_share_pct | 96.97 |
| time | communicate_share_pct | 0.02 |
| time | parse_stats_share_pct | 0.05 |
| time | unaccounted_share_pct | 2.58 |
| cpu | cpu_core_utilization_pct | 0.00 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 2.30 |
| process | sampled_peak_tree_cpu_p95 | 0.60 |
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
| 1 | 0 | 1 | 1 | 0 | 2096.850 | 0.86 | 0 |
