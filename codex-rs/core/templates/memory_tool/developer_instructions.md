## Memory

You have a memory folder with guidance from prior runs. This is high priority.
Use it before repo inspection or other tool calls unless the task is truly trivial and irrelevant to the memory summary.
Treat memory as guidance, not truth. The current tools, code, and environment are the source of truth.

Memory layout (general -> specific):
- {{ base_path }}/memory_summary.md (already provided below; do NOT open again)
- {{ base_path }}/MEMORY.md (searchable registry; primary file to query)
- {{ base_path }}/skills/<skill-name>/ (skill folder)
  - SKILL.md (entrypoint instructions)
  - scripts/ (optional helper scripts)
  - examples/ (optional example outputs)
  - templates/ (optional templates)
- {{ base_path }}/rollout_summaries/ (per-rollout recaps + evidence snippets)

Mandatory startup protocol (for any non-trivial and related task):
1) Skim MEMORY_SUMMARY in this prompt and extract some relevant keywords that are relevant to the user task
   (e.g. repo name, component, error strings, tool names).
2) Search MEMORY.md for those keywords and for any referenced rollout ids or summary files.
3) If a **Related skills** pointer appears, open the skill folder:
   - Read {{ base_path }}/skills/<skill-name>/SKILL.md first.
   - Only open supporting files (scripts/examples/templates) if SKILL.md references them.
4) If you find relevant rollout summary files, open the matching files.
5) If nothing relevant is found, proceed without using memory.

Example for how to search memory (use shell tool):
* Search notes example (fast + line numbers):
`rg -n -i "<pattern>" "{{ base_path }}/MEMORY.md"`

* Search across memory (notes + skills + rollout summaries):
`rg -n -i "<pattern>" "{{ base_path }}" | head -n 50`

* Open a rollout summary example (find by rollout_id, then read a slice):
`rg --files "{{ base_path }}/rollout_summaries" | rg "<rollout_id>"`
`sed -n '<START>,<END>p' "{{ base_path }}/rollout_summaries/<file>"`
(Common slices: `sed -n '1,200p' ...` or `sed -n '200,400p' ...`)

* Open a skill entrypoint (read a slice):
`sed -n '<START>,<END>p' "{{ base_path }}/skills/<skill-name>/SKILL.md"`
* If SKILL.md references supporting files, open them directly by path.

During execution: if you hit repeated errors or confusion, return to memory and check MEMORY.md/skills/rollout_summaries again.
If you found stale or contradicting guidance with the current environment, update the memory files accordingly.

========= MEMORY_SUMMARY BEGINS =========
{{ memory_summary }}
========= MEMORY_SUMMARY ENDS =========

Begin with the memory protocol.
