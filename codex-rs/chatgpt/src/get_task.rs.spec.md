## Overview
`get_task` defines the minimal task schema needed to extract diff artifacts from the ChatGPT backend and provides a helper to fetch that payload for a given task ID.

## Detailed Behavior
- `GetTaskResponse` captures the current diff turn (if any). `AssistantTurn`, `OutputItem`, `PrOutputItem`, and `OutputDiff` mirror the subset of JSON fields required to locate a PR diff.
- Uses `#[serde(tag = "type")]` to discriminate output item variants, treating unrecognized items as `Other` while still parsing the response.
- `get_task(config, task_id)` constructs `/wham/tasks/{task_id}` and delegates to `chatgpt_get_request`, returning the deserialized `GetTaskResponse`.

## Broader Context
- Consumption flows back into `apply_command::apply_diff_from_task`, which depends on the PR diff payload to patch the local workspace.
- The REST client lives in `chatgpt_client`, ensuring HTTP concerns are centralized.

## Technical Debt
- Schema mirrors only the fields needed for diff application; if the backend evolves, missing fields could force refactors across the CLI without versioning support.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Add versioning or feature detection to guard against backend schema changes before they break diff extraction.
related_specs:
  - ../mod.spec.md
  - ./chatgpt_client.rs.spec.md
  - ./apply_command.rs.spec.md
