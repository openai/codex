## Overview
`core::apply_patch` mediates between the Codex `apply_patch` tool and the execution pipeline. It evaluates patch safety, requests approvals when needed, and either executes patches programmatically or delegates to the shell runtime.

## Detailed Behavior
- `apply_patch` receives an `ApplyPatchAction` and uses `assess_patch_safety` (from `safety`) to classify the patch:
  - `SafetyCheck::AutoApprove` returns `InternalApplyPatchInvocation::DelegateToExec` with `ApplyPatchExec`, indicating the patch can run via the exec pipeline without user intervention (while noting whether explicit approval was previously granted).
  - `SafetyCheck::AskUser` triggers `Session::request_patch_approval`, summarizing file changes for user review. Approval results in delegation to exec with `user_explicitly_approved_this_action = true`; denial returns `InternalApplyPatchInvocation::Output` with a rejection error.
  - `SafetyCheck::Reject` constructs an `Output` variant immediately, returning an explanatory error to the model.
- `InternalApplyPatchInvocation` differentiates between inline handling (`Output`) and delegating to exec (for patch application via `handle_container_exec_with_params`).
- `convert_apply_patch_to_protocol` transforms `ApplyPatchAction` changes into protocol `FileChange` entries (Add/Delete/Update), ensuring event payloads and diff trackers share a consistent representation.
- Tests verify conversion for add/update/delete variants, guarding against regressions when `ApplyPatchAction` evolves.

## Broader Context
- This module sits between the tool handler and safety checks, allowing policy-based approvals to be centralized. Execution still flows through the orchestrator, preserving sandbox and telemetry handling.
- Safety decisions reference approval policy and sandbox policy, so changes in policy semantics must keep this evaluation in sync.
- Context can't yet be determined for incremental patch previews; current behaviour assumes binary approve/deny decisions.

## Technical Debt
- None noted; TODOs for richer approval summaries live in neighbouring modules.

---
tech_debt:
  severity: low
  highest_priority_items: []
related_specs:
  - ./tools/handlers/apply_patch.rs.spec.md
  - ./safety.rs.spec.md
  - ./codex.rs.spec.md
