## Overview
`core::custom_prompts` discovers user-authored prompt templates and parses optional frontmatter so Codex can surface them in slash commands. It exposes helpers to locate the default prompts directory, list available prompts, and exclude built-ins.

## Detailed Behavior
- `default_prompts_dir` resolves `$CODEX_HOME/prompts` using `config::find_codex_home`, returning `None` when Codex home is not set.
- `discover_prompts_in` and `discover_prompts_in_excluding` asynchronously traverse a directory:
  - Filter entries to Markdown files (`*.md`) and skip non-files.
  - Ignore names listed in the exclusion set (for built-in prompts already bundled in the binary).
  - Load file content, parse optional frontmatter via `parse_frontmatter`, and return `CustomPrompt { name, path, content, description, argument_hint }`.
  - Sort the resulting prompts alphabetically by name.
- `parse_frontmatter` looks for `---` delimited YAML-like metadata and extracts `description` plus `argument-hint`/`argument_hint` values, handling quotes and comments. Unterminated frontmatter falls back to the original body.

## Broader Context
- Slash-command tooling loads prompts via these helpers to provide user-friendly descriptions and argument hints. Workspace-specific prompts sit alongside config overrides, so keeping discovery resilient is critical for UX.
- Prompt content feeds directly into model instructions, so the parser must preserve newlines and UTF-8 data; tests confirm handling of bad encodings and ordering.
- Context can't yet be determined for additional metadata (e.g., categories, permissions); expanding frontmatter should remain backwards compatible with existing fields.

## Technical Debt
- Discovery reads files serially; large prompt directories might benefit from concurrent reads or caching, though the current workload is typically small.
- Frontmatter parsing is bespoke; using a YAML parser would improve robustness for more complex metadata, albeit at the cost of strict error handling.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Consider swapping the handwritten frontmatter parser for a tolerant YAML parser when prompt metadata evolves beyond simple key/value pairs.
related_specs:
  - ./user_instructions.rs.spec.md
  - ../mod.spec.md
