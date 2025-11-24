# Turn diff on v2 `turn/completed`

Goal: add the turn-level unified diff to the v2 `turn/completed` notification (natural home: a `unified_diff` field on the `Turn` payload). Below is the scoped plan; no code changes yet.

## Current behavior
- Core emits per-patch diffs via `EventMsg::TurnDiff(TurnDiffEvent { unified_diff })` after each `apply_patch` call; `TurnDiffTracker` holds an in-memory baseline across the whole turn.
- App server v2 currently ignores `TurnDiff` and emits `TurnCompletedNotification` (API name `turn/completed`) with only `{ id, items: [], status }`.
- Clients that want a turn-wide diff must listen to the legacy `TurnDiff` stream event; v2 completion does not expose it.

## Proposed API shape
- Add an optional `unified_diff: Option<String>` to the v2 `Turn` struct so `TurnCompletedNotification` can carry the aggregated diff. Optional keeps backward compatibility for consumers that do not need it.

## Implementation steps
1) Protocol (app-server-protocol)
- Extend `v2::Turn` with `unified_diff: Option<String>`; include in TS exports.
- Update any serde defaults/tests expecting the exact shape (v2 turn_start/turn_interrupt suites, test client).

2) Capturing the diff for v2
- App server should persist the latest turn diff when `EventMsg::TurnDiff` arrives (currently ignored). Store `Option<String>` in `TurnSummary` keyed by conversation id; clear on turn completion/abort.
- Source remains the core `TurnDiffTracker` (no new work in core unless we want a final diff emission; existing `TurnDiff` after `PatchApplyEnd` already covers it).

3) Emitting on `turn/completed`
- When handling `EventMsg::TaskComplete`/`TurnAborted`, read the stored diff and include it in the `Turn` payload sent via `TurnCompletedNotification` (regardless of success/failure). Ensure the field is absent (`None`) if no patches were applied.

4) Tests and fixtures
- Update v2 app-server tests (`tests/suite/v2/turn_start.rs`, `turn_interrupt.rs`) to assert the new field and to cover the presence/absence cases.
- Adjust any JSON fixtures or snapshot-like expectations in test clients.

5) Client considerations
- Type updates propagate through TS bindings; document the new optional field in protocol docs/changelog. UIs can choose to render or ignore it.

## Open questions / decisions
- Should interrupted/failed turns include whatever diff was accumulated before abort? (Plan assumes yes; confirm.) - Answer: yes.
- Do we need a final `TurnDiff` emission at turn end for non-patch turns? (Current tracker only emits after patches; if we want an empty string vs `None`, decide before implementation.) - Answer: We want `None` if there were no patches.
