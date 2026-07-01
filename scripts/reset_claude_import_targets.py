#!/usr/bin/env python3

"""Reset selected Codex-side targets created by Claude Code import.

This is a developer helper for retesting the external-agent migration flow. It
never reads from or mutates Claude's source directories. By default it only
prints the changes it would make; pass --apply to mutate Codex targets.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path


MCP_TABLE_HEADER = re.compile(r"^\s*\[\[?\s*mcp_servers(?:\s*\.|\s*\])")
TABLE_HEADER = re.compile(r"^\s*\[\[?.*\]\]?\s*(?:#.*)?$")
ROOT_MCP_ASSIGNMENT = re.compile(r"^\s*mcp_servers(?:\s*\.|\s*=)")


@dataclass(frozen=True)
class ResetTarget:
    label: str
    config_toml: Path
    skills_dir: Path
    hooks_json: Path


def strip_mcp_servers(text: str) -> tuple[str, bool]:
    """Remove generated [mcp_servers...] TOML tables while preserving other text."""
    kept_lines: list[str] = []
    skipping_mcp_table = False
    before_first_table = True
    removed = False

    for line in text.splitlines(keepends=True):
        if TABLE_HEADER.match(line):
            before_first_table = False
            if MCP_TABLE_HEADER.match(line):
                skipping_mcp_table = True
                removed = True
                continue
            skipping_mcp_table = False

        if skipping_mcp_table:
            continue

        # The migration writer emits table headers, but handle an inline or
        # dotted root assignment too so the reset means "all MCP servers".
        if before_first_table and ROOT_MCP_ASSIGNMENT.match(line):
            removed = True
            continue

        kept_lines.append(line)

    return "".join(kept_lines), removed


def resolve_repo_root(explicit_repo_root: Path | None) -> Path | None:
    if explicit_repo_root is not None:
        return explicit_repo_root.expanduser().resolve()

    result = subprocess.run(
        ["git", "rev-parse", "--show-toplevel"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        return None
    return Path(result.stdout.strip()).resolve()


def backup_path(path: Path, timestamp: str) -> Path:
    base = path.with_name(f"{path.name}.before-claude-import-reset-{timestamp}.bak")
    if not base.exists():
        return base

    counter = 2
    while True:
        candidate = base.with_name(f"{base.name}.{counter}")
        if not candidate.exists():
            return candidate
        counter += 1


def move_to_backup(path: Path, timestamp: str, apply: bool) -> None:
    destination = backup_path(path, timestamp)
    verb = "moving" if apply else "would move"
    print(f"  {verb} {path} -> {destination}")
    if apply:
        shutil.move(path, destination)


def reset_config(path: Path, timestamp: str, apply: bool) -> None:
    if not path.is_file():
        print(f"  no config.toml at {path}")
        return

    original = path.read_text(encoding="utf-8")
    updated, removed = strip_mcp_servers(original)
    if not removed:
        print(f"  no MCP server tables in {path}")
        return

    backup = backup_path(path, timestamp)
    verb = "removing" if apply else "would remove"
    print(f"  {verb} MCP server tables from {path}")
    print(f"  {'backing up' if apply else 'would back up'} {path} -> {backup}")
    if apply:
        shutil.copy2(path, backup)
        path.write_text(updated, encoding="utf-8")


def reset_target(target: ResetTarget, timestamp: str, apply: bool) -> None:
    print(f"{target.label}:")
    reset_config(target.config_toml, timestamp, apply)

    if target.skills_dir.exists():
        if not target.skills_dir.is_dir() and not target.skills_dir.is_symlink():
            raise RuntimeError(f"skills target is not a directory: {target.skills_dir}")
        move_to_backup(target.skills_dir, timestamp, apply)
    else:
        print(f"  no skills directory at {target.skills_dir}")

    if target.hooks_json.exists():
        if target.hooks_json.is_dir():
            raise RuntimeError(f"hooks target is a directory: {target.hooks_json}")
        move_to_backup(target.hooks_json, timestamp, apply)
    else:
        print(f"  no hooks.json at {target.hooks_json}")


def home_target(codex_home: Path) -> ResetTarget:
    return ResetTarget(
        label="home-scoped import targets",
        config_toml=codex_home / "config.toml",
        skills_dir=codex_home.parent / ".agents" / "skills",
        hooks_json=codex_home / "hooks.json",
    )


def repo_target(repo_root: Path) -> ResetTarget:
    return ResetTarget(
        label="repo-scoped import targets",
        config_toml=repo_root / ".codex" / "config.toml",
        skills_dir=repo_root / ".agents" / "skills",
        hooks_json=repo_root / ".codex" / "hooks.json",
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Reset Codex-side MCP servers, skills, and hooks.json created while "
            "testing Claude Code import. Defaults to a dry run."
        )
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply the reset. Without this flag, only print planned changes.",
    )
    parser.add_argument(
        "--scope",
        choices=("all", "home", "repo"),
        default="all",
        help="Which import targets to reset. Default: all.",
    )
    parser.add_argument(
        "--codex-home",
        type=Path,
        default=Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")),
        help="Codex home for home-scoped targets. Default: $CODEX_HOME or ~/.codex.",
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        help="Repo root for repo-scoped targets. Default: current git worktree root.",
    )
    args = parser.parse_args()

    codex_home = args.codex_home.expanduser().resolve()
    repo_root = resolve_repo_root(args.repo_root)
    targets: list[ResetTarget] = []

    if args.scope in ("all", "home"):
        targets.append(home_target(codex_home))
    if args.scope in ("all", "repo"):
        if repo_root is None:
            parser.error(
                "--scope includes repo, but no git worktree or --repo-root was found"
            )
        targets.append(repo_target(repo_root))

    timestamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    if not args.apply:
        print("dry run: pass --apply to perform these changes")
    for target in targets:
        reset_target(target, timestamp, args.apply)

    return 0


if __name__ == "__main__":
    sys.exit(main())
