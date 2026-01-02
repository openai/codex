# LMU Curriculum Phases

**Organization**: Celaya Solutions
**Project**: LMU Curriculum Runtime
**Version**: 0.1.0
**Generated**: 2026-01-02T18:54:00Z
**Purpose**: Human-readable explanation of curriculum phases
**Status**: Experimental

---

## Overview

The LMU curriculum is organized into 4 phases, progressing from foundational concepts to advanced optimization. Each phase builds on prior lessons with explicit dependencies.

Total lessons: 6
Total phases: 4
Estimated total duration: ~28 seconds

---

## Phase 1: Foundations

**Lessons**: 0.1, 0.2
**Focus**: Core execution model and defensive parsing

### 0.1 - Introduction to LMU Execution Model

**Objective**: Understand inference as kernel execution

**CUDA Analogy**: LMU operation → CUDA kernel

**What You'll Learn**:
- Map LMU concepts to CUDA equivalents
- Treat inference calls as deterministic operations
- Understand LMU runner as kernel launch
- Model KV cache as GPU memory hierarchy

**Success Criteria**:
- Correctly map all 6 LMU/CUDA concept pairs
- Generate valid spec.md with CUDA analogy
- Produce tasks.json with explicit task weights

**Constraints**:
- No speculative language about AGI
- Explicit, testable success criteria only
- Must include CUDA analogy or fail validation

---

### 0.2 - Defensive JSON Extraction

**Objective**: Parse model output without crashing

**CUDA Analogy**: Defensive parsing → Error checking after kernel launch

**What You'll Learn**:
- Extract JSON from markdown-wrapped responses
- Handle truncated generation gracefully
- Sanitize malformed commas and brackets
- Never raise uncaught exceptions on parse failure

**Success Criteria**:
- Parser survives 10/10 malformed inputs
- Extracts valid JSON from markdown fences
- Returns partial data on truncation
- Logs failures to receipts, continues pipeline

**Constraints**:
- No assumptions of correct model output
- No crashes on malformed JSON
- Partial extraction preferred over total failure

---

## Phase 2: Runtime

**Lessons**: 1.0, 1.1
**Focus**: Operational reliability and observability

### 1.0 - Receipt-Driven Observability

**Objective**: Emit JSONL events for every operation

**CUDA Analogy**: Receipts → nvprof profiling output

**What You'll Learn**:
- Emit structured events (lesson_start, op_done, etc.)
- Log timing, retries, cache hits to JSONL
- Build audit trail for curriculum execution
- Use receipts as source of truth for debugging

**Success Criteria**:
- Every operation emits at least 2 events (start/done)
- Receipts include timestamp, duration, status
- JSONL format valid for stream processing
- Receipts file survives crashes (append-only)

**Constraints**:
- No silent operations
- All events must have UTC timestamps
- Receipts never deleted, only appended

---

### 1.1 - Retry with Constraint Tightening

**Objective**: Modify constraints on retry, not just repeat

**CUDA Analogy**: Retry logic → Kernel re-launch with different parameters

**What You'll Learn**:
- Implement exponential backoff with constraint changes
- Tighten schema requirements on each attempt
- Reduce max tokens, simplify prompts on failure
- Track retry count in receipts

**Success Criteria**:
- Retries use different constraints each attempt
- Max 3 retries before marking operation failed
- Each retry logged with attempt number
- Final failure recorded with all attempt details

**Constraints**:
- Never infinite retry loops
- Constraints must tighten (not loosen) on retry
- Prompt must change between attempts

---

## Phase 3: Evaluation

**Lessons**: 2.0
**Focus**: Scoring and success measurement

### 2.0 - Partial Credit Grading

**Objective**: Score artifacts with weights, allow partial success

**CUDA Analogy**: Grading → Performance metrics (throughput, latency)

**What You'll Learn**:
- Assign weights to artifacts (spec.md=0.3, tasks.json=0.2, etc.)
- Calculate partial credit if some artifacts missing
- Generate pass/fail threshold (70% = pass)
- Emit summary.json with scores and metrics

**Success Criteria**:
- Grader assigns weight to each expected artifact
- Partial success possible (3/5 artifacts = 60%)
- Summary includes pass_rate, retry_count, latency_p95
- Summary.json valid against schema

**Constraints**:
- No binary pass/fail (partial credit required)
- Weights must sum to 1.0
- Missing artifacts scored as 0, not errors

---

## Phase 4: Optimization

**Lessons**: advanced.1
**Focus**: Performance and parallelism

### advanced.1 - Parallel Lesson Execution

**Objective**: Run independent lessons concurrently

**CUDA Analogy**: Parallel execution → CUDA streams, concurrent kernels

**What You'll Learn**:
- Identify lessons with no dependencies
- Launch parallel Python processes for independent lessons
- Manage Ollama connection pool
- Aggregate receipts from parallel streams

**Success Criteria**:
- Lessons 0.1 and 0.2 can't run in parallel (dependency)
- Multiple independent lessons run concurrently
- Total wall-clock time < sum of individual durations
- Receipts correctly interleaved from parallel streams

**Constraints**:
- Respect dependency graph (syllabus.yaml)
- No race conditions in receipt writes
- Handle partial failures in parallel execution
- Resource limits prevent Ollama overload

---

## Dependency Graph

```
0.1 (Foundations)
 └─> 0.2 (Defensive Parsing)
      └─> 1.0 (Receipts)
           └─> 1.1 (Retry Logic)
                └─> 2.0 (Grading)
                     └─> advanced.1 (Parallel Execution)
```

**Critical Path**: 0.1 → 0.2 → 1.0 → 1.1 → 2.0 → advanced.1

**Parallel Opportunities**: None in this linear curriculum (future: add branching)

---

## Success Metrics

### Per-Lesson Metrics
- **Pass rate**: Percentage of artifacts successfully generated
- **Retry count**: Number of attempts before success
- **Latency**: p50, p95, p99 operation duration

### Global Metrics
- **Curriculum pass rate**: Lessons passed / Total lessons
- **Total duration**: Wall-clock time for full run
- **Fault tolerance**: Successful completions despite errors

### Minimum Thresholds
- Curriculum pass rate ≥ 70%
- At least 5 artifacts per lesson
- Receipts and summary.json always generated
- No uncaught exceptions (crashes fail the run)

---

## Usage

```bash
# Run full curriculum in sequence
python run.py

# Run specific phase
python run.py --phase foundations

# Run single lesson
python run.py --lessons 0.1

# Skip lesson on failure (fault tolerance)
python run.py --continue-on-error

# Review phase completion
cat celaya/lmu/artifacts/receipts.jsonl | jq 'select(.phase == "foundations")'
```

---

## Curriculum Evolution

### Future Phases (Planned)
- **Phase 5: Branching**: Multiple learning paths, user choice
- **Phase 6: Caching**: KV cache reuse across lessons
- **Phase 7: Streaming**: Real-time receipt updates

### Not Planned
- Training or fine-tuning (out of scope)
- Cloud deployment (local-first commitment)
- GUI interfaces (CLI-first)

---

## References

- Syllabus definition: `syllabus.yaml`
- CUDA analogy: `../CELAYA.md`
- Receipt format: `../CELAYA.md#receipt-format`
- Success criteria: `syllabus.yaml#success_criteria`
