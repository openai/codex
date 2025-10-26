## Overview
`exec_command` provides shell-command utilities for displaying model-generated commands in the TUI. It normalizes command arguments for presentation and derives user-friendly working directories.

## Detailed Behavior
- `escape_command(command)` uses `shlex::try_join` to join arguments into a shell-escaped string, falling back to a simple join if quoting fails.
- `strip_bash_lc_and_escape(command)` detects `bash -lc <cmd>` or `zsh -lc <cmd>` (including absolute paths to the shell) and returns the inner command directly; other commands defer to `escape_command`.
- `relativize_to_home(path)` returns `path.strip_prefix($HOME)` when possible, yielding a relative path for display; non-absolute paths or paths outside the home directory return `None`.
- `is_login_shell_with_lc` is a helper used internally to identify `bash` / `zsh` invocation patterns.

## Broader Context
- Used by bottom-pane widgets and streaming summaries to render command previews without extraneous shell wrappers, keeping output concise and user friendly.

## Technical Debt
- None identified; helper functions are small and easily extended if additional shell wrappers need special handling.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./bottom_pane/command_popup.rs.spec.md
  - ./streaming/controller.rs.spec.md
