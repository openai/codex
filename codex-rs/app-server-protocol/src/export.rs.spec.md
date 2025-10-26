## Overview
`export` generates TypeScript bindings and JSON Schemas for every app-server protocol type so frontends and external clients can consume the same contract as the Rust implementation.

## Detailed Behavior
- `generate_types(out_dir, prettier)` runs both generators:
  - `generate_ts` exports all `TS`-derived structures (`ClientRequest`, `ServerRequest`, etc.), writes an `index.ts` barrel file, and optionally formats the output with Prettier.
  - `generate_json` walks the schema registry via `schemars`, writing `.json` schemas for every type enumerated in `for_each_schema_type!`.
- `generate_ts` ensures the output directory exists, invokes `export_all_to` for each request/notification enum, and prepends a generated file header to every `.ts` file.
- `generate_json` serializes each `JsonSchema` to disk, normalizes file names, and applies friendly renames (`pretty_schema_name`) to match TypeScript exports.
- Helper functions:
  - `ensure_dir`, `prepend_header_if_missing`, `ts_files_in`, `generate_index_ts` manage filesystem I/O.
  - `write_json_schema` wraps `schema_for!` with deterministic ordering and pretty-printing.
  - `run_prettier` shells out to a provided Prettier binary with `--write`.
  - `pretty_schema_name`, `literal_from_property`, and `to_pascal_case` derive human-friendly schema names from JSON schema contents, especially for sandbox/safety enums.
- Validation utilities (`read_ts_to_json_value`, `compare_json_files`) ensure newly generated JSON matches existing content unless rewrites are necessary.

## Broader Context
- The CLI (`src/bin/export.rs`) invokes `generate_types` during release builds, and the resulting artifacts are consumed by desktop/web clients and internal tools.

## Technical Debt
- Custom schema renaming logic mirrors TypeScript naming heuristics; updates to protocol enums may require adjusting `pretty_schema_name` to avoid awkward identifiers.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Expand schema name heuristics to avoid manual tweaks when new enums or nested objects are added.
related_specs:
  - ../mod.spec.md
  - ./protocol.rs.spec.md
  - ./jsonrpc_lite.rs.spec.md
  - ./bin/export.rs.spec.md
