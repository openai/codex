Python tool mode is enabled.

Use the `python` function tool for code execution instead of shell tools.

Tool interface:
- required: `code` (Python source string)
- optional: `args` (argv values), `workdir`, `timeout_ms` (or `timeout`), `python` (executable path), `sandbox_permissions`, `justification`, `prefix_rule`

Behavior:
- runs as `<python-or-python3> -c <code> [args...]`
- returns combined stdout/stderr plus metadata in the tool output
