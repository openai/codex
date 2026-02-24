# Codex Local Perf Summary

- Generated: `2026-02-23T10:02:17.881407+00:00`
- Command: `sleep 0.6`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-pp4lc_5p/codex-home/config.toml`

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
| latency_ms | 714.387 | 714.387 | 714.387 | 714.387 | 714.387 |
| throughput_runs_per_sec | 1.3998 | 1.3998 | 1.3998 | n/a | n/a |
| max_rss_mb | 0.84 | 0.84 | 0.84 | 0.84 | 0.84 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| system_cpu_sec | 0.000 | 0.000 | 0.000 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 0.0 | 0.0 | 0.0 | n/a | n/a |
| involuntary_ctx_switches | 11.0 | 11.0 | 11.0 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 2.28 | 2.28 | 2.28 | 2.28 |
| sampled_peak_tree_cpu_pct | 0.50 | 0.50 | 0.50 | 0.50 |
| sampled_mean_tree_cpu_pct | 0.25 | 0.25 | 0.25 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.258 | 0.258 | 0.258 |
| spawn_proc | 2.832 | 2.832 | 2.832 |
| monitor_loop | 492.746 | 492.746 | 492.746 |
| communicate | 0.139 | 0.139 | 0.139 |
| parse_stats | 1.153 | 1.153 | 1.153 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.04 |
| time | spawn_proc_share_pct | 0.40 |
| time | monitor_loop_share_pct | 68.97 |
| time | communicate_share_pct | 0.02 |
| time | parse_stats_share_pct | 0.16 |
| time | unaccounted_share_pct | 30.41 |
| cpu | cpu_core_utilization_pct | 0.00 |
| process | peak_open_fds_mean | 6.00 |
| process | peak_direct_children_mean | 1.00 |
| process | sampled_peak_tree_rss_mean_mb | 2.28 |
| process | sampled_peak_tree_cpu_p95 | 0.50 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 4.0 | 4.0 | 4.0 | n/a |
| top_peak_rss_mb | 1.12 | 1.12 | 1.12 | 1.12 |
| top_mean_rss_mb | 1.12 | 1.12 | 1.12 | n/a |
| top_peak_cpu_pct | 0.40 | 0.40 | 0.40 | 0.40 |
| top_mean_cpu_pct | 0.12 | 0.12 | 0.12 | n/a |

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
| 1 | 0 | 1 | 1 | 0 | 714.387 | 0.84 | 0 |
