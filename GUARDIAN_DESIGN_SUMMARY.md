# Guardian Approval Mode Design Summary (Maja Iteration)

## Context

This document summarizes the guardian design iteration and the simplified MVP direction.

Simplified MVP framing:

- `guardian` is an optional approval mode for targeted/internal rollouts.
- It is not the default consumer approval behavior.
- It is gated so stricter GA surfaces/modes do not need to enable or expose it.

## Goals

- Add a new approval mode: `guardian`.
- Feature-flag the existence/availability of `guardian`.
- Keep agent execution sandbox-first.
- On sandbox failure/escalation need (write/network restrictions), ask an internal reviewer model for a risk decision.
- Auto-allow low/medium risk escalation; deny high risk escalation.
- Make review visible in UX with clear assessment messaging.
- Preserve safety against exfiltration/costly destructive actions.

## MVP Non-Conflict Contract

- `guardian` is disabled by default.
- `guardian` is only selectable when an explicit feature flag is enabled.
- Stricter GA configurations can omit `guardian` entirely and use their own approval/monitoring path.
- `guardian` is an AIM-compatible MVP slice for sandbox escalation review, not a replacement for cross-surface confirmation UX.

## High-Level Behavior

In `-a guardian` mode:

1. Tool call runs in sandbox as normal.
2. If sandbox denies and escalation is needed, Codex triggers guardian assessment.
3. Guardian model evaluates the planned escalated action from transcript + context and returns strict JSON risk output.
4. Decision rule:
   - `risk_score < 80`: escalation approved, retry outside sandbox.
   - `risk_score >= 80`: escalation denied.
5. If denied, model receives a policy-rejection response; agent must not circumvent with workaround.

## Guardian Assessment Contract

### Model

- Internal subagent model: `gpt-5.3-codex`.

### Prompting

- Guardian instructions were moved to user message payload (not base/system instructions).
- Transcript payload is wrapped as:
  - `>>> TRANSCRIPT START`
  - `>>> TRANSCRIPT END`

### Output schema

Guardian returns strict JSON:

```json
{
  "risk_level": "low" | "medium" | "high",
  "risk_score": 0-100,
  "rationale": "...",
  "evidence": [{"message":"...","why":"..."}]
}
```

## Transcript Construction (latest iteration)

Guardian transcript builder behavior:

- Preserves key user-intent history even under truncation.
- Keeps all retained user messages plus up to the last 20 recent messages.
- Maintains original conversation numbering (does not renumber from 1 after truncation).
- Adds explicit omission note when history is dropped, including:
  - session id
  - rollout/session file path
  - note that full conversation can be consulted for deeper judgment.

### Context-window control plan

To avoid overshooting model context when “all retained users + last 20” is still too large:

- Treat compaction artifacts as opaque:
  - do not repurpose `ResponseItem::Compaction` payloads as guardian transcript content;
  - compaction ordering/placement is model-training-sensitive (compaction item kept last in that flow), so guardian should not imitate or mutate that format;
  - do not rely on compact-summary formatting for guardian context.
- Use hard budget reservation:
  - reserve space for the planned-call JSON block and omission/meta text first;
  - reserve this section before adding any transcript entries.
- Deterministic inclusion priority:
  - always include original task user intent and latest user ask;
  - then include newest remaining entries (up to 20) while budget allows.
- Overflow fallback order:
  - reduce per-entry text caps using existing truncation utilities (`truncate_text` / token budget heuristics);
  - drop oldest non-user entries first;
  - then drop oldest remaining entries while preserving the required user-anchor messages.
- Never truncate planned-call JSON:
  - if needed, drop transcript entries entirely and send an action-only fallback (planned-call JSON + omission note).

## Escalation Triggers Covered

Guardian review is used for sandbox escalation paths including:

- Filesystem write restrictions.
- Network restrictions (including host allowlist/external network denial flows).

## Explicit Escalation Behavior

A key iteration fixed behavior where explicit `require_escalated` requests were blocked in guardian mode.
Final intent:

- Main agent **can** request `require_escalated` under guardian mode.
- Request routes to guardian review instead of being outright blocked.

## Decision Messaging

### User-facing status/events

- Guardian assessment displayed in warning style (yellow with `⚠`).
- Debug-only guardian event spam was removed in later iteration.

### Message returned to model on guardian rejection

Final intended rejection payload to model:

"Guardian rejected this action due to unacceptable risk. The agent must not attempt to achieve the same outcome via workaround, indirect execution, or policy circumvention. Proceed only with a materially safer alternative, or stop and request user input."

## Important Prompt Policy Intent

The guardian prompt evolved to bias toward evidence-based decisions:

- Use tools/read-only checks when local state matters.
- Do not over-penalize sandbox retry itself.
- Do not treat “outside workspace root” alone as high risk.
- For user-requested local destructive ops (e.g., `rm -rf` on a specific path), evaluate based on verified scope/content.

## Maja YOLO Prompt Proposal (added)

Source artifact currently in repo root:

- `/Users/ccunningham/code/codex/YOLO guardian prompt.md`

Proposal includes two prompt variants:

1. **OpenAI prompt (statsig-overwritable)**: OpenAI-specific policy language.
2. **General prompt**: portable/default wording.

Key additions compared with current summarized guardian prompt intent:

- Explicitly broadens “internal-safe” treatment for approved internal pipelines/workflows and explicitly named approved third-party systems.
- Clarifies credential usage: using credentials for user-requested actions is not automatically credential exfiltration.
- Requires identifying what data is actually leaving before assigning high risk for network actions.
- Clarifies destructive classification: large-scale internal operational actions are not automatically destructive absent evidence of damage/disruption/export.
- Keeps strict JSON response schema unchanged (`risk_level`, `risk_score`, `rationale`, `evidence`).

Integration notes for plan:

- Decide whether to maintain two prompt tiers (OpenAI/internal vs general) in code/config.
- If added to configurable prompt channels (e.g., statsig or config), define precedence and fallback behavior.
- Keep schema contract stable so parser/decision logic (`risk_score < 80`) does not change.
- Validate prompt updates against the known denial/approval edge cases from earlier guardian iteration (local `touch`, user-requested narrow `rm -rf`, network exfiltration checks).

## UX/CLI Requirements Captured

- CLI/TUI should visibly indicate guardian assessment is being performed.
- Final assessment should remain visible and readable.
- Earlier requirement for verbose guardian debug event stream was later removed/cleaned up.

## Cross-Crate Integration Points (MVP Target)

When implemented, changes should be explicitly gated across:

- `protocol` and app-server protocol enum exposure.
- CLI and TUI mode parsing/display.
- core policy/orchestrator/sandbox escalation paths.
- app-server/MCP/SDK mappings.

MVP requirement:

- If the feature flag is off, `guardian` should not appear as an available approval mode in user-facing surfaces.

## Known Operational Issues Encountered During Iteration

- Reviewer subagent timeout at 30s was too tight for large inherited context.
- Fixes included increasing timeout, reducing transcript size, and slimming subagent config.
- One issue stemmed from invalid reasoning effort value (`minimal` not supported for `gpt-5.3-codex`), corrected to `low`.
- In some environments, SSE stream disconnects occurred on responses API; websocket transport was used as practical workaround.

## Open Follow-up Checks for Pickup

When continuing implementation, verify:

1. End-to-end denial path returns the exact guardian rejection string (without extra wrapper text).
2. Feature-gating behaves correctly:
   - with flag off, `guardian` is not exposed/selectable.
   - with flag on, `guardian` is exposed where intended.
3. If enabled, `guardian` appears consistently in:
   - protocol enums
   - app-server v2 types/schemas
   - CLI help/options
   - MCP and SDK mappings.
4. Schema fixtures are regenerated where required (`just write-config-schema`, app-server schema generation).
5. TUI snapshots are updated if user-visible guardian text changed.
6. Full workspace test run policy is followed before merge.

---

If you want, I can next produce a code-pointer matrix (file + function + responsibility) for the current branch you’re on so you can diff this intended design against what is actually present in your clone.
