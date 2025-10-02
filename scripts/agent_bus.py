#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

try:
    import tomllib  # py3.11+
except Exception as e:
    print(f"[agent-bus] Python 3.11+ required: {e}", file=sys.stderr)
    sys.exit(0)

ROOT = Path(__file__).resolve().parent.parent
CFG = ROOT / "local/automation/agent_bus.toml"
LOCK = ROOT / ".git/.agent_bus_last"

from scripts.connectors.github_conn import pr_status, pr_comment, rerun_placeholder
from scripts.connectors.http_conn import http_post
from scripts.connectors.mcp_conn import call_tool_stdio


def load_cfg():
    with CFG.open('rb') as f:
        return tomllib.load(f)


def rate_limited(cfg):
    try:
        min_secs = int(cfg.get('security', {}).get('min_seconds_between_posts', 0))
        if min_secs <= 0:
            return False
        if LOCK.exists():
            last = float(LOCK.read_text().strip())
            if time.time() - last < min_secs:
                return True
        return False
    except Exception:
        return False


def touch_lock():
    try:
        LOCK.parent.mkdir(parents=True, exist_ok=True)
        LOCK.write_text(str(time.time()))
    except Exception:
        pass


def summarize_status(repo: str, pr: int, gh_token: str | None) -> str:
    data = pr_status(repo, pr, gh_token)
    checks = []
    for c in data.get('statusCheckRollup', []) or []:
        name = c.get('workflowName') or c.get('name') or 'check'
        conclusion = c.get('conclusion') or c.get('status') or 'PENDING'
        checks.append(f"- {name}: {conclusion}")
    checks_s = '\n'.join(checks) if checks else '(no checks reported)'
    return (
        f"[agent-bus] PR status\n"
        f"- State: {data.get('state')}\n- Mergeable: {data.get('mergeable')}\n"
        f"- Review: {data.get('reviewDecision')}\n- Updated: {data.get('updatedAt')}\n"
        f"- Link: {data.get('url')}\n\nChecks:\n{checks_s}\n"
    )


def handle_command(cfg, command: str):
    allowed = set(cfg.get('security', {}).get('allow_commands', []))
    if command not in allowed:
        print(f"[agent-bus] command '{command}' not allowed by config")
        return
    gh_token = os.environ.get('UPSTREAM_GH_TOKEN')
    repo = cfg.get('upstream_repo')
    pr = int(cfg.get('pr_number'))
    if command in ('/status', '/rerun'):
        if command == '/status':
            body = summarize_status(repo, pr, gh_token)
            pr_comment(repo, pr, body, gh_token)
        elif command == '/rerun':
            rerun_placeholder(repo, pr, gh_token)
        touch_lock()
    elif command in ('/handoff', '/notify'):
        http = cfg.get('agents', {}).get('http_ops') or {}
        payload = {"source": "agent-bus", "pr": pr, "repo": repo, "ts": int(time.time()), "command": command}
        http_post(http.get('base_url'), '/ci/notify', payload, http.get('headers') or {})
        touch_lock()
    else:
        print(f"[agent-bus] command '{command}' not implemented")


def main():
    if not CFG.exists():
        print(f"[agent-bus] missing config: {CFG}")
        return 0
    cfg = load_cfg()
    if rate_limited(cfg):
        print("[agent-bus] rate-limited")
        return 0
    # Accept command from env, default to /status
    command = os.environ.get('AGENT_BUS_COMMAND', '/status')
    payload = os.environ.get('AGENT_BUS_PAYLOAD')
    # If payload present and command is /notify or /handoff, forward via http_ops
    if payload and command in ('/notify', '/handoff'):
        try:
            data = json.loads(payload)
        except Exception:
            data = {"payload": payload}
        http = cfg.get('agents', {}).get('http_ops') or {}
        http_post(http.get('base_url'), '/ci/notify', data, http.get('headers') or {})
        touch_lock()
    else:
        handle_command(cfg, command)
    return 0


if __name__ == '__main__':
    sys.exit(main())
