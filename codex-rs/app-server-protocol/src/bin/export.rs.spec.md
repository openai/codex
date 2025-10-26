## Overview
`app-server-protocol::bin::export` is a small CLI that regenerates TypeScript bindings and JSON Schemas for the app-server protocol.

## Detailed Behavior
- Uses `clap` to parse:
  - `--out/-o <DIR>`: required output directory.
  - `--prettier/-p <BIN>`: optional path to a Prettier executable.
- Delegates to `codex_app_server_protocol::generate_types`, passing the parsed arguments and returning any errors from the generator.

## Broader Context
- Invoked manually or in release automation to refresh protocol artifacts checked into downstream repositories.

## Technical Debt
- None; logic is intentionally minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../../mod.spec.md
  - ../export.rs.spec.md
