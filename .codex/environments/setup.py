#!/usr/bin/env python3

"""Set up ignored files that should be shared with Codex worktrees."""

import shutil
import subprocess
from functools import cache
from pathlib import Path


@cache
def worktree_paths() -> tuple[Path, Path]:
    script_dir = Path(__file__).resolve().parent
    worktree_root = git_path(script_dir / "../..", "--show-toplevel")
    common_git_dir = git_path(worktree_root, "--git-common-dir")
    return worktree_root, common_git_dir.parent


def git_path(working_directory: Path, argument: str) -> Path:
    output = subprocess.check_output(
        [
            "git",
            "-C",
            str(working_directory),
            "rev-parse",
            "--path-format=absolute",
            argument,
        ],
        text=True,
    )
    return Path(output.strip())


def copy_from_main_worktree_to_worktree(repo_relative_path: str) -> None:
    relative_path = Path(repo_relative_path)
    if relative_path.is_absolute() or ".." in relative_path.parts:
        raise ValueError(f"path must be repository-relative: {repo_relative_path}")

    worktree_root, main_worktree = worktree_paths()
    source_path = main_worktree / relative_path
    destination_path = worktree_root / relative_path

    print(f"  source: {source_path}")
    print(f"  destination: {destination_path}")

    if source_path == destination_path:
        print("  result: running in the main worktree; nothing to copy")
    elif destination_path.exists():
        print("  result: destination already exists; nothing to copy")
    elif not source_path.is_file():
        print("  result: source does not exist; nothing to copy")
    else:
        destination_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source_path, destination_path)
        print(f"  result: copied {repo_relative_path}")


def main() -> None:
    print("Codex environment setup:")
    # See codex-rs/docs/bazel.md for the repository's Bazel workflow.
    copy_from_main_worktree_to_worktree("user.bazelrc")


if __name__ == "__main__":
    main()
