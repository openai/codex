# Celaya Solutions

**Organization**: Celaya Solutions
**Project**: LMU Curriculum Runtime
**Version**: 0.1.0
**Status**: Experimental

---

## Mission

Transform inference into a programmable execution model.

Celaya Solutions builds fault-tolerant curriculum systems that treat model output as untrusted input, enforce deterministic runtime behavior, and produce verifiable artifacts.

No hype. No AGI claims. Runnable, testable, artifact-producing systems.

---

## LMU: Language Model Unit

**Definition**: A programmable inference execution model.

LMU treats language model inference as a structured computational primitive, analogous to GPU kernel execution. Model outputs are untrusted inputs that must be defensively parsed, validated, and isolated.

### Core Principles

1. **Local-first execution** - Default to Ollama, no cloud dependencies
2. **Deterministic runtime** - Reproducible behavior, explicit state transitions
3. **Defensive parsing** - All model output validated against schemas
4. **No crashes** - Malformed generations never halt the pipeline
5. **Receipts and summaries** - Every operation logged to JSONL
6. **Partial success > total failure** - Continue on errors, track degradation
7. **Explicit execution model** - Direct analogy to CUDA/GPU programming

---

## CUDA Analogy

LMU inference maps directly to GPU execution concepts:

| CUDA Concept | LMU Equivalent | Purpose |
|--------------|----------------|---------|
| **CUDA kernel** | LMU operation | Single inference call with defined I/O |
| **Kernel launch** | LMU runner | Orchestrates operation execution |
| **Warp** | LMU lane | Parallel inference lanes |
| **SRAM/HBM** | KV cache | Fast memory for attention states |
| **nvprof** | CORA receipts | Runtime profiling and observability |
| **CUDA stream** | LMU pipeline | Sequential operation chains |
| **Grid** | Curriculum | Multiple parallel learning paths |

### Why This Matters

Just as CUDA made GPUs programmable for general computation, LMU makes language models programmable for structured workflows.

- **Predictable performance**: Inference is an operation with measurable latency
- **Resource management**: Explicit control over context, memory, retries
- **Fault isolation**: One operation failure doesn't crash the grid
- **Observability**: Every operation emits timing and success metrics

---

## LMU Curriculum Runtime

This repository implements a curriculum engine that:

1. **Generates** structured learning modules from syllabus definitions
2. **Executes** LMU operations against local models (Ollama)
3. **Validates** all outputs against JSON schemas
4. **Grades** completions with partial credit scoring
5. **Emits** JSONL receipts for every meaningful action
6. **Survives** malformed outputs without crashing

### Not Included

- Training or fine-tuning (inference only)
- Cloud dependencies (local-first)
- Assumptions of model correctness (defensive parsing)
- Hidden failures (explicit receipts)
- Premature optimization (correctness first)

---

## Architecture

```
celaya/lmu/
├── syllabus/          # Curriculum definition
│   ├── syllabus.yaml  # Lesson order, dependencies
│   └── phases.md      # Human-readable explanation
├── generator/         # Lesson generation
│   ├── pipeline.py    # Orchestration
│   ├── prompts.py     # All prompt templates
│   ├── extract.py     # Defensive JSON extraction
│   └── validators.py  # Schema validation
├── runtime/           # Execution engine
│   ├── receipts.py    # JSONL event writer
│   └── runner.py      # Named operations, timing
├── grading/           # Scoring system
│   ├── weights.json   # Per-lesson artifact weights
│   └── grader.py      # Partial credit calculation
└── artifacts/         # Generated outputs
    └── .gitkeep
```

---

## Usage

```bash
# Verify Ollama connectivity
python run.py --check

# Generate full curriculum
python run.py

# Generate specific lessons
python run.py --lessons 0.1,0.2,1.0

# Review receipts
cat celaya/lmu/artifacts/receipts.jsonl | jq .

# Check grading summary
cat celaya/lmu/artifacts/generation_summary.json
```

---

## Receipt Format

Every meaningful action emits a JSONL event:

```jsonline
{"event":"lesson_start","lesson":"0.1","timestamp":"2026-01-02T18:54:00Z"}
{"event":"op_start","op":"generate_spec","lesson":"0.1","timestamp":"2026-01-02T18:54:01Z"}
{"event":"op_done","op":"generate_spec","lesson":"0.1","duration_ms":1234,"status":"success"}
{"event":"attempt_fail","op":"validate_json","lesson":"0.1","attempt":1,"reason":"invalid_json"}
{"event":"attempt_success","op":"validate_json","lesson":"0.1","attempt":2}
{"event":"lesson_complete","lesson":"0.1","artifacts":["spec.md","tasks.json"]}
{"event":"run_complete","lessons_passed":4,"lessons_failed":1,"total_duration_ms":45678}
```

---

## Success Criteria

A new user on Apple Silicon can:

1. Clone the repository
2. Run `python run.py`
3. Observe a fully generated curriculum
4. See receipts and grading output
5. Verify partial fault tolerance (some failures tolerated)

No crashes. No hidden state. Complete observability.

---

## Version History

### 0.1.0 (2026-01-02)
- Initial LMU curriculum runtime structure
- CUDA analogy documentation
- Core directory layout
- Receipt format specification
