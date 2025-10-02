import json
import os
import subprocess
from typing import Dict, Any


def run_gh(args: list[str], env: dict | None = None) -> tuple[int, str]:
    p = subprocess.Popen(["gh", *args], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=env)
    out, err = p.communicate()
    rc = p.returncode
    if rc != 0:
        raise RuntimeError(f"gh {' '.join(args)} failed: {rc}\n{err}\n{out}")
    return rc, out


def pr_status(repo: str, pr: int, gh_token: str | None) -> Dict[str, Any]:
    env = os.environ.copy()
    if gh_token:
        env["GH_TOKEN"] = gh_token
    _, out = run_gh(["pr", "view", str(pr), "-R", repo, "--json",
                    "number,title,state,isDraft,mergeable,reviewDecision,updatedAt,url,statusCheckRollup"] , env)
    return json.loads(out)


def pr_comment(repo: str, pr: int, body: str, gh_token: str | None) -> None:
    env = os.environ.copy()
    if gh_token:
        env["GH_TOKEN"] = gh_token
    run_gh(["pr", "comment", str(pr), "-R", repo, "-b", body], env)


def rerun_placeholder(repo: str, pr: int, gh_token: str | None) -> None:
    # Placeholder: post a message asking maintainers to rerun, or use gh run when allowed.
    pr_comment(repo, pr, "[agent-bus] Requesting CI rerun.", gh_token)

