AGENTS.md

Celaya Solutions header

Organization: Celaya Solutions

Project: LMU Curriculum Runtime

Version: 0.1.0

Generated: {{generated_utc}}

Purpose: Programmable inference curriculum engine (local-first, fault-tolerant)

Analogy: CUDA → GPU compute :: LMU → token compute

Status: experimental

Overview

This repo is a curriculum runtime, not a content dump.

Agents are roles Codex uses to plan, implement, validate, and ship a system that:

- runs locally (Ollama default)
- treats model output as untrusted input
- never crashes due to malformed generations
- emits receipts for every action
- allows partial success
- enforces CUDA mapping per lesson

Agent contract

All agents must:

- write artifacts, never only prose
- emit receipts and summaries for work they run
- enforce execpolicy rules
- prefer minimal diffs and explicit interfaces
- fail fast on policy violations, not on model output

Agent list

1. Repo Orchestrator
Goal
Coordinate changes across modules and commands.

Responsibilities

- define target repo structure under celaya/lmu/
- wire commands (codex lmu run, python run.py) to LMU pipeline
- ensure one-command behavior and preflight gates
- ensure partial success semantics across lesson generation

Inputs

- syllabus.yaml
- global config (LMU_MODEL, LMU_LESSONS, LMU_MAX_RETRIES, OLLAMA_BASE_URL)

Outputs

- run entrypoints
- preflight checks
- generation_summary.json
- manifest.sha256

Acceptance checks

- python run.py completes with summary even with malformed model output
- receipts written for all major events
1. Syllabus Architect
Goal
Define lesson order, dependencies, and success criteria.

Responsibilities

- author celaya/lmu/syllabus/syllabus.yaml
- define phases and lesson dependency graph
- define required CUDA mapping fields per lesson
- define per-lesson success metrics and artifacts

Outputs

- syllabus.yaml
- phases.md
- lesson template schema for spec.md and tasks.json

Acceptance checks

- every lesson has objective, constraints, success criteria, CUDA mapping
- dependencies are acyclic and resolvable
1. Prompt Library Engineer
Goal
Centralize and version prompt templates.

Responsibilities

- author celaya/lmu/generator/prompts.py
- ban inline prompts outside prompts.py
- implement staged prompts: plan → generate → validate → retry
- enforce JSON-only contracts and schema constraints

Outputs

- prompts.py with named prompt builders
- prompt versioning strategy (prompt_sha256)

Acceptance checks

- prompts produce structured outputs under strict contracts
- retries change constraints, do not repeat the same prompt
1. Extraction and Parsing Engineer
Goal
Make parsing non-fatal and predictable.

Responsibilities

- implement celaya/lmu/generator/extract.py
- strip markdown fences
- extract first valid JSON region
- sanitize common errors (trailing commas, whitespace)
- provide survival path (empty object + receipt) instead of crash

Outputs

- extract.py with must_json and try_json utilities
- extraction receipts for failures (parse_error events)

Acceptance checks

- no uncaught JSONDecodeError from model output
- invalid outputs do not terminate the run
1. Contract and Validator Engineer
Goal
Enforce schemas outside the model.

Responsibilities

- implement celaya/lmu/generator/validators.py
- define strict schemas for plan, lesson spec, tasks, grading weights
- return structured validation errors for retries

Outputs

- validators.py
- schema definitions (python dict schemas or jsonschema files)

Acceptance checks

- validator returns machine-readable errors
- pipeline uses validator errors to tighten prompts
1. Runtime Engineer
Goal
Implement named ops and fault isolation.

Responsibilities

- implement celaya/lmu/runtime/runner.py
- implement op execution with timing
- isolate failures per op and per lesson
- ensure lesson_skip and continue semantics

Outputs

- runner.py
- op receipts (op_start/op_done/op_fail)
- lesson completion receipts

Acceptance checks

- failures are contained, logged, and do not crash the run
- p50/p95 metrics computed from receipts
1. Receipts and Observability Engineer
Goal
Receipts are the source of truth.

Responsibilities

- implement celaya/lmu/runtime/receipts.py
- define required receipt events and fields
- implement summary aggregation writers

Required events

- run_start, run_complete
- lesson_start, lesson_skip, lesson_complete
- op_start, op_done, op_fail
- attempt_start, attempt_fail, attempt_success
- cache_hit, cache_miss
- preflight_ok, preflight_fail

Outputs

- receipts.jsonl per run
- summary.json per lesson and global

Acceptance checks

- receipts are append-only JSONL
- summaries match receipts-derived calculations
1. Grading and Weights Engineer
Goal
Score artifacts with partial credit.

Responsibilities

- implement celaya/lmu/grading/grader.py
- maintain celaya/lmu/grading/weights.json
- compute lesson score and global score from artifacts present and validated

Outputs

- weights.json
- grade_report.json per lesson
- global grade summary

Acceptance checks

- scoring is deterministic and reproducible
- missing artifacts reduce score, do not crash the run
1. Benchmark and Metrics Engineer
Goal
Compare baseline vs LMU behavior.

Responsibilities

- implement baseline harness (variance, latency)
- implement LMU harness (pass_rate, retries, latency distributions)
- output comparison_report.json

Outputs

- comparison_report.json
- metrics tables in summaries

Acceptance checks

- p50/p95 computed correctly
- stable across reruns given same environment
1. CUDA Mapping Steward
Goal
Enforce CUDA analogy consistently.

Responsibilities

- require CUDA mapping block in every lesson spec.md
- validate mapping keys exist:
op→kernel, runner→launch, lane→warp, kv→sram/hbm, receipts→profiler
- fail lesson generation if missing mapping, but do not crash run

Outputs

- cuda_mapping.md template
- validator rule enforcing presence

Acceptance checks

- every lesson includes mapping section
- missing mapping triggers retry or skip with receipt
1. Branding and Docs Steward
Goal
Keep Celaya Solutions identity consistent.

Responsibilities

- maintain global header template
- ensure header injected into every generated file
- maintain CELAYA.md and LMU definition docs

Outputs

- CELAYA.md
- header template utility
- docs updates

Acceptance checks

- every generated file starts with header block
- no hype language, no AGI claims
1. Preflight and CI Steward
Goal
Ensure one-command reliability.

Responsibilities

- implement preflight checks:
python version
module compilation
ollama reachable
model installed
write permissions
- add minimal CI scripts if present

Outputs

- preflight module
- fail reasons as single-line messages

Acceptance checks

- run exits early only on environment failures, not model output failures

Interaction protocol for Codex

When making changes:

- create a short plan
- implement smallest diff
- run local checks (py_compile, a minimal smoke run)
- emit receipts for any run executed
- update docs only after code works

Default ownership map

- generator/*: Prompt Library, Extraction, Validators
- runtime/*: Runtime, Receipts
- grading/*: Grading
- syllabus/*: Syllabus Architect, CUDA Mapping
- root run.py and CLI wiring: Repo Orchestrator, Preflight

End of AGENTS.md

EXECPOLICY.md

Celaya Solutions header

Organization: Celaya Solutions

Project: LMU Curriculum Runtime

Version: 0.1.0

Generated: {{generated_utc}}

Purpose: Execution policies for deterministic, local-first curriculum generation

Status: experimental

Scope

This policy governs code changes and runtime behavior for the LMU curriculum engine inside this repository.

Non-negotiable invariants

1. Local-first
- Default base URL is http://localhost:11434
- Cloud calls are forbidden by default
- Network access is only for Ollama unless explicitly enabled by a config flag
1. Never crash on model output
- Any model output is untrusted input
- Parsing must be defensive
- JSON parsing errors must not terminate the run
- Failures must become receipts and skips, not exceptions
1. Receipts are mandatory
- Every run writes receipts.jsonl
- Every lesson writes summary.json
- Every significant event emits a receipt
1. Partial success is required
- A single lesson failure must not abort the full run
- Skipped lessons are permitted and must be logged
1. Determinism of the runtime, not the model
- Runtime behavior must be deterministic given the same receipts and configuration
- Randomness must be explicit and controlled
- Any sampling parameters must be recorded in receipts
1. CUDA analogy enforcement
- Every lesson must include CUDA mapping fields
- Missing mapping triggers retry or skip with receipt

Forbidden changes

- Adding fine-tuning, training, weight updates, or model architecture changes
- Adding default cloud dependencies or requiring API keys
- Adding background daemons not necessary for generation
- Adding hidden telemetry or silent network calls
- Writing outputs outside the configured output directory
- Deleting user artifacts during normal execution

Required preflight gates

Before generation begins, the system must:

- verify Python version meets minimum requirement
- compile-check critical modules (py_compile)
- verify Ollama reachable (GET /api/tags)
- verify LMU_MODEL installed
- verify output directory writable

If preflight fails:

- emit preflight_fail receipt
- exit with a single clear error line
- do not partially generate content

Model output handling rules

All model-driven structured outputs must follow:

- JSON-only requirement
- no markdown fences
- no prose outside JSON

Extraction rules

- strip markdown fences if present
- locate the outermost JSON object region
- sanitize common faults (trailing commas, tabs/newlines)
- try parse
- on failure:
    - emit parse_error receipt with reason and output_preview
    - return empty object
    - upstream must treat empty object as invalid and skip or retry
- 

Validation rules

- schema validation occurs after extraction, before file writes
- validators return machine-readable errors
- retry prompts must incorporate validation errors

Retry budget rules

- MAX_RETRIES default is small (0–3)
- retries must change constraints
- retries must record attempt_start/attempt_fail/attempt_success receipts
- after budget exhausted:
    - emit lesson_skip
    - continue
- 

Artifact writing rules

- Every generated file begins with Celaya Solutions header
- Every lesson directory contains:
    - spec.md
    - tasks.json
    - expected_artifacts.json
    - grader.md
    - receipts.jsonl or path reference
    - summary.json
- 
- Writes must be atomic where feasible (write temp then rename)
- Never overwrite prior runs unless run_id differs or user requests overwrite

Logging and error policy

- No stack traces for expected model-format failures
- Stack traces allowed for programmer errors only, and must be paired with a receipt event
- Any exception that escapes a lesson must be caught at the course level and recorded, then continue

Security and safety rules

- scope guard must be available as an optional op
- forbidden prompt substrings list maintained in one place
- environment variable allowlist for runtime behavior
- do not execute generated shell commands automatically unless explicitly enabled

Quality gates for merge

A change is acceptable only if:

- python run.py completes at least a small run (LMU_LESSONS=1) in smoke mode
- receipts and summary artifacts are produced
- malformed JSON from model does not crash the run (simulated test)

Style rules

- keep prompts centralized in prompts.py
- keep parsing centralized in extract.py
- keep validation centralized in validators.py
- keep receipts centralized in receipts.py
- no inline magic strings for schema keys

End of EXECPOLICY.md
