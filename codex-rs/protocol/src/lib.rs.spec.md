## Overview
`protocol::lib` exposes the public surface of the `codex-protocol` crate. It re-exports the modules that define account metadata, configuration enums, conversation identifiers, protocol messages, and supporting utilities so downstream crates can depend on a single import path.

## Detailed Behavior
- Makes `account`, `config_types`, `custom_prompts`, `items`, `message_history`, `models`, `num_format`, `parse_command`, `plan_tool`, `protocol`, and `user_input` public, establishing them as part of the crate’s stable API.
- Wraps `conversation_id` as an internal module while re-exporting `ConversationId`, hiding implementation details (UUID handling) yet providing consumers with the strongly typed ID.
- Contains no additional logic; all semantics live in the referenced modules.

## Broader Context
- Serves as the canonical import point for Codex binaries and services, letting them pick specific protocol areas without referencing internal paths directly.
- Because the module list defines the public API, adding/removing entries must be coordinated with client updates and TypeScript schema generation.
- Context can't yet be determined for future feature gating or versioned modules; revisit if the crate adopts optional protocol subsets.

## Technical Debt
- None observed; the file is a minimal routing layer over the crate’s modules.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./protocol.rs.spec.md
  - ./config_types.rs.spec.md
  - ./models.rs.spec.md
  - ./items.rs.spec.md
  - ./plan_tool.rs.spec.md
  - ./user_input.rs.spec.md
  - ./custom_prompts.rs.spec.md
  - ./message_history.rs.spec.md
  - ./parse_command.rs.spec.md
  - ./account.rs.spec.md
  - ./conversation_id.rs.spec.md
  - ./num_format.rs.spec.md
