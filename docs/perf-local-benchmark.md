# Local Perf Benchmark Harness (OTEL-backed)

Use `codex-rs/scripts/perf_otel_benchmark.py` for reproducible local runs with optional
multi-agent swarm concurrency.
The script collects:

- latency (wall-clock duration)
- throughput (runs/sec)
- max RSS (from `/usr/bin/time`)
- user/system CPU seconds (from `/usr/bin/time`)
- CPU percent when available (platform-dependent)
- context switches (voluntary/involuntary)
- peak open file descriptors per worker process (sampled while running)
- peak direct child-process fanout per worker process (sampled while running)
- queue/cancel metric datapoints captured from existing `codex-otel` hooks
- optional macOS `top` attach samples for temporal CPU and memory behavior (`--top-attach`)
- optional macOS `vmmap` snapshots for memory-attribution progression (`--vmmap-snapshots`)
- optional macOS `xctrace` Time Profiler captures with extracted top hotspots (`--xctrace-capture`)

## Run

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "cargo run --manifest-path codex-rs/Cargo.toml -p codex-cli -- --help" \
  --iterations 10 \
  --warmup 2 \
  --out-dir codex-rs/perf-results
```

Notes:

- `--concurrency` controls parallel workers per measured iteration (default: `1`).
- At `--concurrency 1`, behavior is sequential and matches prior runs.
- At `--concurrency > 1`, each iteration is a batch window over N parallel worker invocations.
- Optional profile metadata:
  - `--profile-name` (optional)
  - `--profile-phase` (default: `measure`)
- A temporary `CODEX_HOME` is created per benchmark run.
- In concurrent mode, each worker gets an isolated `CODEX_HOME` suffix under the same temp root.
- OTEL metrics are exported to a local in-process HTTP collector via:
  - `[analytics] enabled = true`
  - `[otel] metrics_exporter = { otlp-http = { ..., protocol = "json" } }`
- `--top-attach` enables `top(1)` PID sampling for each worker.
- `--top-interval-ms` controls `top` sampling interval (default: `250` ms).
- `--vmmap-snapshots` captures start/mid/end `vmmap -summary` snapshots.
- `--xctrace-capture` records a Time Profiler trace and extracts top hotspot frames.
- `--xctrace-time-limit-sec` and `--xctrace-hotspots-limit` control capture duration and output depth.
- `--monitor-sleep-ms` controls monitor-loop sleep cadence (lower = more overhead).
- `--probe-interval-ms` controls expensive `ps/pgrep/vmmap` probe frequency.
- `--otel-flush-wait-ms` controls post-run wait before OTEL payload snapshot.

## Frozen Schema Contract (v4)

All benchmark outputs should preserve these keys and units.

| Scope | Key | Unit |
|---|---|---|
| `summary` | `latency_ms.{mean,p50,p95,min,max}` | `ms` |
| `summary` | `throughput_runs_per_sec.{mean,p50,p95}` | `runs/s` |
| `summary` | `max_rss_mb.{mean,p50,p95,min,max}` | `MB` |
| `summary` | `user_cpu_sec.{mean,p50,p95}` | `sec` |
| `summary` | `system_cpu_sec.{mean,p50,p95}` | `sec` |
| `summary` | `cpu_pct.{mean,p50,p95}` | `percent` |
| `summary` | `peak_open_fds.{mean,p50,p95,max}` | `count` |
| `summary` | `peak_direct_children.{mean,p50,p95,max}` | `count` |
| `summary` | `process_tree_sampled.peak_tree_rss_mb.{mean,p50,p95,max}` | `MB` |
| `summary` | `process_tree_sampled.peak_tree_cpu_pct.{mean,p50,p95,max}` | `percent` |
| `summary` | `process_tree_sampled.mean_tree_cpu_pct.{mean,p50,p95}` | `percent` |
| `summary` | `worker_step_timings_ms.{build_cmd,spawn_proc,monitor_loop,communicate,parse_stats}.{mean,p50,p95}` | `ms` |
| `summary` | `top_attach.sample_count.{mean,p50,p95}` | `samples` |
| `summary` | `top_attach.peak_rss_mb.{mean,p50,p95,max}` | `MB` |
| `summary` | `top_attach.mean_rss_mb.{mean,p50,p95}` | `MB` |
| `summary` | `top_attach.peak_cpu_pct.{mean,p50,p95,max}` | `percent` |
| `summary` | `top_attach.mean_cpu_pct.{mean,p50,p95}` | `percent` |
| `summary` | `vmmap_snapshots.start_physical_footprint_mb.{mean,p50,p95}` | `MB` |
| `summary` | `vmmap_snapshots.mid_physical_footprint_mb.{mean,p50,p95}` | `MB` |
| `summary` | `vmmap_snapshots.end_physical_footprint_mb.{mean,p50,p95}` | `MB` |
| `summary` | `xctrace.trace_count` | `count` |
| `summary` | `xctrace.trace_paths` | `paths[]` |
| `summary` | `xctrace.hotspots_top[]` | `frame, weight_ms, samples` |
| `summary` | `otel_turn_action_stream.{turn_metric_points,action_metric_points,stream_metric_points}` | `points` |
| `summary` | `queue_cancel_metrics` | `name->count` |
| `summary` | `successful_runs_total`, `failed_runs_total` | `count` |
| `runs[]` | `duration_ms`, `max_rss_mb` | `ms`, `MB` |
| `runs[]` | `top_sample_count`, `top_peak_rss_mb`, `top_mean_rss_mb`, `top_peak_cpu_pct`, `top_mean_cpu_pct` | `samples`, `MB`, `MB`, `percent`, `percent` |

## Swarm Profiles

Concurrent smoke:

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "/bin/echo ok" \
  --iterations 5 \
  --warmup 0 \
  --concurrency 4 \
  --profile-name "swarm-smoke" \
  --profile-phase "measure"
```

Queue/cancel stress profile:

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "cargo run --manifest-path codex-rs/Cargo.toml -p codex-cli -- exec --sandbox danger-full-access --skip-git-repo-check 'echo queue && sleep 0.1'" \
  --iterations 10 \
  --warmup 2 \
  --concurrency 8 \
  --profile-name "queue-cancel-stress" \
  --profile-phase "stress" \
  --out-dir codex-rs/perf-results
```

Cold startup profile:

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "codex --help >/dev/null" \
  --iterations 15 \
  --warmup 0 \
  --concurrency 1 \
  --profile-name "swarm-cli-startup-cold" \
  --profile-phase "startup" \
  --out-dir codex-rs/perf-results
```

Swarm throughput profile (real exec path):

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "cargo run --manifest-path codex-rs/Cargo.toml -p codex-cli -- exec --sandbox danger-full-access --skip-git-repo-check 'echo swarm-ping && sleep 0.03'" \
  --iterations 10 \
  --warmup 2 \
  --concurrency 6 \
  --profile-name "swarm-throughput" \
  --profile-phase "measure" \
  --out-dir codex-rs/perf-results
```

Attach-mode temporal profile (`top` enabled):

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "codex --help >/dev/null" \
  --iterations 10 \
  --warmup 1 \
  --concurrency 1 \
  --top-attach \
  --top-interval-ms 250 \
  --profile-name "cold-top-attach" \
  --profile-phase "measure" \
  --out-dir codex-rs/perf-results
```

Attribution + hotspot profile (`top` + `vmmap` + `xctrace`):

```bash
python3 codex-rs/scripts/perf_otel_benchmark.py \
  --cmd "codex --help >/dev/null" \
  --iterations 3 \
  --warmup 0 \
  --concurrency 1 \
  --top-attach \
  --vmmap-snapshots \
  --xctrace-capture \
  --xctrace-time-limit-sec 6 \
  --xctrace-hotspots-limit 10 \
  --profile-name "cold-attribution-hotspots" \
  --profile-phase "measure" \
  --out-dir codex-rs/perf-results
```

## Output

Each run writes under `codex-rs/perf-results/<name>-<timestamp>/`:

- `summary.json`: machine-readable aggregate and per-iteration data
- `summary.md`: quick human-readable report
- `iteration-###.json`: per-iteration record

## Reading Results

- `summary.latency_ms`: p50/p95/min/max latency across measured iterations.
- `summary.throughput_runs_per_sec`: derived from successful runs per batch duration.
- `summary.max_rss_mb`: per-iteration max worker resident memory.
- `summary.user_cpu_sec` / `summary.system_cpu_sec`: aggregated CPU-time distribution.
- `summary.cpu_pct`: aggregated CPU-percent distribution when `/usr/bin/time` exposes it.
- `summary.voluntary_ctx_switches` / `summary.involuntary_ctx_switches`: scheduler pressure.
- `summary.peak_open_fds`: per-iteration peak worker FD count (best-effort, platform-dependent).
- `summary.peak_direct_children`: per-iteration peak direct child count (process fanout proxy).
- `summary.top_attach`: `top`-derived temporal sample stats (CPU/RSS).
- `summary.vmmap_snapshots`: start/mid/end physical footprint progression from `vmmap`.
- `summary.xctrace`: trace artifacts and aggregated top hotspot frames.
- `summary.queue_cancel_metrics`: datapoint totals for OTEL metric names matching
  `queue|queued|cancel|cancell|interrupt|abort`.
- `summary.successful_runs_total` / `summary.failed_runs_total`: aggregate worker outcomes.
- `profile`: `{name, phase, concurrency, warmup, iterations}` metadata.
- For `--concurrency > 1`, each iteration record includes `worker_results` with
  `{worker_id, return_code, duration_ms, max_rss_mb, user_cpu_sec, system_cpu_sec, cpu_pct, voluntary_ctx_switches, involuntary_ctx_switches, peak_open_fds, peak_direct_children}`.

If queue/cancel metrics are absent, `summary.queue_cancel_metrics` will be empty for that run.

## SLA Targets (Local, Account-Auth)

Reference runs:

- `codex-rs/codex-rs/perf-results/codex-sla-cold-v2-quiet-20260223-013621/summary.json`
- `codex-rs/codex-rs/perf-results/codex-sla-throughput-v2-quiet-20260223-013652/summary.json`
- `codex-rs/codex-rs/perf-results/codex-sla-exec-v2-quiet-20260223-013725/summary.json`

Proposed initial thresholds:

- Cold start (`codex --help`, concurrency `1`)
  - `failed_runs_total == 0`
  - `latency_ms.p95 <= 1000`
  - `throughput_runs_per_sec.mean >= 2.0`
  - `peak_open_fds.mean <= 6`
- Swarm throughput (`codex --help`, concurrency `6`)
  - `failed_runs_total == 0`
  - `latency_ms.p95 <= 1200`
  - `throughput_runs_per_sec.mean >= 8.0`
  - `peak_open_fds.mean <= 6`
- Real exec (`codex exec`, concurrency `1`)
  - `failed_runs_total == 0`
  - `latency_ms.p95 <= 8000`
  - `throughput_runs_per_sec.mean >= 0.14`
  - `peak_direct_children.mean <= 1.5`

Use `codex-rs/scripts/perf_sla_check.py` to enforce:

```bash
python3 codex-rs/scripts/perf_sla_check.py \
  --summary codex-rs/codex-rs/perf-results/codex-sla-cold-v2-quiet-20260223-013621/summary.json \
  --label cold \
  --max-failed-runs 0 \
  --max-latency-p95-ms 1000 \
  --min-throughput-mean 2.0 \
  --max-peak-fds-mean 6

python3 codex-rs/scripts/perf_sla_check.py \
  --summary codex-rs/codex-rs/perf-results/codex-sla-throughput-v2-quiet-20260223-013652/summary.json \
  --label throughput \
  --max-failed-runs 0 \
  --max-latency-p95-ms 1200 \
  --min-throughput-mean 8.0 \
  --max-peak-fds-mean 6

python3 codex-rs/scripts/perf_sla_check.py \
  --summary codex-rs/codex-rs/perf-results/codex-sla-exec-v2-quiet-20260223-013725/summary.json \
  --label exec \
  --max-failed-runs 0 \
  --max-latency-p95-ms 8000 \
  --min-throughput-mean 0.14 \
  --max-peak-children-mean 1.5
```

## Expansion: Turn / Action / Streaming Analysis

`codex-rs/scripts/perf_otel_benchmark.py` now emits OTEL-derived per-iteration and aggregate
signal families in `summary.otel_turn_action_stream`:

- `turn_metric_points` and `turn_metric_value_sum`
- `action_metric_points` and `action_metric_value_sum`
- `stream_metric_points` and `stream_metric_value_sum`

These are derived from metric-name pattern groups over exported OTEL datapoints:

- `turn`: names matching `turn.*(duration|latency)` or `codex.turn`
- `action`: names matching `(tool|exec_command|apply_patch|search|agent).*(duration|latency)` or `codex.tool`
- `stream`: names matching `(stream|first_token|first_event|ttfb|chunk).*(duration|latency|ms)`

You can gate signal presence/value budgets with `codex-rs/scripts/perf_sla_check.py`:

```bash
python3 codex-rs/scripts/perf_sla_check.py \
  --summary <summary.json> \
  --label streaming-sla \
  --max-failed-runs 0 \
  --min-turn-metric-points-mean 1 \
  --min-action-metric-points-mean 1 \
  --min-stream-metric-points-mean 1
```

Notes:

- Point counts are best used as coverage/assertion signals for instrumentation.
- Value sums are useful as bounded trend signals, not exact end-to-end latency replacements.
- Keep end-to-end SLA gates on `latency_ms` and `throughput_runs_per_sec` as primary.

## Granular Resource Breakdown (Per Worker Step)

New summary blocks now emitted by `perf_otel_benchmark.py`:

- `summary.process_tree_sampled`
  - `peak_tree_rss_mb` (mean/p50/p95/max)
  - `peak_tree_cpu_pct` (mean/p50/p95/max)
  - `mean_tree_cpu_pct` (mean/p50/p95)
- `summary.worker_step_timings_ms`
  - `build_cmd`
  - `spawn_proc`
  - `monitor_loop`
  - `communicate`
  - `parse_stats`

Per-iteration payloads include these fields too, plus worker-level fields under `worker_results`
for concurrent runs.

`perf_sla_check.py` now supports gating these:

- `--max-sampled-peak-tree-rss-mean-mb`
- `--max-sampled-peak-tree-cpu-p95`
- `--max-build-cmd-mean-ms`
- `--max-spawn-proc-mean-ms`
- `--max-monitor-loop-mean-ms`
- `--max-communicate-mean-ms`
- `--max-parse-stats-mean-ms`
- `--min-top-sample-count-mean`
- `--max-top-peak-rss-mean-mb`
- `--max-top-peak-cpu-p95`
- `--max-top-mean-cpu-mean`
- `--max-vmmap-start-physical-mean-mb`
- `--max-vmmap-mid-physical-mean-mb`
- `--max-vmmap-end-physical-mean-mb`
- `--min-xctrace-trace-count`

## SLA Profiles (One-Command)

Use `codex-rs/scripts/perf_sla_profiles.py` to apply named thresholds without manually
passing every `perf_sla_check.py` argument.

Available profiles:

- `cold`
- `throughput`
- `exec`
- `streaming`

Examples:

```bash
python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile cold \
  --summary codex-rs/codex-rs/perf-results/codex-sla-cold-v2-quiet-20260223-013621/summary.json

python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile throughput \
  --summary codex-rs/codex-rs/perf-results/codex-sla-throughput-v2-quiet-20260223-013652/summary.json

python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile exec \
  --summary codex-rs/codex-rs/perf-results/codex-sla-exec-v2-quiet-20260223-013725/summary.json
```

To add one-off overrides (forwarded directly to `perf_sla_check.py`), use `--extra`:

```bash
python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile cold \
  --summary <summary.json> \
  --extra --max-latency-p95-ms 900 --max-peak-fds-mean 5
```

### All-Map Matrix

Run all profiles with profile-specific summaries and get one matrix:

```bash
python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile all \
  --cold-summary codex-rs/codex-rs/perf-results/codex-sla-cold-v2-quiet-20260223-013621/summary.json \
  --throughput-summary codex-rs/codex-rs/perf-results/codex-sla-throughput-v2-quiet-20260223-013652/summary.json \
  --exec-summary codex-rs/codex-rs/perf-results/codex-sla-exec-v2-quiet-20260223-013725/summary.json \
  --streaming-summary codex-rs/codex-rs/perf-results/codex-sla-signals-exec-smoke-20260223-014637/summary.json
```

### Passing All-Map Baseline (Current)

```bash
python3 codex-rs/scripts/perf_sla_profiles.py \
  --profile all \
  --cold-summary codex-rs/codex-rs/perf-results/codex-sla-cold-v2-quiet-20260223-013621/summary.json \
  --throughput-summary codex-rs/codex-rs/perf-results/codex-sla-throughput-v2-quiet-20260223-013652/summary.json \
  --exec-summary codex-rs/codex-rs/perf-results/codex-sla-exec-v2-quiet-20260223-013725/summary.json \
  --streaming-summary codex-rs/codex-rs/perf-results/codex-streaming-sla-v1-20260223-015850/summary.json
```

## Normalized Budget Metrics

`summary.resource_budget` provides decision-friendly normalized metrics:

- `time_budget_ms`: wall-time shares by worker step (`build_cmd`, `spawn_proc`, `monitor_loop`, `communicate`, `parse_stats`) plus `unaccounted_*`.
- `cpu_budget`: user/system/total CPU seconds and `cpu_core_utilization_pct` normalized by wall time.
- `process_budget`: mean FD/child fanout and sampled process-tree RSS/CPU.
- `stability`: total/success/failed/timeout runs with failure and timeout rates.

Use this section first when comparing CLIs temporally and by non-memory resource usage.

## Baseline Indices + Auto Suggestions

Build statistically factored baseline indices from one or more `summary.json` files:

```bash
python3 codex-rs/scripts/perf_baseline_index.py \
  --summary <summary-1.json> \
  --summary <summary-2.json> \
  --summary <summary-3.json> \
  --summary-glob 'codex-rs/perf-results/*/summary.json' \
  --group-mode heuristic \
  --window 30 \
  --min-samples 5 \
  --out codex-rs/perf-results/perf-baseline-index.json
```

Generate automated bottleneck warnings/suggestions:

```bash
python3 codex-rs/scripts/perf_bottleneck_suggest.py \
  --index codex-rs/perf-results/perf-baseline-index.json \
  --out codex-rs/perf-results/perf-bottleneck-suggestions.json
```

`perf_baseline_index.py` emits robust stats per metric:

- center: mean, median, 10% trimmed mean
- tails: p95, p99, min, max
- spread: std, CV, MAD, IQR
- drift: delta percent from median
- severity index: robust z-score + status (`green`/`yellow`/`red`/`insufficient_data`)

`perf_bottleneck_suggest.py` maps warning signals to likely bottlenecks and concrete suggestions.
