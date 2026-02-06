# Agent Jobs

This document describes the generic batch job engine used for large agentic workloads.
Agent jobs are designed to be:

1. Resumable and durable via SQLite.
2. Bounded by configured concurrency and thread limits.
3. Observable via explicit status/progress tools.
4. Exportable to CSV at stage boundaries.

## Tools

All tools are function-style and gated by the `collab` feature.

### `spawn_agents_on_csv`

Create a new job from a CSV input and immediately start it.

Required args:
- `csv_path`: path to the CSV file (first row is headers).
- `instruction`: instruction template to apply to each row. Use `{column_name}` placeholders to
  inject values from the CSV row (column names are case-sensitive and may include spaces).
  Use `{{` and `}}` for literal braces.

Optional args:
- `id_column`: header column name to use as a stable item id.
- `job_name`: human-friendly label.
- `output_csv_path`: destination for CSV export (defaults to `<input>.agent-job-<id>.csv`).
- `output_schema`: JSON schema for result payloads (best-effort guidance).
- `max_concurrency`: cap on parallel workers (defaults to 64, then capped by config).
- `auto_export`: auto-export CSV on successful completion (default true).

### `run_agent_job`

Resume an existing job by id. Jobs auto-start when created. When resuming, any items
left in `running` state are reset to `pending` unless they already reported a result.

### `get_agent_job_status`

Return job status and progress counters. Most flows should prefer `wait_agent_job`
to deterministically block until completion.

### `wait_agent_job`

Wait for a job to complete, or return after a timeout.

### `export_agent_job_csv`

Export the current job results to CSV using the stored headers and results. You can
optionally override the destination path.

### `report_agent_job_result`

Workers must call this exactly once to report a JSON object for their assigned item.

## Execution Model

1. Jobs are stored in SQLite with per-item state (pending/running/completed/failed).
2. The job runner spawns subagents up to `max_concurrency`.
3. The job instruction is rendered per row by substituting `{column_name}` placeholders.
4. Each worker processes one item and calls `report_agent_job_result`.
5. The runner marks items completed after the worker finishes.
6. If `auto_export` is true, the runner writes a CSV snapshot on successful completion.
7. CSV export can also be generated manually by a single writer from the SQLite store.

## CSV Output

Exports include original input columns plus:
- `job_id`
- `item_id`
- `row_index`
- `source_id`
- `status`
- `attempt_count`
- `last_error`
- `result_json`
- `reported_at`
- `completed_at`

## CLI Example (Auto-Export)

The example below creates a small CSV, runs a batch job, waits for completion,
and prints the auto-exported CSV.

```bash
./codex-rs/target/debug/codex exec \
  --enable collab \
  --enable sqlite \
  --full-auto \
  -C /path/to/repo \
  - <<'PROMPT'
Create /tmp/security_rank_input_demo.csv with 5 rows using paths:
- codex-rs/core/src/tools/handlers/agent_jobs.rs
- codex-rs/core/src/tools/handlers/shell.rs
- codex-rs/core/src/agent/control.rs
- codex-rs/core/src/exec_policy.rs
- codex-rs/core/src/tools/handlers/mcp.rs
Columns: path, area (use "core" for area).

Then call spawn_agents_on_csv with:
csv_path: /tmp/security_rank_input_demo.csv
instruction: read the file (relative to repo root), skim first 200 lines, score security relevance 1-10, output JSON with keys path, score, rationale, signals array.
output_csv_path: /tmp/security_rank_output_demo.csv

Do NOT call export_agent_job_csv manually.
Wait for completion and then print the output path and `head -n 6` of the CSV.
PROMPT
```
