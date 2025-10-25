## Overview
`core::safety` evaluates whether file-system patches proposed through the `apply_patch` tool can execute automatically, require user approval, or must be rejected. It encodes policy decisions drawn from sandbox configuration, writable roots, and platform capabilities to keep edits within the allowed workspace.

## Detailed Behavior
- `SafetyCheck` enumerates outcomes:
  - `AutoApprove` tracks the sandbox type that will be used and whether the user explicitly approved.
  - `AskUser` signals that manual approval is required.
  - `Reject` carries a message describing why the operation is unsafe.
- `assess_patch_safety`:
  - Rejects empty patches outright.
  - Respects `AskForApproval` policy semantics, short-circuiting to `AskUser` for `UnlessTrusted` until writable-root checks run (see TODO in source).
  - Calls `is_write_patch_constrained_to_writable_paths` to verify that all file changes remain inside declared writable roots; if so, and sandboxing is available, it auto-approves with the platform-specific sandbox identified by `get_platform_sandbox`.
  - Falls back to `AskUser` when sandboxing cannot be enforced or when policy requires manual review (`OnRequest`). Rejects attempts that escape writable roots under `AskForApproval::Never`.
- `get_platform_sandbox` reports the sandbox runtime available on the current OS (Seatbelt on macOS, seccomp on Linux, `None` elsewhere).
- `is_write_patch_constrained_to_writable_paths`:
  - Expands the active sandbox policy into normalized writable roots relative to the current working directory.
  - Checks every added, deleted, or updated path (including move destinations) to confirm containment within an allowed root, handling both absolute and relative paths without touching the filesystem.
  - Honors `DangerFullAccess` (always true) and `ReadOnly` (always false).
- Unit tests cover writable-root evaluation for adds and policy overrides with temporary workspaces.

## Broader Context
- Used by the apply-patch tool handler (`./tools/handlers/apply_patch.rs.spec.md`) and command safety layers to decide whether to run edits automatically or escalate to users.
- Integrates with sandbox configuration from `./config.rs.spec.md` and execution plumbing in `./shell.rs.spec.md` / `./seatbelt.rs.spec.md`.
- Complements command-level checks in `command_safety` by focusing on file-system impact rather than shell commands.

## Technical Debt
- Approval flow for `AskForApproval::UnlessTrusted` short-circuits before writable-root validation; the TODO notes this should probably continue through the normal safety pipeline to avoid unnecessary prompts.

---
tech_debt:
  severity: medium
  highest_priority_items:
    - Ensure `UnlessTrusted` policies run writable-root checks before prompting users.
related_specs:
  - ./config.rs.spec.md
  - ./command_safety/mod.rs.spec.md
  - ./tools/handlers/apply_patch.rs.spec.md
  - ./seatbelt.rs.spec.md
