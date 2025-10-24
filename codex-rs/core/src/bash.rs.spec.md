## Overview
`core::bash` uses tree-sitter to parse simple shell scripts. It enables Codex to inspect `bash -lc` invocations, extract plain word-only commands, and determine whether a script is safe for automatic transformation (e.g., identifying sequences of simple commands).

## Detailed Behavior
- `try_parse_shell` configures a tree-sitter parser with `tree_sitter_bash` and parses the given script, returning a `Tree` or `None` on failure.
- `try_parse_word_only_commands_sequence` walks the parse tree and enforces strict constraints:
  - Only allows specific node kinds (`program`, `list`, `pipeline`, `command`, `word`, `string`, `number`, etc.).
  - Rejects punctuation tokens outside `&&`, `||`, `;`, `|`, and quote characters, disallowing redirections, subshells, control flow, or substitutions.
  - Collects `command` nodes, sorts them by source order, and converts each to a vector of words via `parse_plain_command_from_node`.
- `parse_plain_command_from_node` ensures each command consists of word/string/number tokens, handling quoted strings by stripping quotes and rejecting complex forms (e.g., embedded expressions).
- `parse_shell_lc_plain_commands` inspects `bash -lc` or `zsh -lc` invocations, uses `try_parse_shell`, and returns sequences of plain commands when all constraints pass.
- Tests cover positive and negative cases, ensuring the parser accepts sequences of simple commands and rejects unsupported constructs (parentheses, command substitution, etc.).

## Broader Context
- `tools/mod.rs` and other middleware can leverage this module to detect when commands are simple enough to rewrite or inline, aiding features such as command safety heuristics or plan extraction.
- Using tree-sitter provides robust parsing compared to manual tokenization, reducing false positives when deciding whether to transform scripts.
- Context can't yet be determined for more complex command analysis; current logic intentionally rejects anything beyond simple word-only commands to stay safe.

## Technical Debt
- None noted; expanding support for additional constructs would require revisiting the allowed node list and supporting transformations.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./shell.rs.spec.md
  - ./tools/handlers/shell.rs.spec.md
