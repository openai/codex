#!/usr/bin/env python3

from __future__ import annotations

import os
import sys

from wrapper_common import (
    build_final_args,
    ensure_source_prerequisites,
    exec_command,
    parse_wrapper_args,
    repo_root,
    set_default_lint_env,
)


def main() -> "Never":
    root = repo_root()
    parsed = parse_wrapper_args(sys.argv[1:])
    final_args = build_final_args(parsed, root / "codex-rs" / "Cargo.toml")

    env = os.environ.copy()
    ensure_source_prerequisites(env)
    set_default_lint_env(env)

    command = ["cargo", "dylint", "--path", str(root / "tools" / "argument-comment-lint")]
    if not parsed.has_library_selection:
        command.append("--all")
    command.extend(final_args)
    exec_command(command, env)


if __name__ == "__main__":
    main()
