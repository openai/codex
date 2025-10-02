#!/usr/bin/env python3
import json
import os
import subprocess
from pathlib import Path

REPO = os.environ.get('GITHUB_REPOSITORY')  # owner/repo for the fork
ROOT = Path(__file__).resolve().parent.parent


def run(cmd):
    p = subprocess.Popen(cmd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    out, err = p.communicate()
    if p.returncode != 0:
        raise RuntimeError(f"cmd failed: {' '.join(cmd)}\n{err}\n{out}")
    return out


def list_tasks():
    out = run(["gh", "issue", "list", "--search", "label:agent-task state:open", "--limit", "20", "--json", "number,title,body"])
    return json.loads(out)


def comment_issue(num, body):
    run(["gh", "issue", "comment", str(num), "-b", body])


def edit_issue_labels(num, remove=None, add=None):
    cmd = ["gh", "issue", "edit", str(num)]
    if remove:
        for lab in remove:
            cmd += ["--remove-label", lab]
    if add:
        for lab in add:
            cmd += ["--add-label", lab]
    run(cmd)


def count_error_comments(num) -> int:
    try:
        out = run(["gh", "issue", "view", str(num), "--json", "comments"])
        data = json.loads(out)
        comments = data.get("comments") or []
        needle = "[agent-bus queue] error"
        return sum(1 for c in comments if (c.get("body") or "").startswith(needle))
    except Exception:
        return 0


def process_task(issue):
    body = (issue.get('body') or '').strip()
    # First line is the command, rest is payload (optional JSON)
    lines = body.splitlines()
    cmd = lines[0].strip() if lines else '/status'
    payload = '\n'.join(lines[1:]) if len(lines) > 1 else ''

    env = os.environ.copy()
    env['AGENT_BUS_COMMAND'] = cmd
    # Optional JSON payload on following lines
    if payload:
        try:
            json.loads(payload)  # validate JSON before passing through
            env['AGENT_BUS_PAYLOAD'] = payload
        except Exception:
            # Pass raw if not valid JSON
            env['AGENT_BUS_PAYLOAD'] = payload
    # Let agent_bus read its TOML and route accordingly
    p = subprocess.Popen(["python3", str(ROOT / 'scripts/agent_bus.py')], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
    out, err = p.communicate()
    ok = (p.returncode == 0)

    result = f"Processed task with command: {cmd}\nReturn code: {p.returncode}\n---\nstdout:\n{out}\n---\nstderr:\n{err}\n"
    comment_issue(issue['number'], f"[agent-bus queue]\n{result}")
    # Move label from agent-task to agent-task-done
    edit_issue_labels(issue['number'], remove=["agent-task"], add=["agent-task-done"]) if ok else None


def main():
    try:
        tasks = list_tasks()
    except Exception as e:
        print(f"queue: failed to list tasks: {e}")
        return 0
    for issue in tasks:
        try:
            process_task(issue)
        except Exception as e:
            num = issue['number']
            comment_issue(num, f"[agent-bus queue] error: {e}")
            # dead-letter after 3 failures
            failures = count_error_comments(num)
            if failures >= 3:
                try:
                    edit_issue_labels(num, remove=["agent-task"], add=["agent-task-failed"])
                except Exception:
                    pass
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
