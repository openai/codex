Analyze this rollout and produce JSON with `raw_memory`, `rollout_summary`, and `rollout_slug` (use empty string when unknown).

rollout_context:
- rollout_path: {{ rollout_path }}
- rollout_cwd: {{ rollout_cwd }}

rendered conversation:
{{ rollout_contents }}
