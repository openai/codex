# Codex migrations

Codex now ships an opinionated `@codex/migrate` package that sets up manifests, task graphs, and UI helpers so multi-agent migrations follow the same playbook. The workflow combines dedicated CLI commands with the `/migrate` slash command inside the TUI.

## 1. Install the package

Run the setup command once per repository:

```bash
codex migrate setup \
  --connector source-db \
  --connector billing-api \
  --mcp internal-secrets
```

This creates `.codex/migrate/manifest.toml` (edit it any time connectors or MCP servers change) plus an empty `.codex/migrate/index.json` registry. Use `--force` to re-initialize.

## 2. Generate a plan + workspace

Describe the migration and let Codex scaffold the workspace:

```bash
codex migrate plan "Phase 2 – move billing to Postgres"
```

The command:

1. Creates `migrations/migration_<slug>` with `plan.md`, `journal.md`, `tasks.json`, `prompt.txt`, and a `runs/` directory for per-task runbooks.
2. Seeds a task graph that separates gating steps from parallel workstreams (`parallel-scout-*`, `parallel-telemetry`, etc.).
3. Updates `.codex/migrate/index.json` so dashboards and teammates can discover the workspace.

Open the workspace in Codex and run `/migrate` to load the system prompt and mirror the plan into `plan.md` automatically. The generated `prompt.txt` is the same set of instructions if you want to paste it into another agent.

## 3. Drive execution

Use `codex migrate execute` to orchestrate tasks:

- `codex migrate execute` – pick the next runnable task, mark it `running`, and drop a runbook under `runs/`.
- `codex migrate execute TASK_ID --status done` – manually advance or unblock a task.
- `codex migrate execute TASK_ID --note "published dry-run #2"` – append a row to `journal.md` while updating the task.
- `codex migrate execute --workspace migrations/migration_slug …` – target a specific workspace (defaults to the most recent one).

The command enforces dependencies, updates `.codex/migrate/index.json`, and keeps `tasks.json` in sync so multiple operators (or agents) can share state.

## 4. Spin up the dashboard

Generate a lightweight dashboard scaffold that visualizes `index.json`:

```bash
codex migrate ui init
```

Serve the repository root (for example `npx http-server . -c-1 -p 4173`) and open `http://localhost:4173/migration-ui/`. The vanilla HTML/JS/CSS app polls `.codex/migrate/index.json`, showing statuses, ready-to-run parallel tasks, and the latest workspace metadata. Ask Codex to edit `migration-ui/app.js` or `styles.css` to customize it.

## Working with `/migrate`

Inside the TUI, `/migrate` prompts you for a short description, creates the same `plan.md`/`journal.md` artifacts, and sends the engineered migration prompt to the agent. When paired with the CLI workflow:

- Use `codex migrate plan` to bootstrap directories + tasks, then `/migrate` to populate the plan and executive summary.
- Keep `plan.md` as the canonical blueprint and `journal.md` for hand-offs or updates (the slash command reminds collaborators of this split).
- Use the CLI `execute` command whenever you want to launch or reassign a task—its runbooks include the same context you would otherwise paste manually.

## Quick reference to share with teammates

1. `codex migrate setup --connector <system>` – install the package + manifest.
2. `codex migrate plan "<description>"` – create `migration_<slug>` with plan/journal/tasks and prompt.
3. `codex migrate execute [task_id] [--status … --note …]` – mark tasks running/done and log updates.
4. `codex migrate ui init` – scaffold a dashboard that reads `.codex/migrate/index.json`.
5. Open the workspace in Codex and run `/migrate` whenever you need the agent to refresh the plan or produce new coordination artifacts.

Because every artifact lives inside the repo, you can commit the plan, journal, `tasks.json`, runbooks, and UI changes to collaborate asynchronously or ask Codex to evolve the dashboard itself.
