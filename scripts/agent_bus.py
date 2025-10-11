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
# Prefer local/automation (private); fall back to docs/automation examples
CFG_PRIMARY = ROOT / "local/automation/agent_bus.toml"
CFG_FALLBACK = ROOT / "docs/automation/agent_bus.example.toml"
LOCK = ROOT / ".git/.agent_bus_last"
HTTP_DEFAULT_PATH = "/ci/notify"
IDEMP_HEADER = "X-Idempotency-Key"

from scripts.connectors.github_conn import pr_status, pr_comment, rerun_placeholder
from scripts.connectors.http_conn import http_post
from scripts.connectors.mcp_conn import call_tool_stdio


def load_cfg():
    cfg_path = CFG_PRIMARY if CFG_PRIMARY.exists() else CFG_FALLBACK
    with cfg_path.open('rb') as f:
        return tomllib.load(f)


def http_path_allowed(cfg: dict, path: str) -> bool:
    agents = cfg.get("agents", {})
    http_ops = agents.get("http_ops") or {}
    allow = set(http_ops.get("allow_paths") or [])
    # Require explicit allowlist; refuse if none
    if not allow:
        return False
    return path in allow


def ts_bucket(ts: float, bucket_seconds: int = 300) -> int:
    return int(ts // bucket_seconds * bucket_seconds)


def make_idempotency_key(repo: str, pr: int, route: str, ts_val: int, path: str | None = None) -> str:
    import hashlib
    # Include path (if provided) to differentiate multiple allowed endpoints
    raw = f"{repo}|{pr}|{route}|{path or ''}|{ts_val}"
    return "sha256:" + hashlib.sha256(raw.encode("utf-8")).hexdigest()


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
        path = HTTP_DEFAULT_PATH
        if not http_path_allowed(cfg, path):
            print(f"[agent-bus] http path not allowed by config: {path}")
            return
        now = int(time.time())
        idem_ts = ts_bucket(now)
        idem_key = make_idempotency_key(repo, pr, command, idem_ts, path)
        payload = {
            "source": "agent-bus",
            "pr": pr,
            "repo": repo,
            "ts": now,
            "command": command,
            "idempotency_key": idem_key,
        }
        headers = (http.get('headers') or {}).copy()
        headers[IDEMP_HEADER] = idem_key
        http_post(
            http.get('base_url'),
            path,
            payload,
            headers=headers,
            allowed_paths=http.get('allow_paths') or [],
            hmac_secret_env=http.get('hmac_secret_env'),
            hmac_header=http.get('hmac_header', 'X-Hub-Signature-256'),
        )
        touch_lock()
    else:
        print(f"[agent-bus] command '{command}' not implemented")


def main():
    if not (CFG_PRIMARY.exists() or CFG_FALLBACK.exists()):
        print(f"[agent-bus] missing config: {CFG_PRIMARY} or {CFG_FALLBACK}")
        return 0
    cfg = load_cfg()
    if rate_limited(cfg):
        print("[agent-bus] rate-limited")
        return 0
    # Accept command from env, default to /status
    command = os.environ.get('AGENT_BUS_COMMAND', '/status')
    payload = os.environ.get('AGENT_BUS_PAYLOAD')
    # If payload present and command is /notify or /handoff, forward via http_ops (still gated)
    if payload and command in ('/notify', '/handoff'):
        # Enforce command allowlist before forwarding (prevents bypass via env)
        allowed = set(cfg.get('security', {}).get('allow_commands', []))
        if command not in allowed:
            print(f"[agent-bus] command '{command}' not allowed by config (payload branch)")
            return 0
        try:
            data = json.loads(payload)
        except Exception:
            data = {"payload": payload}
        http = cfg.get('agents', {}).get('http_ops') or {}
        path = HTTP_DEFAULT_PATH
        if not http_path_allowed(cfg, path):
            print(f"[agent-bus] http path not allowed by config: {path}")
            return 0
        repo = cfg.get('upstream_repo')
        pr = int(cfg.get('pr_number'))
        now = int(time.time())
        idem_ts = ts_bucket(now)
        idem_key = make_idempotency_key(repo, pr, command, idem_ts, path)
        if isinstance(data, dict):
            data.setdefault("ts", now)
            data.setdefault("idempotency_key", idem_key)
            data.setdefault("source", "agent-bus")
            data.setdefault("repo", repo)
            data.setdefault("pr", pr)
            data.setdefault("command", command)
        headers = (http.get('headers') or {}).copy()
        headers[IDEMP_HEADER] = idem_key
        http_post(
            http.get('base_url'),
            path,
            data,
            headers=headers,
            allowed_paths=http.get('allow_paths') or [],
            hmac_secret_env=http.get('hmac_secret_env'),
            hmac_header=http.get('hmac_header', 'X-Hub-Signature-256'),
        )
        touch_lock()
    else:
        handle_command(cfg, command)
    return 0


if __name__ == '__main__':
    sys.exit(main())
