## Overview
`resume` covers CLI behaviors for resuming Codex exec sessions, ensuring conversation files are appended rather than recreated and that CLI overrides persist.

## Detailed Behavior
- Helper routines scan the sessions directory for markers and extract conversation IDs from JSONL rollout files.
- `exec_resume_last_appends_to_existing_file` runs `codex-exec`, resumes with `resume --last`, and asserts the same JSONL file contains both markers.
- `exec_resume_by_id_appends_to_existing_file` resumes using an explicit conversation ID parsed from the session metadata.
- `exec_resume_preserves_cli_configuration_overrides` verifies that subsequent resume runs honor updated model flags while retaining sandbox settings, checking stderr for configuration echoes and confirming the same file is appended.

## Broader Context
- Ensures the CLI features for resuming conversations remain reliable, aligning with session management specs in `codex-core`.

## Technical Debt
- Tests parse JSONL manually; extracting shared utilities for session scanning would help other suites that need similar assertions.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Move JSONL scanning helpers into shared test support to avoid duplication if more suites inspect session files.
related_specs:
  - ../mod.spec.md
  - ../../src/lib.rs.spec.md
