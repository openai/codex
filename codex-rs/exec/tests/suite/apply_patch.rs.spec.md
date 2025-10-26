## Overview
`apply_patch` tests the CLI and streaming tool paths for applying patches through `codex-exec`, covering both standalone binary usage and streamed tool invocations.

## Detailed Behavior
- `test_standalone_exec_cli_can_use_apply_patch` invokes `codex-exec` with the `--apply-patch` style argument, ensuring the CLI can apply patches without the multitool wrapper.
- `test_apply_patch_tool` (non-Windows) mounts SSE streams that first deliver a custom tool call, then a function-call-based patch, verifying both code paths create/update files.
- `test_apply_patch_freeform_tool` covers freeform patch events to ensure arbitrary file diffs apply correctly.
- Each scenario validates final file contents on disk, exercising response sequencing and the patch handler’s persistence logic.

## Broader Context
- Complements Phase 3 tool specs by confirming the CLI respects patch semantics and streaming events emitted by the server.

## Technical Debt
- Tests rely on SSE fixtures generated during runtime; consolidating repeated fixture setup across apply-patch tests could reduce duplication.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Extract shared SSE builders for patch sequences to simplify future tests.
related_specs:
  - ../mod.spec.md
  - ../../src/tools/apply_patch.rs.spec.md
