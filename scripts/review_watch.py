#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

try:
    import tomllib  # py3.11+
except Exception as e:
    print(f"[review-watch] ERROR: Python 3.11+ required (tomllib missing): {e}", file=sys.stderr)
    sys.exit(0)

ROOT = Path(__file__).resolve().parent.parent
CFG_PRIMARY = ROOT / "local/automation/review_watch_config.toml"
CFG_FALLBACK = ROOT / "docs/automation/review_watch.example.toml"
LOCK = ROOT / ".git/.review_watch_last_post"

def load_cfg():
    cfg_path = CFG_PRIMARY if CFG_PRIMARY.exists() else CFG_FALLBACK
    with cfg_path.open('rb') as f:
        return tomllib.load(f)

def allowed_event(cfg: dict, event_name: str) -> bool:
    return event_name in cfg.get('allow', {}).get('events', [])

def allowed_comment(cfg: dict, payload: dict) -> bool:
    body = (payload.get('comment') or {}).get('body') or ''
    cmds = cfg.get('allow', {}).get('comment_commands', [])
    if any(body.strip().startswith(cmd) for cmd in cmds):
        if cfg.get('allow', {}).get('allow_any_user', True):
            return True
        # TODO: restrict to maintainers if desired
    return False

def rate_limited(cfg: dict) -> bool:
    try:
        min_secs = int(cfg.get('rate_limit', {}).get('min_seconds_between_posts', 0))
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

def run_review_watch(cfg: dict):
    upstream = cfg.get('upstream_repo')
    pr = str(cfg.get('pr_number'))
    env = os.environ.copy()
    env['UPSTREAM_REPO'] = upstream
    env['PR_NUMBER'] = pr
    script = str(ROOT / 'scripts/review_watch.sh')
    rc = os.system(script)
    touch_lock()
    return rc

def main():
    if not (CFG_PRIMARY.exists() or CFG_FALLBACK.exists()):
        print(f"[review-watch] Config missing: {CFG_PRIMARY} or {CFG_FALLBACK}")
        return 0
    cfg = load_cfg()
    event_name = os.environ.get('GITHUB_EVENT_NAME', 'schedule')
    if not allowed_event(cfg, event_name):
        print(f"[review-watch] Event '{event_name}' not allowed by config; skipping")
        return 0
    if event_name == 'issue_comment':
        payload_path = os.environ.get('GITHUB_EVENT_PATH')
        if payload_path and Path(payload_path).exists():
            payload = json.loads(Path(payload_path).read_text())
            if not allowed_comment(cfg, payload):
                print("[review-watch] Comment not allowed by config; skipping")
                return 0
    if rate_limited(cfg):
        print("[review-watch] Rate-limited; skipping post")
        return 0
    return run_review_watch(cfg)

if __name__ == '__main__':
    sys.exit(main())
