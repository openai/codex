#!/usr/bin/env python3

from __future__ import annotations

import os
import sys

from wrapper_common import (
    build_final_args,
    exec_command,
    fetch_packaged_entrypoint,
    find_packaged_cargo_dylint,
    normalize_packaged_library,
    parse_wrapper_args,
    prefer_rustup_shims,
    repo_root,
    set_default_lint_env,
)


def main() -> "Never":
    root = repo_root()
    parsed = parse_wrapper_args(sys.argv[1:])
    final_args = build_final_args(parsed, root / "codex-rs" / "Cargo.toml")

    env = os.environ.copy()
    prefer_rustup_shims(env)
    set_default_lint_env(env)

    package_entrypoint = fetch_packaged_entrypoint(
        root / "tools" / "argument-comment-lint" / "argument-comment-lint",
        env,
    )
    cargo_dylint = find_packaged_cargo_dylint(package_entrypoint)
    library_path = normalize_packaged_library(package_entrypoint)

    command = [str(cargo_dylint), "dylint", "--lib-path", str(library_path)]
    if not parsed.has_library_selection:
        command.append("--all")
    command.extend(final_args)
    exec_command(command, env)


if __name__ == "__main__":
    main()
