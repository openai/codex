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


def _env_with_token(gh_token: str | None) -> dict:
    env = os.environ.copy()
    if gh_token:
        env["GH_TOKEN"] = gh_token
    return env


def pr_review(repo: str, pr: int, action: str, body: str | None, gh_token: str | None) -> None:
    """
    Submit a formal PR review upstream.
    action: "approve" | "request_changes" | "comment"
    """
    env = _env_with_token(gh_token)
    args = ["pr", "review", str(pr), "-R", repo]
    action_norm = (action or "").lower()
    if action_norm == "approve":
        args += ["--approve"]
    elif action_norm in ("request_changes", "request-changes", "changes", "decline", "defer"):
        args += ["--request-changes"]
    else:
        args += ["--comment"]
    if body:
        args += ["-b", body]
    run_gh(args, env)


def rerun_latest_actions_run(repo: str, pr: int, gh_token: str | None) -> None:
    """
    Rerun the most recent GitHub Actions run for the PR's head branch filtered to event=pull_request.
    Requires a token with workflow scope on the upstream repository (UPSTREAM_GH_TOKEN).
    """
    env = _env_with_token(gh_token)
    # Get PR head branch
    _, pr_json = run_gh([
        "pr", "view", str(pr), "-R", repo,
        "--json", "headRefName"
    ], env)
    try:
        head_ref = json.loads(pr_json).get("headRefName")
    except Exception as e:
        raise RuntimeError(f"failed to parse PR head ref: {e}\n{pr_json}")
    if not head_ref:
        raise RuntimeError("missing headRefName for PR")

    # List most recent runs for that branch and event=pull_request
    runs_cmd = [
        "api", "-X", "GET",
        f"/repos/{repo}/actions/runs",
        "-F", f"branch={head_ref}",
        "-F", "event=pull_request",
        "-F", "per_page=1"
    ]
    _, runs_out = run_gh(runs_cmd, env)
    try:
        runs = json.loads(runs_out).get("workflow_runs") or []
    except Exception as e:
        raise RuntimeError(f"failed to parse runs: {e}\n{runs_out}")
    if not runs:
        pr_comment(repo, pr, "[agent-bus] No recent runs found to rerun.", gh_token)
        return
    run_id = runs[0].get("id")
    if not run_id:
        pr_comment(repo, pr, "[agent-bus] Could not identify a run id to rerun.", gh_token)
        return

    # Rerun this workflow run
    rerun_cmd = ["api", "-X", "POST", f"/repos/{repo}/actions/runs/{run_id}/rerun"]
    run_gh(rerun_cmd, env)
    pr_comment(repo, pr, f"[agent-bus] Requested rerun of latest run on branch '{head_ref}' (run id {run_id}).", gh_token)


def rerun_placeholder(repo: str, pr: int, gh_token: str | None) -> None:
    # Backward-compat name: actually rerun the most recent Actions run
    rerun_latest_actions_run(repo, pr, gh_token)
