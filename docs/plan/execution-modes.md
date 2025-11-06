# Execution Modes - Detailed Guide

**Version**: 0.57.0

---

## Overview

plan mode supports 3 execution strategies, each optimized for different use cases.

| Mode | Use Case | Speed | Quality | Complexity |
|------|----------|-------|---------|------------|
| **Single** | Simple edits | ⚡⚡⚡ | ⭐⭐ | Low |
| **Orchestrated** | Complex features | ⚡⚡ | ⭐⭐⭐ | Medium |
| **Competition** | Optimization | ⚡ | ⭐⭐⭐⭐ | High |

---

## Single Mode

### When to Use

- ✅ Single-file changes
- ✅ Typo fixes, docstring additions
- ✅ Simple refactoring (rename variable)
- ✅ Quick iterations

### Architecture

```
Plan (approved)
    ↓
Single LLM Agent
    ↓
File edits + tests
    ↓
Done
```

### Example

**Task**: Add docstring to a function

```bash
codex /Plan "Add docstring to calculate_total()" --mode=single
```

**Execution**:
1. Single agent reads function
2. Generates docstring
3. Applies patch
4. Runs linter
5. Done in ~30 seconds

---

## Orchestrated Mode (Default)

### When to Use

- ✅ Multi-file features
- ✅ Full-stack changes (backend + frontend)
- ✅ Database migrations
- ✅ Security-sensitive code
- ✅ Test generation required

### Architecture

```
Plan (approved)
    ↓
Central Planner
    ├── Task DAG
    └── Sub-Agent Assignments
        ↓
Parallel Execution
├─ Backend Agent    → API changes
├─ Frontend Agent   → UI updates
├─ Database Agent   → Schema migrations
├─ Security Agent   → Vulnerability review
└─ QA Agent         → Test generation
    ↓
Integrator
├─ Collect patches
├─ Run linters/tests
└─ Prepare PR
    ↓
Done
```

### Example

**Task**: Migrate session auth to JWT

```bash
codex /Plan "Refactor auth to JWT" --mode=orchestrated
```

**Execution Flow**:

1. **Planner** analyzes Plan
   - Identifies 5 work items (JWT service, DB migration, API endpoints, frontend client, tests)
   - Assigns to specialist agents

2. **Parallel Execution**:
   - Backend Agent: `auth/jwt_service.py` (new file)
   - Database Agent: `migrations/002_add_refresh_tokens.sql`
   - Security Agent: Reviews token security
   - Frontend Agent: `frontend/src/api/client.ts` (patch)
   - QA Agent: Generates integration tests

3. **Integration**:
   - Collects 5 patches
   - Runs `pytest`, `npm test`
   - All tests pass → Prepare PR

4. **Done** in ~5 minutes

### Agents

| Agent | Specialty | Files |
|-------|-----------|-------|
| Backend | Python, Rust, Go, Node.js | `src/`, `api/` |
| Frontend | React, Vue, Svelte, TypeScript | `frontend/`, `ui/` |
| Database | SQL, migrations, schema | `migrations/`, `db/` |
| Security | Vulnerability scanning | All files |
| QA | Test generation | `tests/` |

---

## Competition Mode

### When to Use

- ✅ Performance optimization
- ✅ Algorithm selection
- ✅ Multiple valid approaches
- ✅ Need empirical comparison

### Architecture

```
Plan (approved)
    ↓
Spawn Worktrees (A, B, C)
    ↓
Parallel Execution
├─ Variant A → git worktree A
├─ Variant B → git worktree B
└─ Variant C → git worktree C
    ↓
Run Tests & Benchmarks
├─ pytest (pass/fail)
├─ Benchmark suite (latency)
└─ LOC analysis (simplicity)
    ↓
Compute Scores
Score = 0.5×tests + 0.3×perf + 0.2×simplicity
    ↓
Present Comparison Table
    ↓
Auto-Merge Winner
    ↓
Archive Losers
    ↓
Done
```

### Example

**Task**: Optimize slow DB query (currently 800ms → target <100ms)

```bash
codex /Plan "Optimize user listing query" --mode=competition
```

**Execution Flow**:

1. **Variant Creation**:
   - Variant A: Composite index + pagination
   - Variant B: Materialized view + caching
   - Variant C: Partial index + query rewrite

2. **Parallel Execution** (in separate worktrees):
   ```
   .codex/worktrees/
   ├── A/  (branch: Plan-competition-A)
   ├── B/  (branch: Plan-competition-B)
   └── C/  (branch: Plan-competition-C)
   ```

3. **Testing & Benchmarking**:
   - Run `pytest tests/performance/` in each worktree
   - Execute load test (1000 concurrent requests)
   - Measure p50/p95 latency

4. **Scoring**:
   ```
   Variant A:
   - Tests: 100.0 (all pass)
   - Performance: 95.2 (p95: 48ms)
   - Simplicity: 92.0 (clean index, minimal code)
   - Total: 95.6 (weighted)
   
   Variant B:
   - Tests: 100.0
   - Performance: 98.5 (p95: 35ms) ← fastest
   - Simplicity: 75.0 (complex materialized view refresh)
   - Total: 92.2
   
   Variant C:
   - Tests: 100.0
   - Performance: 88.0 (p95: 75ms)
   - Simplicity: 95.0 (simple partial index)
   - Total: 92.6
   ```

5. **Winner**: Variant A (best balance)

6. **Merge**:
   ```bash
   git checkout main
   git merge Plan-competition-A --no-ff
   ```

7. **Archive Losers**:
   - Branch renamed: `archived-Plan-competition-B`
   - Worktrees removed

8. **Webhook Notification**:
   ```json
   {
     "Plan_id": "bp-123",
     "state": "approved",
     "summary": "Competition completed - Variant A won",
     "score": {
       "variant": "A",
       "total": 95.6,
       "is_winner": true
     }
   }
   ```

---

## Scoring Details

### Weights (Configurable)

```json
{
  "codex.competition.weights": {
    "tests": 0.5,       // 50%
    "performance": 0.3, // 30%
    "simplicity": 0.2   // 20%
  }
}
```

### Test Score

```
Score = (Tests Passed / Total Tests) × 100
```

If ANY test fails → Score = 0 (variant disqualified)

### Performance Score

Measured by benchmark suite. Common metrics:
- API latency (p50, p95, p99)
- Throughput (requests/sec)
- Memory usage
- Bundle size (frontend)

### Simplicity Score

Heuristics:
- Lines of code changed (fewer is better)
- Cyclomatic complexity
- Number of dependencies added
- Code maintainability index

---

## Switching Modes

### At Plan Creation

```bash
codex /Plan "Task" --mode=orchestrated
```

### After Creation

```bash
# Set global default
codex /mode competition

# All future Plans use competition mode
```

### Per-Plan Override

Modify Plan before approval:

```yaml
mode: orchestrated  # Change to: competition
```

---

## Comparison Matrix

| Feature | Single | Orchestrated | Competition |
|---------|--------|--------------|-------------|
| Speed | Fastest | Medium | Slowest |
| Quality | Basic | High | Highest |
| Sub-agents | 0 | 2-5 | 0 (multi-variant) |
| Parallelism | No | Yes | Yes (variants) |
| Test coverage | Basic | Comprehensive | All variants |
| Cost (tokens) | Low | Medium | High |
| Best for | Simple edits | Features | Optimization |

---

## Best Practices

### Use Single Mode When

- Single file, < 50 lines changed
- Typo fixes, comments, docstrings
- No tests required
- Quick iteration needed

### Use Orchestrated Mode When

- Multiple files (≥ 3)
- Full-stack changes
- Database schema involved
- Security-sensitive
- Comprehensive tests needed

### Use Competition Mode When

- Performance is critical
- Multiple valid approaches exist
- Need empirical comparison
- Budget allows (3x tokens vs. orchestrated)

---

## Examples

See `docs/Plans/samples/`:
- `simple-feature.md` (single mode)
- `orchestrated-refactor.md` (orchestrated mode)
- `competition-optimization.md` (competition mode)

---

**Made with ❤️ by zapabob**

