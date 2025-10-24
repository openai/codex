## Overview
`protocol::custom_prompts` defines the data model for user-supplied prompt snippets that can be injected via slash commands. It keeps prompt metadata and filesystem paths consistent across the CLI, TUI, and MCP integrations.

## Detailed Behavior
- Declares `PROMPTS_CMD_PREFIX`, the canonical prefix used when forming slash commands (e.g., `/prompts:name`). Consolidating it here prevents drift between clients.
- `CustomPrompt` stores the prompt name, on-disk `PathBuf`, raw content, and optional metadata such as a human-readable description or argument hint that UIs can surface when browsing prompts.
- Derives the serialization and schema traits needed for cross-process transport and TypeScript generation.

## Broader Context
- Consumers use this type when syncing prompt directories and presenting prompt pickers. Centralizing the path and metadata fields ensures new prompt-related capabilities automatically propagate to all clients.
- Context can't yet be determined for remote prompt repositories or sharing features; future extensions can add fields while maintaining backward compatibility through optional entries.

## Technical Debt
- None observed; the struct captures the currently supported prompt metadata without redundant logic.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
