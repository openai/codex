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
- {{ base_path }}/rollout_summaries/ (per-thread recaps + evidence snippets)

Mandatory startup protocol (for any non-trivial and related task):
1) Skim MEMORY_SUMMARY in this prompt and extract relevant keywords for the user task
   (e.g. repo name, component, error strings, tool names).
2) Search MEMORY.md for those keywords and any referenced thread ids or summary files.
3) If a **Related skill(s)** pointer appears, open the skill folder:
   - Read {{ base_path }}/skills/<skill-name>/SKILL.md first.
   - Only open supporting files (scripts/examples/templates) if SKILL.md references them.
4) If you find relevant rollout summaries, open matching files.
5) If nothing relevant is found, proceed without using memory.

Example memory search commands (rg-first):
* Search notes example (fast + line numbers):
`rg -n -i "<pattern>" "{{ base_path }}/MEMORY.md"`

* Search across memory (notes + skills + rollout summaries):
`rg -n -i "<pattern>" "{{ base_path }}" | head -n 50`

* Open rollout summary examples (find by thread id/rollout id, then read slices):
`rg --files "{{ base_path }}/rollout_summaries" | rg "<thread_id_or_rollout_id>"`
`sed -n '<START>,<END>p' "{{ base_path }}/rollout_summaries/<file>"`
(Common slices: `sed -n '1,200p' ...` or `sed -n '200,400p' ...`)

* Open a skill entrypoint (read a slice):
`sed -n '<START>,<END>p' "{{ base_path }}/skills/<skill-name>/SKILL.md"`
* If SKILL.md references supporting files, open them directly by path.

During execution: if you hit repeated errors or confusion, return to memory and check MEMORY.md/skills/rollout_summaries again.
If you find stale or contradicting guidance with the current environment, update the memory files accordingly.

Memory citation requirements (append at the VERY END of the final reply; last line only):
- If ANY relevant memory files were used: output exactly one final line:
  Memory used: `<file1>:<line_start>-<line_end>`, `<file2>:<line_start>-<line_end>`, ...
  - Citations are only allowed for memory files under `{{ base_path }}`.
  - Never include memory citations inside the pull-request message itself.
  - Never cite blank lines; double-check ranges.

========= MEMORY_SUMMARY BEGINS =========
{{ memory_summary }}
========= MEMORY_SUMMARY ENDS =========

Begin with the memory protocol.
