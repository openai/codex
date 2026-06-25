#!/usr/bin/env python3

import json
import os
import subprocess
import sys
from collections.abc import Callable
from collections.abc import Mapping
from pathlib import Path
from typing import Any


# TODO(anp): Extend this lint workflow to cover server-sent replies.
SCHEMA_PATH = "codex-rs/app-server-protocol/schema/json/ClientRequest.json"
KNOWN_BREAKAGES_PATH = "codex-rs/app-server-protocol/stable-api-breakages.toml"
DEFAULT_BASE_REF = "origin/main"
EMPTY_KNOWN_BREAKAGES = "version = 1\n"
SCHEMA_EVOLUTION_TARGET = "//codex-rs/schema-evolution:codex-schema-evolution"

GitRunner = Callable[..., str]


def run_git(root: Path, *args: str) -> str:
    process = subprocess.run(
        ["git", "-C", str(root), *args],
        check=False,
        capture_output=True,
        encoding="utf-8",
    )
    if process.returncode != 0:
        detail = process.stderr.strip() or "no error output"
        raise RuntimeError(f"git {' '.join(args)} failed: {detail}")
    return process.stdout


def find_repo_root() -> Path:
    process = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=False,
        capture_output=True,
        encoding="utf-8",
    )
    if process.returncode != 0:
        detail = process.stderr.strip() or "no error output"
        raise RuntimeError(f"git rev-parse failed: {detail}")
    return Path(process.stdout.strip())


def schema_base_ref(environ: Mapping[str, str] = os.environ) -> str:
    return environ.get("CODEX_SCHEMA_BASE_REF", "").strip() or DEFAULT_BASE_REF


def build_lint_input(
    root: Path,
    base_ref: str,
    *,
    git: GitRunner = run_git,
) -> dict[str, Any]:
    base = git(root, "merge-base", "HEAD", base_ref).strip()
    if not base:
        raise RuntimeError(f"git merge-base HEAD {base_ref} returned no revision")

    baseline_log_path = git(
        root,
        "ls-tree",
        "--name-only",
        base,
        "--",
        KNOWN_BREAKAGES_PATH,
    ).strip()
    before_known_breakages = EMPTY_KNOWN_BREAKAGES
    if baseline_log_path == KNOWN_BREAKAGES_PATH:
        before_known_breakages = git(root, "show", f"{base}:{KNOWN_BREAKAGES_PATH}")

    return {
        "before": json.loads(git(root, "show", f"{base}:{SCHEMA_PATH}")),
        "after": json.loads((root / SCHEMA_PATH).read_text(encoding="utf-8")),
        "beforeKnownBreakages": before_known_breakages,
        "afterKnownBreakages": (root / KNOWN_BREAKAGES_PATH).read_text(
            encoding="utf-8"
        ),
    }


def run_schema_evolution(root: Path, payload: Mapping[str, Any]) -> int:
    process = subprocess.run(
        ["bazel", "run", SCHEMA_EVOLUTION_TARGET],
        check=False,
        cwd=root,
        input=json.dumps(payload, separators=(",", ":")) + "\n",
        encoding="utf-8",
    )
    return process.returncode


def main() -> int:
    try:
        root = find_repo_root()
        payload = build_lint_input(root, schema_base_ref())
        return run_schema_evolution(root, payload)
    except (OSError, RuntimeError, ValueError) as error:
        print(f"app-server schema lint failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
