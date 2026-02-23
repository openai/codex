# Agent Teams (experimental)

This note summarizes the current in-process Agent Teams workflow implemented by Codex multi-agent tools.

## Team lifecycle

1. Create a team:

```json
{
  "team_id": "my-team",
  "members": [
    {
      "name": "planner",
      "task": "Plan the work",
      "agent_type": "architect",
      "worktree": true,
      "background": true
    },
    { "name": "worker", "task": "Implement the plan", "agent_type": "develop" }
  ]
}
```

Call: `spawn_team`

There is no fixed default team size. Set `members` according to task complexity and independent workstreams.

2. Wait for members:

- Call `wait_team` with `mode: "all"` or `mode: "any"`.

3. Close members:

- Call `close_team` (optional `members` list for partial close).
- Call `team_cleanup` to shut down all members and remove persisted team artifacts.

## Persisted data

When `spawn_team` succeeds, Codex persists:

- Team config: `$CODEX_HOME/teams/<team_id>/config.json`
- Initial tasks: `$CODEX_HOME/tasks/<team_id>/*.json`

Team config is updated after partial `close_team`. Team config/tasks are removed after full close/cleanup.

## Task tools

- `team_task_list`: list persisted tasks.
- `team_task_claim`: claim a specific task.
- `team_task_claim_next`: claim the next claimable pending task (optionally for `member_name`).
- `team_task_complete`: mark a task completed.

Typical flow:

1. `team_task_list`
2. `team_task_claim_next`
3. Do work
4. `team_task_complete`

## Team messaging tools

- `team_message`: send input to one member by `member_name`.
- `team_broadcast`: send one message/items payload to all team members.

Both accept either `message` or `items` (not both), and optional `interrupt`.

## End-to-end JSON example (`spawn_team` -> `team_cleanup`)

> Notes:
>
> - `team_id` is explicitly set for deterministic follow-up calls.
> - `agent_type` can be built-in roles (for example `architect`, `develop`, `code-review`) or custom roles from your config.
> - `worktree` (optional, default `false`) spawns that member in a dedicated git worktree.
> - `background` (optional, default `false`) marks that member as background work (informational).
> - IDs like `agent_id`, `task_id`, `submission_id` are runtime values.

1. `spawn_team`

Request:

```json
{
  "team_id": "demo-team",
  "members": [
    {
      "name": "planner",
      "task": "Define rollout plan",
      "agent_type": "architect"
    },
    {
      "name": "implementer",
      "task": "Implement the changes",
      "agent_type": "develop"
    },
    {
      "name": "reviewer",
      "task": "Review risks and edge cases",
      "agent_type": "code-review"
    }
  ]
}
```

Example result:

```json
{
  "team_id": "demo-team",
  "members": [
    { "name": "planner", "agent_id": "9a2f...e81", "status": "pending_init" },
    {
      "name": "implementer",
      "agent_id": "cf93...b70",
      "status": "pending_init"
    },
    { "name": "reviewer", "agent_id": "4ed2...7ab", "status": "pending_init" }
  ]
}
```

2. `team_task_list`

Request:

```json
{ "team_id": "demo-team" }
```

Example result:

```json
{
  "team_id": "demo-team",
  "tasks": [
    {
      "task_id": "task-a",
      "title": "Define rollout plan",
      "state": "pending",
      "depends_on": [],
      "assignee_name": "planner",
      "assignee_agent_id": "9a2f...e81",
      "updated_at": 1739988000
    },
    {
      "task_id": "task-b",
      "title": "Implement the changes",
      "state": "pending",
      "depends_on": [],
      "assignee_name": "implementer",
      "assignee_agent_id": "cf93...b70",
      "updated_at": 1739988000
    }
  ]
}
```

3. `team_task_claim_next`

Request:

```json
{ "team_id": "demo-team", "member_name": "planner" }
```

Example result:

```json
{
  "team_id": "demo-team",
  "claimed": true,
  "task": {
    "task_id": "task-a",
    "title": "Define rollout plan",
    "state": "claimed",
    "depends_on": [],
    "assignee_name": "planner",
    "assignee_agent_id": "9a2f...e81",
    "updated_at": 1739988002
  }
}
```

4. `team_message` and `team_broadcast`

`team_message` request:

```json
{
  "team_id": "demo-team",
  "member_name": "planner",
  "message": "Please deliver a 3-step plan.",
  "interrupt": false
}
```

`team_message` example result:

```json
{
  "team_id": "demo-team",
  "member_name": "planner",
  "agent_id": "9a2f...e81",
  "submission_id": "subm-1"
}
```

`team_broadcast` request:

```json
{
  "team_id": "demo-team",
  "message": "Post current status in one paragraph.",
  "interrupt": false
}
```

`team_broadcast` example result:

```json
{
  "team_id": "demo-team",
  "sent": [
    {
      "member_name": "planner",
      "agent_id": "9a2f...e81",
      "submission_id": "subm-2"
    },
    {
      "member_name": "implementer",
      "agent_id": "cf93...b70",
      "submission_id": "subm-3"
    }
  ],
  "failed": []
}
```

5. `team_task_complete`

Request:

```json
{ "team_id": "demo-team", "task_id": "task-a" }
```

Example result:

```json
{
  "team_id": "demo-team",
  "completed": true,
  "task": {
    "task_id": "task-a",
    "title": "Define rollout plan",
    "state": "completed",
    "depends_on": [],
    "assignee_name": "planner",
    "assignee_agent_id": "9a2f...e81",
    "updated_at": 1739988004
  }
}
```

6. `wait_team`

Request:

```json
{
  "team_id": "demo-team",
  "mode": "all",
  "timeout_ms": 120000
}
```

Example result:

```json
{
  "completed": true,
  "mode": "all",
  "triggered_member": null,
  "member_statuses": [
    { "name": "planner", "agent_id": "9a2f...e81", "state": "shutdown" },
    { "name": "implementer", "agent_id": "cf93...b70", "state": "shutdown" },
    { "name": "reviewer", "agent_id": "4ed2...7ab", "state": "shutdown" }
  ]
}
```

7. `team_cleanup`

Request:

```json
{ "team_id": "demo-team" }
```

Example result:

```json
{
  "team_id": "demo-team",
  "removed_from_registry": true,
  "removed_team_config": true,
  "removed_task_dir": true,
  "closed": [
    {
      "name": "planner",
      "agent_id": "9a2f...e81",
      "ok": true,
      "status": "shutdown",
      "error": null
    },
    {
      "name": "implementer",
      "agent_id": "cf93...b70",
      "ok": true,
      "status": "shutdown",
      "error": null
    },
    {
      "name": "reviewer",
      "agent_id": "4ed2...7ab",
      "ok": true,
      "status": "shutdown",
      "error": null
    }
  ]
}
```
