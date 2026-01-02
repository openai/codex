# Celaya Solutions LMU Curriculum Runtime

**Organization**: Celaya Solutions
**Version**: 0.1.0
**Status**: Experimental

Programmable inference execution model treating language models as structured computational primitives.

## Quick Start

```bash
# Verify Ollama is running
python run.py --check

# Generate full curriculum
python run.py

# Generate specific lessons
python run.py --lessons 0.1,0.2

# Generate specific phase
python run.py --phase foundations

# Grade curriculum
python run.py --grade
```

## What This Is

LMU (Language Model Unit) transforms inference into a programmable execution model, analogous to CUDA for GPUs.

- **Local-first**: Runs on Ollama, no cloud dependencies
- **Fault-tolerant**: Partial success preferred over total failure
- **Observable**: JSONL receipts for every operation
- **Defensive**: All model output treated as untrusted input
- **Deterministic**: Reproducible behavior, explicit state

## Architecture

```
celaya/
├── CELAYA.md              # Mission, LMU definition, CUDA analogy
└── lmu/
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
        ├── receipts.jsonl
        └── generation_summary.json
```

## CUDA Analogy

| CUDA          | LMU              |
|---------------|------------------|
| Kernel        | LMU operation    |
| Kernel launch | LMU runner       |
| Warp          | LMU lane         |
| SRAM/HBM      | KV cache         |
| nvprof        | CORA receipts    |
| Stream        | LMU pipeline     |

## Curriculum

See [syllabus/phases.md](lmu/syllabus/phases.md) for full curriculum description.

**Phases**:
1. **Foundations** (0.1-0.2): LMU execution model, defensive parsing
2. **Runtime** (1.0-1.1): Receipts, retry logic
3. **Evaluation** (2.0): Partial credit grading
4. **Optimization** (advanced.1): Parallel execution

## Receipts

Every operation emits JSONL events:

```jsonl
{"event":"lesson_start","lesson":"0.1","timestamp":"2026-01-02T18:54:00Z"}
{"event":"op_start","op":"generate_spec","lesson":"0.1"}
{"event":"op_done","op":"generate_spec","duration_ms":1234,"status":"success"}
{"event":"lesson_complete","lesson":"0.1","artifacts":["spec.md","tasks.json"]}
```

View receipts:
```bash
cat celaya/lmu/artifacts/receipts.jsonl | jq .
```

## Grading

Artifacts are weighted per lesson:
- spec.md: 25%
- tasks.json: 20%
- run.sh/run.py: 15%
- receipts.jsonl: 10%
- summary.json: 10%
- etc.

Passing threshold: 70%

Missing artifacts score 0 (not errors). Partial success allowed.

## Success Criteria

A user on Apple Silicon can:
1. Clone repository
2. Run `python run.py`
3. Observe generated curriculum
4. See receipts and grading
5. Verify fault tolerance

No crashes. Complete observability.

## Not Included

- Training/fine-tuning (inference only)
- Cloud dependencies (local-first)
- Model correctness assumptions (defensive)
- Hidden failures (explicit receipts)

## Documentation

- [CELAYA.md](CELAYA.md) - Mission, LMU definition
- [syllabus/phases.md](lmu/syllabus/phases.md) - Curriculum phases
- [syllabus/syllabus.yaml](lmu/syllabus/syllabus.yaml) - Lesson definitions

---

**Celaya Solutions** - Programmable inference execution
