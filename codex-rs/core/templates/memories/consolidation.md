## Memory Phase 2 (Consolidation)
Consolidate Codex memories in: {{ memory_root }}

Primary inputs in this directory:
- `rollout_summaries/`: per-thread summaries from Phase 1.
- `raw_memories.md`: merged Stage-1 raw memories (equivalent to a generated `raw_memory_merged.md` input artifact).
- Existing outputs if present:
  - `MEMORY.md`
  - `memory_summary.md`
  - `skills/*`

Operating mode:
- `INIT`: outputs are missing or nearly empty.
- `INCREMENTAL`: outputs already exist; integrate new signal with minimal churn.

Rules:
- Prefer targeted updates over rewrites.
- No-op is allowed when there is no meaningful net-new signal.
- Treat phase-1 artifacts as immutable evidence.
- Deduplicate aggressively and remove generic/filler guidance.
- Keep only reusable, high-signal knowledge: first steps, failure shields, concrete commands/paths/errors, verification checks, and stop rules.
- Resolve conflicts explicitly:
  - prefer newer guidance by default;
  - if older guidance is better-evidenced, keep both with a brief verification note.

Workflow (order matters):
1. Read `rollout_summaries/` for routing, then cross-check details in `raw_memories.md`.
2. Read existing `MEMORY.md`, `memory_summary.md`, and `skills/` if they exist.
3. Update `skills/` only when a reusable, reliable procedure clearly exists.
4. Update `MEMORY.md` as the durable registry; add clear pointers to relevant skills in note bodies when useful.
5. Write `memory_summary.md` last as a compact routing layer.
6. Optional housekeeping: remove duplicate or low-signal rollout summaries when clearly redundant.

Expected outputs (create/update as needed):
- `MEMORY.md`
- `memory_summary.md`
- `skills/<skill-name>/...` (optional)
