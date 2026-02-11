## Memory Consolidation
Consolidate Codex memories in this directory: {{ memory_root }}

Phase-1 inputs already prepared in this same directory:
- `rollout_summaries/` contains per-thread rollout summary markdown files (`<thread_id>.md`).
- `raw_memories.md` contains merged raw memory content from recent stage-1 outputs.

Consolidation goals:
1. Read `rollout_summaries/` first to route quickly, then cross-check details in `raw_memories.md`.
2. Resolve conflicts explicitly:
   - prefer newer guidance by default;
   - if older guidance has stronger evidence, keep both with a verification note.
3. Extract only reusable, high-signal knowledge:
   - proven first steps;
   - failure modes and pivots;
   - concrete commands/paths/errors;
   - verification and stop rules;
   - unresolved follow-ups.
4. Deduplicate aggressively and remove generic advice.

Expected outputs for this directory (create/update as needed):
- `MEMORY.md`: merged durable memory registry for this shared memory root.
- `skills/<skill-name>/...`: optional skill folders when there is clear reusable procedure value.

Do not rewrite phase-1 artifacts except when adding explicit cross-references:
- keep `rollout_summaries/` as phase-1 output;
- keep `raw_memories.md` as the merged stage-1 raw-memory artifact.
