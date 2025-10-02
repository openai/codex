# Agent Bus — Core Capabilities

This page ties the four critical capabilities to concrete code and workflows in this repo.

1) Near real-time agent communication
- GitHub events: `.github/workflows/agent_bus_events.yml` responds to `issue_comment`, `check_suite`, `workflow_run`.
- External agents → repository_dispatch: `.github/workflows/agent_bus_dispatch.yml` accepts `agent-event` and routes to the bus.
- Outbound notifications: `scripts/agent_bus.py` → `scripts/connectors/http_conn.py` (allowlisted paths, HMAC, idempotency).
- MCP (pluggable): `scripts/connectors/mcp_conn.py` invokes an external stdio MCP client if available (≤800 ms budget); otherwise returns retryable.

2) Push/pull data from approved APIs
- Push: `http_conn.http_post()` enforces allowlisted paths, short timeouts, retries, idempotency header, optional HMAC.
- Pull: `scripts/monitors_poll.py` + `docs/automation/monitors.example.toml` (GET + header env expansion, matchers, cooldown+d edup). Disabled by default until secrets exist.

3) Push/pull PRs and reviews
- Pull: `scripts/connectors/github_conn.py#pr_status` → used by `/status`, review_gate, review_watch.
- Push (comments): `pr_comment()` used throughout.
- Push (formal reviews): `pr_review()` submits APPROVE/REQUEST_CHANGES via review_gate on owner commands when checks are green.
- Rerun: `rerun_latest_actions_run()` triggers a rerun of the latest pull_request run for the PR head branch (requires UPSTREAM_GH_TOKEN with workflow scope).

4) Task queue for multiple requests
- Queue: `scripts/agent_bus_queue.py` processes issues labeled `agent-task` (first line = command; rest = JSON payload), comments results, and moves to `agent-task-done` or `agent-task-failed` after retries.
- Schedules: `.github/workflows/agent_bus_queue.yml` runs on cron and manual dispatch.

Security and guardrails
- Command allowlist: enforced in both direct and env-forwarding paths.
- HTTP path allowlist + optional HMAC signature and idempotency header.
- Rate limiting and per-monitor cooldown/fingerprint dedup.
- Branch scoping for all workflows (main|feat/agent-bus by default).

