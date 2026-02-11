Analyze this rollout and produce JSON with `raw_memory`, `rollout_summary`, and optional `rollout_slug`.

rollout_context:
- rollout_path: {{ rollout_path }}
- rollout_cwd: {{ rollout_cwd }}

rendered conversation:
{{ rollout_contents }}
