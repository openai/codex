## Overview
`protocol::parse_command` declares the structured representation of shell commands parsed from model output. It lets downstream tooling reason about command intent (read/list/search/unknown) without inspecting raw strings.

## Detailed Behavior
- `ParsedCommand` is a tagged enum with variants for `Read`, `ListFiles`, `Search`, and `Unknown`. Each variant records the original command string and variant-specific metadata (`name`, `path`, `query`).
- The `Read` variant stores a best-effort absolute `PathBuf`, enabling audit tools to determine which files the agent intends to inspect.
- Derives serialization and schema traits so the enum can travel across the protocol boundary and drive analytics or approval UIs.
- The enum is marked `PartialEq`/`Eq` to support direct comparison in tests or higher-level logic.

## Broader Context
- Produced by command parsing logic in `codex-core` and consumed by review and approval flows when assessing model activity. Aligning fields with CLI expectations ensures consistent messaging.
- The enum currently covers the most common command types; expanding the parser will require adding new variants here.
- Context can't yet be determined for write or network command classifications; those would necessitate additional variants and metadata.

## Technical Debt
- None observed; the enum mirrors current parser capabilities without lingering TODOs.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ./protocol.rs.spec.md
