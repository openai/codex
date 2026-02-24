# Codex Local Perf Summary

- Generated: `2026-02-23T05:38:12.399597+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check "Reply with OK"`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-rqw8hkd8/codex-home/config.toml`

## Profile

- Name: `swarm-exec-real-min`
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
| latency_ms | 63744.160 | 63744.160 | 63744.160 | 63744.160 | 63744.160 |
| throughput_runs_per_sec | 0.0000 | 0.0000 | 0.0000 | n/a | n/a |
| max_rss_kb | n/a | n/a | n/a | n/a | n/a |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| system_cpu_sec | n/a | n/a | n/a | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| involuntary_ctx_switches | n/a | n/a | n/a | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 0 | 1 | 63744.160 | n/a | 1 |
