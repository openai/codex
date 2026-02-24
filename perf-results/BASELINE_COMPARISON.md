# Codex Perf Baseline Comparison

Generated: 2026-02-22

## Baseline Runs

- Smoke baseline: `codex-rs/perf-results/smoke-echo-20260222-070023/summary.json`
- Real workload baseline: `codex-rs/perf-results/codex-tui-footer-test-20260222-164834/summary.json`

## Metrics Comparison

| Metric | Smoke (`/bin/echo ok`) | Real workload (`cargo test ...footer_collapse_snapshots`) |
|---|---:|---:|
| Latency mean (ms) | 31.677 | 2861.973 |
| Latency p50 (ms) | 10.130 | 3020.511 |
| Latency p95 (ms) | 67.950 | 3107.544 |
| Throughput mean (runs/s) | 75.306 | 0.353 |
| Throughput p50 (runs/s) | 98.717 | 0.331 |
| Throughput p95 (runs/s) | 131.433 | 0.401 |
| Max RSS mean (KB) | 885248 | 170690112 |
| Max RSS p50 (KB) | 885248 | 170739264 |
| Max RSS p95 (KB) | 885248 | 170783443 |
| Queue/cancel datapoints | 0 | 0 |

## Notes

- The real workload baseline includes Cargo test runner/build harness overhead.
- Use the smoke run to validate harness health and noise floor.
- Use the real workload run for relative regression tracking across commits.

## Delta Tracking Template

For each new run, append a row to this table against the same workload profile.

| Date | Run folder | Workload profile | Latency p50 (ms) | Latency p95 (ms) | Throughput mean (runs/s) | Max RSS p50 (KB) | Queue/cancel datapoints | Delta summary |
|---|---|---|---:|---:|---:|---:|---:|---|
| 2026-02-22 | `codex-tui-footer-test-20260222-164834` | `cargo test ...footer_collapse_snapshots` | 3020.511 | 3107.544 | 0.353 | 170739264 | 0 | Baseline |
| 2026-02-23 | `local-perf-20260222-172649` | profile:swarm-smoke/measure/c=2 | 105.257 | 175.890 | 42.790 | 885248.000 | 0 | Swarm smoke |
| 2026-02-23 | `swarm-smoke-verify-20260222-172843` | profile:swarm-smoke/measure/c=2 | 74.521 | 92.441 | 28.901 | 885248.000 | 0 | Swarm smoke verify |
| 2026-02-23 | `queue-cancel-stress-20260222-175827` | profile:queue-cancel-stress/stress/c=8 | 75.220 | 82.489 | 0.000 | n/a | 0 | Queue/cancel stress baseline |

## Next Recommended Benchmark Profiles

1. Direct binary profile (no Cargo wrapper) for cleaner runtime-only memory and latency.
2. Multi-agent profile that exercises queue/cancel paths so OTEL queue/cancel metrics populate.
3. Fixed synthetic tool-call profile to track scheduler/backpressure changes across commits.
