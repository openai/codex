# Android Agent Parent Session HOME Plan

## Summary

Split this work into two stages:

1. Framework, Launcher, and AgentSDK support for first-class HOME presentation of
   AGENT-anchored parent sessions, validated with AgentStub and GenieStub.
2. Codex Agent adoption of that framework surface, including a split between the
   management/admin UI and the per-session planner UI.

The first stage is the blocker. The current platform contract supports HOME
badging and tap routing for app-target sessions, but not a framework-owned HOME
icon surface for an AGENT-anchored parent session.

## Piece 1: Framework, Launcher, AgentSDK, AgentStub, and GenieStub

### Goal

Add a framework-owned HOME presentation for AGENT parent sessions so a live
planner session gets its own HOME icon, state badges, and `HANDLE_SESSION` tap
routing, with child-app ring adornments derived from its current Genie children.

### Required platform changes

- Add a HOME-visible icon surface for `ANCHOR_AGENT` parent sessions, keyed by
  parent `sessionId`.
- Standardize AGENT-parent HOME icon behavior:
  - `RUNNING`: show planner icon with active child-app ring and no
    question/result badge.
  - `WAITING_FOR_USER`: same icon plus question badge.
  - terminal result or error: same icon plus result badge.
  - icon tap always dispatches `android.app.agent.action.HANDLE_SESSION` for
    that parent session.
- Add explicit consume/remove semantics for the AGENT-parent HOME icon
  presentation:
  - removing the HOME icon must not delete the underlying session record.
  - management or admin UI must still be able to inspect the session afterward.
- Define child-app ring derivation:
  - preferred: framework derives the ring directly from the parent's current
    non-terminal child sessions.
  - recommended defaults:
    - show only currently active or waiting child sessions
    - stable ordering by child creation order
    - cap visible ring icons at 4 and use overflow treatment beyond that
- Keep parent question and result notifications and `HANDLE_SESSION` flow
  framework-owned and state-driven.
- Ensure HOME and AGENT remain convergent presenters:
  - if AGENT clears the parent HOME icon presentation, Launcher stops showing
    the icon.
  - if child-session set changes, Launcher updates the parent icon ring without
    AGENT-local bookkeeping drift.

### AgentSDK shape

- Prefer no new AGENT-authored icon metadata API if the framework can derive
  child ring state directly from child sessions.
- If an explicit consume API is needed, keep it narrowly scoped to
  AGENT-parent HOME presentation, not general session deletion.
- Keep tap routing through `HANDLE_SESSION`; the new surface should change
  presentation, not ownership.

### Stub validation

Update AgentStub and GenieStub to prove the framework behavior in the abstract
 before Codex adopts it.

- AgentStub should provide:
  - a management or admin surface
  - a per-parent-session handling surface for `HANDLE_SESSION`
  - a `Done` action that consumes or removes the parent HOME icon presentation
    without deleting session history
  - sequential handling for multiple user-facing questions from one parent
    session
- GenieStub should be updated only as needed to create realistic parent-child
  session trees and escalation paths.

### Acceptance tests

1. Starting an AGENT parent session creates one HOME parent-session icon.
2. Spawning Genie children adds their app icons around that parent icon.
3. Child question escalation produces a question badge on the parent icon.
4. Answering clears the question badge and resumes execution.
5. Parent completion produces a result badge.
6. Tapping `Done` removes the HOME parent-session icon but the session remains
   inspectable in AgentStub's admin UI.
7. Parent cancellation removes the HOME icon and cancels child sessions.
8. Launcher or SystemUI restart reconstructs the parent HOME icon from
   framework state.

### Prompt for the framework or stub implementation

```text
Implement first-class HOME presentation for AGENT-anchored parent sessions, then validate it with AgentStub and GenieStub.

Goal:
- A live AGENT parent session should get its own HOME icon surface, separate from target-app HOME badging.
- That HOME icon should use the Agent or Codex icon as the base, be keyed by parent sessionId, and route icon taps to `android.app.agent.action.HANDLE_SESSION` for that parent session.
- The icon should summarize current child Genie activity by showing the launcher icons of the parent’s currently active or waiting child target apps arranged around the base icon.
- Parent `WAITING_FOR_USER` should add a question badge.
- Parent terminal result or error should add a result badge.
- A consume or remove action should clear only the HOME icon presentation, not delete the parent session record.

Required behavior:
- Works for `ANCHOR_AGENT` parent sessions, not only HOME app-target sessions.
- HOME icon ring should reflect only current active or waiting child sessions, not all historical children.
- Recommended defaults unless implementation constraints force otherwise:
  - child ring ordering by child creation order
  - max 4 visible child icons with an overflow treatment beyond that
- Tapping the parent HOME icon always dispatches `HANDLE_SESSION` to the AGENT role holder.
- Clearing the parent HOME icon presentation should leave the session inspectable in the AGENT management UI.
- HOME and Launcher should converge from framework state; do not require the AGENT to manually push icon-ring state if framework can derive it from child sessions.

Validation client:
- Use AgentStub and GenieStub, not Codex Agent or Genie.
- Update AgentStub so it has:
  - a management or admin surface
  - a per-parent-session handling surface for `HANDLE_SESSION`
  - a `Done` action that consumes or removes the parent HOME icon presentation without deleting session history
  - sequential handling for multiple user-facing questions from one parent session
- Update GenieStub only as needed to create parent-child trees and question escalations.

Acceptance tests:
1. Starting an AGENT parent session creates one HOME parent-session icon.
2. Spawning Genie children adds their app icons around that parent icon.
3. Child question escalation produces a question badge on the parent icon.
4. Answering clears the question badge and resumes execution.
5. Parent completion produces a result badge.
6. Tapping Done removes the HOME parent-session icon but the session remains inspectable in AgentStub’s admin UI.
7. Parent cancellation removes the HOME icon and cancels child sessions.
8. Launcher restart or SystemUI restart still reconstructs the parent HOME icon from framework state.
```

## Piece 2: Codex Agent After Framework Support Lands

### Goal

Consume the new AGENT-parent HOME icon surface and split the Codex Agent UI
into:

- a management or admin entrypoint
- a planner-session entrypoint and per-session handling UI

### Codex changes

- Keep one APK, but expose two launcher icons:
  - `Codex Manager`: session list, session inspection, session cancellation, and
    debugging or admin controls
  - `Codex`: starts one new AGENT parent session prompt flow
- Add a dedicated per-parent-session UI flow:
  - create one parent session from the launcher prompt
  - show only that session's trace, child summaries, questions, results, and
    follow-up prompts
  - do not drop the user into the management list for normal session use
- Wire parent-session `HANDLE_SESSION` to the per-session UI, not the
  management UI.
- Use the framework-owned AGENT-parent HOME icon rather than app-managed pinned
  shortcuts.
- Parent question or result popup semantics:
  - question badge tap opens popup and asks queued user-facing questions in
    sequence
  - result badge tap opens result popup
  - `Done` consumes or removes the parent HOME icon presentation
  - session remains inspectable later from Manager

### Codex validation

- Tapping the `Codex` launcher icon opens only the new parent-session prompt UI.
- Tapping `Codex Manager` opens only admin or session-list UI.
- Starting a planner session creates a parent HOME icon and routes future taps
  for that session through the per-session UI.
- Child app ring, question badge, result badge, and `Done` behavior match the
  framework-validated stub behavior.
- Completed parent sessions disappear from HOME after `Done` but remain visible
  in Manager.

## Assumptions

- One APK with two launcher entrypoints is the chosen product shape.
- AGENT-parent HOME icons are framework-owned, not pinned shortcuts.
- Only current active or waiting child Genies should appear in the parent icon
  ring.
- After `Done`, the parent session HOME icon is removed, but session history
  remains inspectable from the management UI.
