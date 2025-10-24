#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

try:
    import tomllib  # py3.11+
except Exception as e:
    print(f"[review-gate] Python 3.11+ required: {e}", file=sys.stderr)
    sys.exit(0)

ROOT = Path(__file__).resolve().parent.parent
CFG_PRIMARY = ROOT / "local/automation/agent_bus.toml"
CFG_FALLBACK = ROOT / "docs/automation/agent_bus.example.toml"

from scripts.connectors.github_conn import pr_status, pr_comment, pr_review


def load_cfg():
    cfg_path = CFG_PRIMARY if CFG_PRIMARY.exists() else CFG_FALLBACK
    with cfg_path.open('rb') as f:
        return tomllib.load(f)


def actor_in_owners(cfg: dict, login: str | None) -> bool:
    if not login:
        return False
    owners = cfg.get('review', {}).get('owners') or []
    if not owners:
        return False  # owners required for actionable review commands
    norm = login.lower()
    owners_norm = set(o.lower().lstrip('@') for o in owners)
    return norm in owners_norm


def all_barriers_green(status_data: dict, barriers: list[str]) -> tuple[bool, list[str]]:
    missing: list[str] = []
    roll = status_data.get('statusCheckRollup') or []
    by_name = {}
    for c in roll:
        name = c.get('workflowName') or c.get('name') or ''
        conclusion = (c.get('conclusion') or '').upper()
        by_name[name] = conclusion
    for b in barriers or []:
        concl = by_name.get(b)
        if concl != 'SUCCESS':
            missing.append(b)
    return (len(missing) == 0, missing)


def gh_issue_create(title: str, body: str, labels: list[str] | None = None):
    import subprocess
    cmd = ["gh", "issue", "create", "-t", title, "-b", body]
    for lab in (labels or []):
        cmd += ["-l", lab]
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    out, err = p.communicate()
    if p.returncode != 0:
        raise RuntimeError(f"gh issue create failed: {p.returncode}\n{err}\n{out}")
    print(out)


def main():
    if not (CFG_PRIMARY.exists() or CFG_FALLBACK.exists()):
        print(f"[review-gate] missing config: {CFG_PRIMARY} or {CFG_FALLBACK}")
        return 0
    cfg = load_cfg()
    upstream = cfg.get('upstream_repo')
    pr_num = int(cfg.get('pr_number'))
    gh_token = os.environ.get('UPSTREAM_GH_TOKEN')
    review_cfg = cfg.get('review', {})
    barriers = review_cfg.get('barriers') or []
    actions = review_cfg.get('actions') or {"approve": "/apply", "defer": "/defer", "decline": "/decline"}
    fire_on_fail = bool(review_cfg.get('fire_on_fail', False))
    approve_on_fail = bool(review_cfg.get('approve_on_fail', False))
    # Map configured action commands to review intents
    approve_cmd = actions.get("approve", "/apply")
    defer_cmd = actions.get("defer", "/defer")
    decline_cmd = actions.get("decline", "/decline")
    request_changes_cmds = {defer_cmd, decline_cmd}
    approve_cmds = {approve_cmd}
    actionable_cmds = approve_cmds.union(request_changes_cmds)

    # Fetch PR status once
    status = pr_status(upstream, pr_num, gh_token)
    ok, missing = all_barriers_green(status, barriers)

    event_name = os.environ.get('GITHUB_EVENT_NAME', 'schedule')
    if event_name == 'issue_comment':
        event_path = os.environ.get('GITHUB_EVENT_PATH')
        payload = json.loads(Path(event_path).read_text()) if event_path and Path(event_path).exists() else {}
        body = (payload.get('comment') or {}).get('body') or ''
        actor = (payload.get('comment') or {}).get('user', {}).get('login')
        # For actionable review commands, require actor to be in owners.
        body_stripped = body.strip()
        if body_stripped.startswith('/') and body_stripped.split('\n', 1)[0] in ("/apply", "/defer", "/decline"):
            if not actor_in_owners(cfg, actor):
                print(f"[review-gate] actor '{actor}' not in owners; skipping actionable review command")
                return 0
        # Map owner intent to formal PR review. If fire_on_fail is set, proceed even when barriers are missing.
        if body_stripped in actionable_cmds:
            if not ok:
                pr_comment(upstream, pr_num, f"[review-gate] Barriers not satisfied; missing: {', '.join(missing)}", gh_token)
                if not fire_on_fail:
                    return 0
            review_msg = None
            if "\n" in body:
                review_msg = body.split("\n", 1)[1].strip() or None
            if body_stripped in approve_cmds:
                if ok or approve_on_fail:
                    pr_review(upstream, pr_num, "approve", review_msg or "[review-gate] Approved.", gh_token)
                    pr_comment(upstream, pr_num, "[review-gate] Submitted APPROVE review.", gh_token)
                else:
                    # If not allowed to approve on failure, submit REQUEST_CHANGES but still fire
                    note = "[review-gate] Requested changes (checks not green)."
                    pr_review(upstream, pr_num, "request_changes", review_msg or note, gh_token)
                    pr_comment(upstream, pr_num, note, gh_token)
            else:
                pr_review(upstream, pr_num, "request_changes", review_msg or "[review-gate] Requested changes (deferred/declined).", gh_token)
                pr_comment(upstream, pr_num, "[review-gate] Submitted REQUEST_CHANGES review.", gh_token)
            return 0
        print("[review-gate] no actionable comment; ignoring")
        return 0

    # For workflow_run/check_suite/schedule: just post a status summary and list missing barriers when not ok
    if ok:
        pr_comment(upstream, pr_num, "[review-gate] All required checks are green.", gh_token)
    else:
        pr_comment(upstream, pr_num, f"[review-gate] Missing checks: {', '.join(missing)}", gh_token)
    return 0


if __name__ == '__main__':
    sys.exit(main())
