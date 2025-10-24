## Overview
`core::command_safety::is_safe_command` determines whether a command can be auto-approved without user intervention. It recognises a conservative set of read-only utilities, validates certain flag combinations, and understands `bash -lc`/`zsh -lc` wrappers by parsing their scripts with tree-sitter.

## Detailed Behavior
- Normalises `zsh` invocations to `bash` to share logic across shells.
- On Windows, delegates to `windows_safe_commands::is_safe_command_windows` before continuing.
- `is_safe_to_call_with_exec` implements the core safelist:
  - Allows specific binaries (`ls`, `grep`, `head`, `git status`, etc.).
  - Rejects dangerous flags (`find -exec`, ripgrep `--pre`, etc.).
  - Supports `sed -n {N|M,N}p` via `is_valid_sed_n_arg`.
- For `bash -lc`/`zsh -lc`, uses `parse_shell_lc_plain_commands` (tree-sitter) to ensure the script consists solely of plain commands combined with `&&`, `||`, `;`, or `|`. Every constituent command must pass `is_safe_to_call_with_exec`.
- The module includes comprehensive tests for known-safe commands, unsafe edge cases, and bash script sequences, ensuring regressions are caught early.

## Broader Context
- The tool orchestrator consults this module when deciding whether to auto-run shell commands or prompt for approval. Keeping the safelist conservative minimises risk while preserving common read-only queries.
- The tree-sitter dependency (`crate::bash`) enables safe evaluation of shell scripts without resorting to fragile string heuristics.
- Context can't yet be determined for expanding the safelist; additions should be cautious and always paired with tests.

## Technical Debt
- TODOs elsewhere (e.g., PowerShell escaping) may affect future safelist expansions, but no debt noted in this file.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./is_dangerous_command.rs.spec.md
  - ./windows_safe_commands.rs.spec.md
  - ../bash.rs.spec.md
