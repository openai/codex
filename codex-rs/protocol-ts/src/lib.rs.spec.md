## Overview
`codex_protocol_ts::lib` implements the TypeScript generation pipeline used to mirror Codex protocol types in downstream TypeScript projects. It exports `generate_ts`, which glues together `ts-rs` exports, header injection, index generation, and optional Prettier formatting.

## Detailed Behavior
- Invokes `ensure_dir` to create the output directory before writing any files.
- Uses `ts-rs` helpers to export request/response types from `codex_app_server_protocol`: it writes client-to-server (`ClientRequest`, `export_client_responses`, `ClientNotification`) and server-to-client (`ServerRequest`, `export_server_responses`, `ServerNotification`) bindings.
- `generate_index_ts` scans the output directory for `.ts` files (excluding `index.ts`), constructs re-export statements, and writes an `index.ts` with the shared header.
- `prepend_header_if_missing` ensures every generated file starts with the `// GENERATED CODE!` banner, rewriting files as needed.
- `ts_files_in` enumerates `.ts` files for header injection and Prettier formatting; it sorts the list to produce deterministic output.
- When a Prettier binary is provided, `generate_ts` invokes it with `--write` and the explicit file list, surfacing detailed errors if Prettier fails.

## Broader Context
- Consumers run this library via the CLI or scripts whenever the Rust protocol changes. Keeping header logic here ensures downstream repositories always receive consistent warnings against manual edits.
- The pipeline assumes generated files can be rewritten in place; if future workflows require incremental updates, the helper functions may need to support diff-friendly operations.
- Context can't yet be determined for bundling `.d.ts` declarations or ESM/CJS variants; those would extend the generation steps.

## Technical Debt
- None observed; the module cleanly orchestrates generation and formatting with clear error propagation.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ../mod.spec.md
  - ./main.rs.spec.md
  - ../generate-ts.spec.md
