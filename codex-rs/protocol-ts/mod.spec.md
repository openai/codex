## Overview
`codex-protocol-ts` generates TypeScript bindings for Codex’s protocol types. The crate exposes a library with reusable generation logic and a small CLI (`generate-ts`) that invokes it, ensuring TypeScript clients stay in sync with the Rust protocol definitions.

## Detailed Behavior
- The library (`src/lib.rs`) exports `generate_ts`, which orchestrates TypeScript code generation, prepends a standard header, builds an `index.ts`, and optionally formats the output with Prettier.
- The binary (`src/main.rs`) wraps the library with a Clap-based interface that accepts the output directory and optional Prettier path.
- A helper script (`generate-ts`) invokes the just task `codex generate-ts`, directing output to a temporary directory. Contributors can run it to spot-check bindings without wiring their own arguments.

## Broader Context
- Generation relies on `ts-rs` derives within `codex-app-server-protocol`; any schema change in the Rust types requires re-running this generator to update the TypeScript artifacts consumed by SDKs and IDE integrations.
- The crate lives alongside other foundational protocol crates (`codex-protocol`, `codex-app-server-protocol`). Coordinated releases should include regenerated TS bindings.
- Context can't yet be determined for publishing the generated artifacts to npm; current workflow assumes consumers check the files into downstream repositories manually.

## Technical Debt
- Generation assumes Prettier is installed in `node_modules/.bin`. When that path changes, the helper script and docs need updates.

---
tech_debt:
  severity: low
  highest_priority_items:
    - Document an alternative Prettier path or validation step to catch missing dependencies before generation fails.
related_specs:
  - ./src/lib.rs.spec.md
  - ./src/main.rs.spec.md
  - ./generate-ts.spec.md
