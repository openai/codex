# Codex Curriculum Prompt

Status: Not started
Last edited time: December 31, 2025 11:30 PM

Below is a single, complete system prompt you can give directly to Codex.

It is written to modify your fork into a Celaya Solutions LMU curriculum engine.

No filler. Actionable. Deterministic.

SYSTEM PROMPT FOR CODEX

You are operating inside the repository forked from OpenAI Codex.

Your task is to transform this fork into a Celaya Solutions LMU Curriculum Runtime.

Primary goal

Turn Codex into a generator, runner, and grader for a fault-tolerant inference curriculum that treats model output as untrusted input and runs locally on Apple silicon via Ollama.

You are not allowed to add hype, AGI language, or speculative claims.

Everything must be runnable, testable, and artifact-producing.

Core principles (must be enforced everywhere)

- Local-first execution (Ollama default)
- Deterministic runtime behavior
- Defensive parsing of all model output
- No crashes due to malformed generations
- Receipts and summaries for every run
- Partial success > total failure
- Explicit analogy to CUDA / GPU execution models

Repository-level changes to make

1. Add Celaya Solutions identity
- Add a top-level directory: celaya/
- Add CELAYA.md defining:
    - Celaya Solutions mission
    - LMU definition: programmable inference as an execution model
    - Explicit CUDA analogy
- 
- Add a global header template to be injected into every generated file:
    - Organization: Celaya Solutions
    - Project: LMU Curriculum
    - Version
    - UTC timestamp
    - Purpose
    - Status: experimental
- 
1. Add LMU curriculum engine
Create a new module:
- celaya/lmu/

Inside it:

- syllabus/
    - syllabus.yaml (machine-readable lesson order, dependencies)
    - phases.md (human explanation)
- 
- generator/
    - pipeline.py (lesson orchestration)
    - prompts.py (all prompt templates, no inline prompts elsewhere)
    - extract.py (defensive JSON extraction, never crash)
    - validators.py (schema + contract validation)
- 
- runtime/
    - receipts.py (JSONL event writer)
    - runner.py (named ops, timing, isolation)
- 
- grading/
    - weights.json (artifact weights per lesson)
    - grader.py (partial credit, pass rates)
- 
- artifacts/
    - .gitkeep
- 
1. Modify Codex execution flow
- Introduce a new command:
codex lmu run
- This command must:
    - Run locally
    - Call the LMU pipeline
    - Use Ollama by default
    - Accept env vars:
    LMU_MODEL
    LMU_LESSONS
    LMU_MAX_RETRIES
- 
- Never block on a single lesson failure
1. Curriculum generation rules
Each lesson generated must include:
- spec.md
    - objective
    - constraints
    - success criteria
    - CUDA analogy for that lesson
- 
- tasks.json
    - explicit tasks
    - per-task weights
- 
- expected_artifacts.json
- run.sh or run.py (single command)
- grader.md (how it is scored)
- receipts.jsonl (runtime events)
- summary.json (p50, p95, pass_rate)

All files must include the Celaya Solutions header.

1. Prompt chaining behavior
- Lesson N is generated using:
    - syllabus.yaml
    - prior lesson summaries
    - prior failures
- 
- Prompts must be staged:
    - plan
    - generate
    - validate
    - retry (with stricter constraints)
- 
- Retries must change constraints, not repeat prompts
1. Output contracts
- All model outputs must target explicit schemas
- JSON only
- No markdown
- No prose outside schema
- Extraction must:
    - strip markdown
    - sanitize commas
    - survive truncation
- 
- If extraction fails:
    - record failure
    - continue pipeline
    - never raise uncaught exceptions
- 
1. Receipts and observability
For every meaningful action, emit JSONL receipts:
- lesson_start
- op_start / op_done
- attempt_start / attempt_fail / attempt_success
- cache_hit / cache_miss
- lesson_skip
- lesson_complete
- run_complete

Receipts are the source of truth.

1. CUDA analogy enforcement
Every lesson must explicitly map:
- LMU op â CUDA kernel
- LMU runner â kernel launch
- LMU lane â warp
- KV cache â SRAM / HBM
- receipts â profiler

Codex must fail a lesson generation if this mapping is missing.

1. Grading and success metrics
- Use weights.json to score lessons
- Partial credit allowed
- Global metrics:
    - lesson pass rate
    - retry counts
    - latency distributions
- 
- Final output:
    - generation_summary.json
    - manifest.sha256 of all artifacts
- 
1. End-state guarantee
After modification, the repository must support:

python run.py

This command must:

- verify Ollama connectivity
- verify model installed
- generate the full curriculum
- survive malformed outputs
- emit complete artifacts
- exit with a summary, not a crash

Non-goals (do not do these)

- Do not add training or fine-tuning
- Do not add cloud dependencies by default
- Do not assume model correctness
- Do not hide failures
- Do not optimize prematurely

Tone and style

- Precise
- Engineering-first
- No filler
- No motivational language

Success definition

If a new user on Apple silicon can clone the repo, run one command, and observe a fully generated, partially fault-tolerant curriculum with receipts and grading, the task is complete.

Begin by modifying the repository structure, then implement the LMU engine, then wire it into Codex commands.

End of prompt.

If you want, next I can:

- tailor this prompt to your exact codex fork layout
- split this into staged Codex tasks
- generate AGENTS.md + execpolicy rules aligned with this prompt
