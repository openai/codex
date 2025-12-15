# Demo: Graphiti Memory Integration (Codex CLI)

This demo shows how Codex can **ingest every turn** into Graphiti and **recall relevant facts** on later turns without
slowing the agent loop (ingestion is async; recall is time-bounded and fail-open).

## Prerequisites
- Graphiti REST service running (example endpoints):
  - Graphiti API: `http://localhost:8000` (or `http://graph:8000` in Docker networks)
  - Neo4j Bolt: `bolt://localhost:7687` (or `bolt://neo4j:7687`)
- Verify Graphiti is healthy:
  - `curl -fsS http://localhost:8000/healthcheck`
  - `codex graphiti test-connection --endpoint http://localhost:8000 --smoke --allow-untrusted`

## Configure Codex
1. Mark the project as trusted in `~/.codex/config.toml`:

   ```toml
   [projects."/absolute/path/to/your/repo"]
   trust_level = "trusted"
   ```

2. Enable Graphiti ingestion + recall:

   ```toml
   [graphiti]
   enabled = true
   consent = true
   endpoint = "http://localhost:8000"
   group_id_strategy = "hashed"
   # Optional: include a one-time ownership context episode per group
   include_system_messages = true
   # Optional: stable per-user key for deriving Global scope group_id
   # user_scope_key = "me@example.com"
   ingest_scopes = ["session", "workspace"]

   [graphiti.recall]
   enabled = true
   # static (default) | auto (includes global only for "my preferences/terminology" queries)
   scopes_mode = "auto"
   scopes = ["session", "workspace"]

   [graphiti.auto_promote]
   # Optional: promote explicit Memory Directives like "preference (global): …"
   enabled = true
   ```

3. Check resolved status:
   - `codex graphiti status`

## End-to-end scenario
1. Start Codex in your repo (interactive):
   - `codex`
2. In the first turn, promote a stable preference via a Memory Directive:
   - `preference (global): I prefer rg over grep for searches.`
3. Continue the conversation (or start a new one) and ask:
   - “What is my preference for searching in this repo?”
4. Confirm recall is being injected:
   - Use a prompt inspection tool, or enable request logging in your setup.
   - You should see a `<graphiti_memory>` section included as a system message when recall returns facts.

## Inspect Graphiti directly (optional)
- Use `codex graphiti status` to find your derived workspace `group_id` and then query:

  ```bash
  curl -sS -X POST http://localhost:8000/search \
    -H 'content-type: application/json' \
    -d '{"group_ids":["<workspace_group_id>"],"query":"rg vs grep","max_facts":5}'
  ```

## Why this is better than the baseline
- Baseline Codex relies on the **current prompt window**; compaction can discard details and cross-session context is
  limited.
- Graphiti integration provides:
  - **Workspace scope** memory that persists across sessions.
  - **Session scope** memory isolated per conversation.
  - **Global scope** for durable “lessons learned” via promotion.
  - **Fail-open** behavior: Graphiti outages never block a turn.

## Redeploy Graphiti (runbook)
From the Graphiti repo:
- `docker compose up -d --build graph neo4j`
- Validate:
  - `docker compose ps`
  - `docker compose logs -f graph`
  - `curl -fsS http://localhost:8000/healthcheck`

Common failure modes:
- Neo4j not healthy → check `docker compose logs neo4j` and credentials.
- `/search` errors → ensure your Graphiti container has required model env vars (e.g. `OPENAI_API_KEY`, `OPENAI_BASE_URL`).
