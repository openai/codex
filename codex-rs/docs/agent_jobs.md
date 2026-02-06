# Agent Jobs

This document describes the generic batch job engine used for large agentic workloads.
Agent jobs are designed to be:

1. Durable via SQLite for progress tracking and export.
2. Bounded by configured concurrency and thread limits.
3. Exportable to CSV on successful completion.

## Tools

All tools are function-style and gated by the `collab` feature.

### `spawn_agents_on_csv`

Create a new job from a CSV input and immediately start it.
This tool blocks until the job completes and auto-exports on success.

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

### `report_agent_job_result`

Worker-only tool used internally for reporting JSON results. Main agents should not call this.

## Execution Model

1. Jobs are stored in SQLite with per-item state (pending/running/completed/failed).
2. The job runner spawns subagents up to `max_concurrency`.
3. The job instruction is rendered per row by substituting `{column_name}` placeholders.
4. Each worker processes one item and reports results through `report_agent_job_result`.
5. The runner marks items completed after the worker finishes.
6. The runner writes a CSV snapshot on successful completion.

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

After completion, print the output path and `head -n 6` of the CSV.
PROMPT
```
