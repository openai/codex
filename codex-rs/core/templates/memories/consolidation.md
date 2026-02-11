## Memory Phase 2 (Consolidation)
Consolidate Codex memories in: {{ memory_root }}

You are in Phase 2 (Consolidation / cleanup pass).
Integrate Phase 1 artifacts into a stable, retrieval-friendly memory hierarchy with minimal churn.

Primary inputs in this directory:
- `rollout_summaries/` (per-thread summaries from Phase 1)
- `raw_memories.md` (merged Stage 1 raw memories; latest first)
- Existing outputs if present:
  - `MEMORY.md`
  - `memory_summary.md`
  - `skills/*`

Operating mode:
- `INIT`: outputs are missing or nearly empty.
- `INCREMENTAL`: outputs already exist; integrate net-new signal without unnecessary rewrites.

Core rules (strict):
- Treat Phase 1 artifacts as immutable evidence.
- Prefer targeted edits over broad rewrites.
- No-op is valid when there is no meaningful net-new signal.
- Deduplicate aggressively and remove generic/filler guidance.
- Keep only reusable, high-signal memory:
  - decision triggers and efficient first steps
  - failure shields (`symptom -> cause -> fix/mitigation`)
  - concrete commands/paths/errors/contracts
  - verification checks and stop rules
- Resolve conflicts explicitly:
  - prefer newer guidance by default
  - if older guidance is better-evidenced, keep both with a brief verification note
- Keep clustering light:
  - cluster only strongly related tasks
  - avoid large, weakly related mega-clusters

Expected outputs (create/update only these):
- `MEMORY.md`
- `memory_summary.md`
- `skills/<skill-name>/...` (optional, when a reusable procedure is clearly warranted)

Workflow (order matters):
1. Determine mode (`INIT` vs `INCREMENTAL`) from artifact availability/content.
2. Read `rollout_summaries/` first for routing, then validate details in `raw_memories.md`.
3. Read existing `MEMORY.md`, `memory_summary.md`, and `skills/` for continuity.
4. Update `skills/` only for reliable, repeatable procedures with clear verification.
5. Update `MEMORY.md` as the durable registry; add clear related-skill pointers in note bodies when useful.
6. Write `memory_summary.md` last as a compact, high-signal routing layer.
7. Optional housekeeping:
  - remove duplicate or low-signal rollout summaries when clearly redundant
  - keep one best summary per thread when duplicates exist
8. Final consistency pass:
  - remove cross-file duplication
  - ensure referenced skills exist
  - keep output concise and retrieval-friendly
