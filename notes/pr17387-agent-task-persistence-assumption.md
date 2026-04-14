# PR 17387 Agent Task Persistence Assumption

The review request said to persist the registered agent task_id "in the same place that other
session details are stored" without specifying the exact persistence shape.

Assumption used in this patch:

- Persist session-scoped agent task state in the thread rollout file, alongside other resumable
  thread/session data.
- Keep agent identity registration on startup, but register the backend task lazily when a session
  first needs task-scoped auth.
- Model task updates as an append-only RolloutItem::SessionState record instead of mutating the
  canonical first SessionMeta line, because resumed threads need later updates and clears to win.
- Do not carry auth-binding fields on the persisted session task; only persist the task fields
  needed to resume the same backend task for the same stored registered agent identity.
