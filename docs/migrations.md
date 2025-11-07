# Codex migrations

Codex ships a purpose-built `migrate-cli` binary plus slash commands so every migration follows the same playbook. The CLI manages workspaces under `migrations/migration_<slug>/`, keeps `.codex/migrate/index.json` updated, and prints detailed task briefs that Codex can execute.

## CLI quickstart

> Run the binary directly (`migrate-cli plan ...`) or via Cargo while developing (`cargo run -p codex-cli --bin migrate-cli -- plan ...`).

### `migrate-cli plan "<description>"`

* Creates `migrations/migration_<slug>/` with:
  * `plan.md` – canonical blueprint.
  * `journal.md` – running log of progress, hand-offs, and blockers.
  * `tasks.json` – orchestration metadata and dependencies.
  * `runs/` – runbooks generated per task when execution starts.
* Seeds a dependency-aware task graph so you can parallelize safely.
* Updates `.codex/migrate/index.json` so dashboards (or other agents) discover the workspace.

Use this command whenever you kick off a new initiative. After it runs, open the repo in Codex and use `/migrate` so the agent runs the same command and fills out `plan.md`/`journal.md` automatically.

### `migrate-cli execute [TASK_ID] [--status <state>] [--note "..."]`

* With no arguments it picks the next runnable task, marks it `running`, prints a task brief (workspace path, plan/journal locations, checkpoints), and drops a runbook under `runs/`.
* Use `--task-id <id> --status done --note "summary"` when you finish so the CLI records the journal entry and advances the graph.
* Use `--status blocked` to flag issues, or pass `--workspace <path>` if you are not working on the most recent migration.

Every invocation refreshes `.codex/migrate/index.json`, so team members and tools always see accurate status.

## Slash commands inside Codex

| Command | Purpose |
| --- | --- |
| `/migrate` | Ask Codex to run `migrate-cli plan` with your description, gather context, and populate `plan.md`/`journal.md`. |
| `/continue-migration` | Ask Codex to run `migrate-cli execute`, accept the next task brief, and push that task forward. |

Because the CLI writes the real artifacts, the slash commands simply queue up the right instructions so the agent runs the tool for you.

## Recommended workflow to share with your team

1. **Plan** – Open the repo in Codex and run `/migrate`. Codex will run `migrate-cli plan "<description>"`, scaffold the workspace, and fill in the executive overview plus incremental plan inside `plan.md` and `journal.md`.
2. **Execute** – Whenever you want the next piece of work, run `/continue-migration`. Codex runs `migrate-cli execute`, receives the task brief, and uses it (plus repo context) to do the work. When done it should mark the task complete with `migrate-cli execute --task-id <ID> --status done --note "summary"`.
3. **Repeat** – Continue using `/continue-migration` to keep the task graph flowing. `tasks.json` and `.codex/migrate/index.json` stay up to date automatically, and `runs/` accumulates runbooks for auditability.

Since everything lives in the repo, you can commit `plan.md`, `journal.md`, `tasks.json`, and `runs/` so asynchronous contributors (human or agent) always have the latest state.
