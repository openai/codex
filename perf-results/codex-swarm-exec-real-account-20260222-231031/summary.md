# Codex Local Perf Summary

- Generated: `2026-02-23T06:10:40.124666+00:00`
- Command: `codex exec --sandbox danger-full-access --skip-git-repo-check "Reply with OK only."`
- Iterations: `1`
- Config: `/var/folders/wl/z8733y815nzg28yy0_1_lh7w0000gn/T/codex-perf-v0tsowb4/codex-home/config.toml`

## Profile

- Name: `swarm-exec-real-account`
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
| latency_ms | 8311.011 | 8311.011 | 8311.011 | 8311.011 | 8311.011 |
| throughput_runs_per_sec | 0.1203 | 0.1203 | 0.1203 | n/a | n/a |
| max_rss_kb | 1905.0 | 1905.0 | 1905.0 | 1905.0 | 1905.0 |

## CPU / Scheduler / Process Shape

| Metric | Mean | P50 | P95 | Min | Max |
|---|---:|---:|---:|---:|---:|
| user_cpu_sec | 0.170 | 0.170 | 0.170 | n/a | n/a |
| system_cpu_sec | 0.380 | 0.380 | 0.380 | n/a | n/a |
| cpu_pct | n/a | n/a | n/a | n/a | n/a |
| voluntary_ctx_switches | 609.0 | 609.0 | 609.0 | n/a | n/a |
| involuntary_ctx_switches | 5153.0 | 5153.0 | 5153.0 | n/a | n/a |
| peak_open_fds | 6.0 | 6.0 | 6.0 | n/a | 6.0 |
| peak_direct_children | 1.0 | 1.0 | 1.0 | n/a | 1.0 |

## Queue / Cancel Metrics

No queue/cancel metric datapoints were observed.

## Iteration Return Codes

| Iteration | Return code | Workers | Success | Failed | Duration (ms) | RSS (KB) | OTEL payloads |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1 | 0 | 1 | 1 | 0 | 8311.011 | 1905 | 1 |
