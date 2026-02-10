## Memory Consolidation
Consolidate Codex memories in this directory: {{ memory_root }}

Phase-1 inputs already prepared in this same directory:
- `raw_memories/` contains per-thread raw memory markdown files.
- `memory_summary.md` contains a compact routing map from short summary -> thread id.

Consolidation goals:
1. Read `memory_summary.md` first to route quickly, then open the most relevant files in `raw_memories/`.
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
- `MEMORY.md`: merged durable memory registry for this CWD.
- `skills/<skill-name>/...`: optional skill folders when there is clear reusable procedure value.

Do not rewrite phase-1 artifacts except when adding explicit cross-references:
- keep `raw_memories/` as phase-1 output;
- keep `memory_summary.md` as the compact map generated from the latest summaries.
