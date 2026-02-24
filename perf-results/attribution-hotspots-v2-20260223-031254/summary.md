# Codex Local Perf Summary

- Generated: `2026-02-23T10:13:12.962507+00:00`
- Command: `/bin/sh -c 'i=0; while [  -lt 500000 ]; do i=1; :; done'`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-u9o449qh/codex-home/config.toml`

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
| latency_ms | 18330.198 | 18330.198 | 18330.198 | 18330.198 | 18330.198 |
| throughput_runs_per_sec | 0.0546 | 0.0546 | 0.0546 | n/a | n/a |
| max_rss_mb | n/a | n/a | n/a | n/a | n/a |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| system_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| involuntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| peak_open_fds | 185.0 | 185.0 | 185.0 | n/a | 185.0 |
| peak_direct_children | 2.0 | 2.0 | 2.0 | n/a | 2.0 |

## Process Tree Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| sampled_peak_tree_rss_mb | 363.69 | 363.69 | 363.69 | 363.69 |
| sampled_peak_tree_cpu_pct | 250.90 | 250.90 | 250.90 | 250.90 |
| sampled_mean_tree_cpu_pct | 42.80 | 42.80 | 42.80 | n/a |

## Worker Step Timings (ms)

| Step | Mean | P50 | P95 |
|---|---:|---:|---:|
| build_cmd | 0.592 | 0.592 | 0.592 |
| spawn_proc | 2.819 | 2.819 | 2.819 |
| monitor_loop | 9717.405 | 9717.405 | 9717.405 |
| communicate | 0.060 | 0.060 | 0.060 |
| parse_stats | 0.984 | 0.984 | 0.984 |

## Resource Budget (Normalized)

| Budget | Metric | Value |
|---|---|---:|
| time | build_cmd_share_pct | 0.00 |
| time | spawn_proc_share_pct | 0.02 |
| time | monitor_loop_share_pct | 53.01 |
| time | communicate_share_pct | 0.00 |
| time | parse_stats_share_pct | 0.01 |
| time | unaccounted_share_pct | 46.96 |
| cpu | cpu_core_utilization_pct | n/a |
| process | peak_open_fds_mean | 185.00 |
| process | peak_direct_children_mean | 2.00 |
| process | sampled_peak_tree_rss_mean_mb | 363.69 |
| process | sampled_peak_tree_cpu_p95 | 250.90 |
| stability | failure_rate_pct | 0.00 |
| stability | timeout_rate_pct | 0.00 |

## top Attach Samples

| Metric | Mean | P50 | P95 | Max |
|---|---:|---:|---:|---:|
| top_sample_count | 39.0 | 39.0 | 39.0 | n/a |
| top_peak_rss_mb | 88.38 | 88.38 | 88.38 | 88.38 |
| top_mean_rss_mb | 23.02 | 23.02 | 23.02 | n/a |
| top_peak_cpu_pct | 80.00 | 80.00 | 80.00 | 80.00 |
| top_mean_cpu_pct | 8.21 | 8.21 | 8.21 | n/a |

## vmmap Snapshots

| Metric | Mean | P50 | P95 |
|---|---:|---:|---:|
| vmmap_start_physical_footprint_mb | 0.45 | 0.45 | 0.45 |
| vmmap_mid_physical_footprint_mb | 19.90 | 19.90 | 19.90 |
| vmmap_end_physical_footprint_mb | n/a | n/a | n/a |

## xctrace Hotspots

- Trace captures: `1`

| Frame | Weight (ms) | Samples |
|---|---:|---:|
| `dyld4::RuntimeState::loadInsertedLibraries(dyld3::OverflowSafeArray<dyld4::Loader*, 4294967295ull>&, dyld4::Loader const*)` | 1.000 | 1 |
| `__sysctl` | 1.000 | 1 |
| `0x1029695a1` | 1.000 | 1 |
| `<deduplicated_symbol>` | 1.000 | 1 |
| `0x102656474` | 1.000 | 1 |

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
| 1 | 0 | 1 | 1 | 0 | 18330.198 | n/a | 0 |
