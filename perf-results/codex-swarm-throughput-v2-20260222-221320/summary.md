# Codex Local Perf Summary

- Generated: `2026-02-23T05:13:27.814158+00:00`
- Command: `codex --help >/dev/null`
- Iterations: `10`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-moy5l0lh/codex-home-worker-1/config.toml`

## Profile

- Name: `swarm-throughput`
- Phase: `measure`
- Concurrency: `6`
- Warmup: `2`
- Measured iterations: `10`

## Totals

- Successful runs: `60`
- Failed runs: `0`

## Latency / Throughput / RSS

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| latency_ms | 389.057 | 385.248 | 510.949 | 252.007 | 526.675 |
| throughput_runs_per_sec | 16.2240 | 15.7111 | 22.1446 | n/a | n/a |
| max_rss_kb | 1978444.8 | 1983360.0 | 1992371.2 | 1966976.0 | 1999744.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| system_cpu_sec | 0.010 | 0.010 | 0.010 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 1.5 | 1.0 | 3.6 | n/a | n/a |
| involuntary_ctx_switches | 194.4 | 183.0 | 277.4 | n/a | n/a |
| peak_open_fds | 5.5 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 0.4 | 0.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 6 | 6 | 0 | 421.184 | 1966976 | 0 |
| 2 | 0 | 6 | 6 | 0 | 491.730 | 1999744 | 0 |
| 3 | 0 | 6 | 6 | 0 | 252.007 | 1966976 | 0 |
| 4 | 0 | 6 | 6 | 0 | 340.830 | 1983360 | 0 |
| 5 | 0 | 6 | 6 | 0 | 321.430 | 1966976 | 0 |
| 6 | 0 | 6 | 6 | 0 | 349.311 | 1983360 | 0 |
| 7 | 0 | 6 | 6 | 0 | 461.280 | 1983360 | 0 |
| 8 | 0 | 6 | 6 | 0 | 427.768 | 1966976 | 0 |
| 9 | 0 | 6 | 6 | 0 | 298.352 | 1983360 | 0 |
| 10 | 0 | 6 | 6 | 0 | 526.675 | 1983360 | 0 |
