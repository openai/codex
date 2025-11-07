You are resuming the active migration. Stay in the repository root and coordinate through the CLI tool so every agent shares the same state.

1. Run `migrate-cli execute`.
   - It selects the next runnable task, marks it `running`, prints a detailed brief, and drops a runbook under `runs/`.
   - Note the workspace path plus the plan/journal locations from the CLI output.
2. Follow the brief:
   - Read any referenced files, services, or dashboards.
   - Update `plan.md` when scope changes and log progress plus artifacts in `journal.md`.
   - Keep the checkpoints in the runbook so other agents can audit what happened.
3. When you finish, record the result with `migrate-cli execute --task-id <TASK_ID> --status done --note "short summary"`, then run `migrate-cli execute` again to fetch the next task.
4. If you discover blockers, use `--status blocked --note "context"` so the index reflects reality.

Always make the artifacts inside the migration workspace the source of truth: `plan.md` for decisions and sequencing, `journal.md` for hand-offs, `tasks.json`/`runs/` for orchestration metadata.
