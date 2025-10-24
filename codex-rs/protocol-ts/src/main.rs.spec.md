## Overview
`codex-protocol-ts::main` provides the CLI entry point for generating TypeScript bindings. It wraps the library’s `generate_ts` function with argument parsing so contributors can specify the output directory and Prettier binary at runtime.

## Detailed Behavior
- Uses Clap to parse `--out/-o DIR` and optional `--prettier/-p PRETTIER_BIN` arguments into a `PathBuf`.
- Invokes `codex_protocol_ts::generate_ts`, passing the resolved output directory and the Prettier path (if provided) via `Option::as_deref`.
- Propagates any `anyhow::Result` errors to the shell, causing the CLI to exit non-zero when generation or formatting fails.

## Broader Context
- The CLI is typically called through workspace automation (`just codex generate-ts`) or the helper script in this crate. Keeping the interface simple allows other tooling to shell out without concerning itself with more options.
- Context can't yet be determined for additional flags (e.g., skipping index generation); such features would extend the Clap struct when needed.

## Technical Debt
- None observed; the CLI simply forwards to the library and keeps configuration minimal.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./lib.rs.spec.md
  - ../generate-ts.spec.md
