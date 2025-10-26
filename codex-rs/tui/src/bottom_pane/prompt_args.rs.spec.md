## Overview
`prompt_args` parses and expands custom prompt invocations entered via slash commands (e.g., `/prompts:daily USER=alice`). It extracts prompt names, positional parameters, keyed arguments, and validates that required placeholders are provided.

## Detailed Behavior
- `parse_slash_name(line)` splits the first line into the command name and trailing text after `/`.
- `parse_positional_args(rest)` uses `shlex` to parse space-separated arguments with quote support.
- Placeholder detection:
  - `PROMPT_ARG_REGEX` matches `$TOKEN` placeholders.
  - `prompt_argument_names(content)` returns unique placeholder names (excluding `$ARGUMENTS`) in encounter order, ignoring escaped `$$`.
- `parse_prompt_inputs(rest)` parses `key=value` pairs, returning a map or `PromptArgsError` when assignments are malformed.
- `expand_custom_prompt(text, prompts)`:
  - Validates the command uses the `/prompts:name` prefix and that the named prompt exists.
  - Gathers required named placeholders, parses inputs, and ensures all required keys are present, returning `PromptExpansionError::MissingArgs` otherwise.
  - Handles positional placeholders (numeric `$1`, `$2`, etc.) via `expand_if_numeric_with_positional_args` and `expand_custom_prompt`.
  - Builds the expanded text, substituting placeholders and leaving unknown prompts untouched (`Ok(None)`).
- Helper functions support numeric placeholder expansion, slash-name parsing, and slash command detection.
- Error types (`PromptArgsError`, `PromptExpansionError`) provide user-friendly `user_message()` descriptions surfaced in the UI.

## Broader Context
- `ChatComposer` leverages these helpers to expand saved prompts when users invoke them, guiding autocomplete and error messaging.

## Technical Debt
- Expansion logic spans multiple functions; consolidating positional and named placeholder handling into a single pass could reduce duplication.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Simplify placeholder substitution by unifying named and positional logic.
related_specs:
  - ./chat_composer.rs.spec.md
  - ../../slash_command.rs.spec.md
