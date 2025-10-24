## Overview
`codex-protocol-ts/generate-ts` is a convenience script that runs the workspace `just codex generate-ts` recipe and writes the generated TypeScript bindings to a temporary directory. It simplifies manual verification of the generator output.

## Detailed Behavior
- Uses `set -euo pipefail` to propagate failures from any command.
- Changes into the crate root (`cd "$(dirname "$0")"/..`) before execution so relative paths resolve correctly.
- Creates a temporary directory via `mktemp -d`, then invokes `just codex generate-ts` with the Prettier path pinned to `../node_modules/.bin/prettier` and the `--out` argument pointing at the temporary directory.
- Prints the directory path after completion so callers can inspect the generated files.

## Broader Context
- Relies on the `just` recipe defined elsewhere in the workspace; contributors must have `just` installed and Prettier available at the expected location. The script serves as a quick sanity check for local development.
- Context can't yet be determined for publishing artifacts; when distribution is automated, this script may become part of a CI pipeline or be replaced by a more structured tool.

## Technical Debt
- Hardcodes the Prettier path; environments without `node_modules/.bin/prettier` will fail. Making the path configurable (via env var or argument) would improve portability.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Allow overriding the Prettier binary path so the script works outside the workspace root or with globally installed tooling.
related_specs:
  - ./mod.spec.md
  - ./src/lib.rs.spec.md
  - ./src/main.rs.spec.md
