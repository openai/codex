# Agent Bus: High-level Journeys

This document outlines the main automation journeys enabled by the Agent Bus. These flows are CI-only (no runtime product changes) and are strictly gated via TOML configs, rate limits, and allowlists.

## 1) Reviewer PR health

- Trigger: Reviewer comments `/status` or manually runs “Agent Bus (events)”.
- Action: Bus fetches upstream PR status and posts a concise summary (checks, review decision, mergeability).
- Guardrails: `/status` must be allowlisted; rate-limited to avoid spam.

## 2) CI finished → notify downstream

- Trigger: `workflow_run` or `check_suite` completion for allowed workflows.
- Action: Bus sends a minimal signed `/notify` to the ops endpoint.
- Security: HTTP path allowlisted, idempotency key in header and JSON, HMAC `X-Hub-Signature-256`.

## 3) Alert to ops from monitors

- Trigger: Poller matches configured conditions (contains / json_path eq/ne/contains).
- Action: Bus posts a signed `/notify` (optionally includes a sample).
- Safety: Per-monitor cooldown and fingerprint dedup suppress alert loops.

## 4) Handoff to another agent

- Trigger: Reviewer or automation uses `/handoff`.
- Action: Bus posts an idempotent envelope to the approved endpoint.
- Limits: Route command must be allowlisted; HTTP path must be in allow_paths.

## 5) Queue-driven actions

- Trigger: Open issues labeled `agent-task`, where:
  - First line = command (e.g., `/notify`)
  - Body (optional) = JSON payload
- Action: Bus executes and comments results; moves to `agent-task-done` on success.
- Reliability: Dead-letters to `agent-task-failed` after repeated failures.

## 6) Review gate (follow-up PR)

- Trigger: Checks green and owner comments `/apply` or `/defer`.
- Action: Bus evaluates configured barriers and enqueues next steps.
- Control: Owners and barriers enforced from TOML.

## Security Overview

- Commands and routes are strictly allowlisted in TOML.
- Rate limiting prevents spam; monitors include cooldown + dedup.
- Outbound hooks are restricted by path allowlist, include idempotency, and are optionally HMAC-signed.
- Secrets are only expanded for specific headers via `${ENV:VAR}`.

## Operations Notes

- To enable monitors, add secrets in Actions and set `enabled = true` per monitor in `local/automation/monitors.toml`.
- To change notification paths, update `allow_paths` and `base_url` in `agent_bus.toml`.

