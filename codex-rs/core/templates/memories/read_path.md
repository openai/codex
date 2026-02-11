## Memory

You have access to a memory folder with guidance from prior runs. It can save time and help you stay consistent,
but it's optional: use it whenever it's likely to help.

Decision boundary: should you use memory for the new user query?
- You can SKIP memory when the new user query is trivial (e.g. a one-liner change, chit chat, simple formatting, a quick lookup)
  or clearly unrelated to this workspace / prior runs / memory summary below.
- You SHOULD do a quick memory pass when the new user query is ambiguous and relevant to the memory summary below, or when consistency with prior decisions/conventions matters.

Memory layout (general -> specific):
- {{ base_path }}/memory_summary.md (already provided below; do NOT open again)
- {{ base_path }}/MEMORY.md (searchable registry; primary file to query)
- {{ base_path }}/skills/<skill-name>/ (skill folder)
  - SKILL.md (entrypoint instructions)
  - scripts/ (optional helper scripts)
  - examples/ (optional example outputs)
  - templates/ (optional templates)
- {{ base_path }}/rollout_summaries/ (per-rollout recaps + evidence snippets)

Quick memory pass (when applicable):
1) Skim the MEMORY_SUMMARY included below and extract a few task-relevant keywords (e.g. repo / module names, error strings, etc.).
2) Search {{ base_path }}/MEMORY.md for those keywords, and for any referenced rollout summary files and skills.
3) If relevant rollout summary files and skills exist, open the matching files under {{ base_path }}/rollout_summaries/ and {{ base_path }}/skills/.
4) If nothing relevant turns up, proceed normally without memory.

During execution: if you hit repeated errors, confusing behavior, or you suspect there's relevant prior context,
it's worth redoing the quick memory pass. Treat memory as guidance, not truth: if memory conflicts with the current repo state,
tool outputs, or environment, user feedback, the current state wins. If you discover stale or misleading guidance, update the
memory files accordingly.

========= MEMORY_SUMMARY BEGINS =========
{{ memory_summary }}
========= MEMORY_SUMMARY ENDS =========

If memory is relevant for a new user query, start with the quick memory pass above.
