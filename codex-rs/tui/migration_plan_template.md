# Migration plan – {{MIGRATION_SUMMARY}}

> Workspace: `{{WORKSPACE_NAME}}`
> Generated: {{CREATED_AT}}

Use this document as the single source of truth for the migration effort. Keep it updated so any engineer (or agent) can jump in mid-flight.

## 1. Context & stakes
- Current state snapshot
- Target end state and deadline/launch windows
- Guardrails, SLAs, compliance/regulatory constraints

## 2. Incremental plan (numbered)
1. `[Step name]` — Purpose, scope, primary owner/skillset, upstream/downstream dependencies, validation & rollback signals.
2. `…`

Each step must explain:
- Preconditions & artifacts required before starting
- Specific code/data/infrastructure changes
- Telemetry, tests, or dry-runs that prove success

## 3. Parallel workstreams
| Workstream | Objective | Inputs & dependencies | Ownership / skills | Progress & telemetry hooks |
| ---------- | --------- | --------------------- | ------------------ | ------------------------- |
| _Fill in during planning_ |  |  |  |  |

## 4. Data + rollout considerations
- Data migration / backfill plan
- Environment readiness, feature flags, or config toggles
- Rollout plan (phases, smoke tests, canaries) and explicit rollback/kill-switch criteria

## 5. Risks, decisions, and follow-ups
- Top risks with mitigation owners
- Open questions / decisions with DRI and due date
- Handoff expectations (reference `journal.md` for ongoing updates)
