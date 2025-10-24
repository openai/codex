#!/usr/bin/env python3

"""
Download a release artifact for the npm package and publish it.

Given a release version like `0.20.0`, this script:
  - Downloads the `codex-npm-<version>.tgz` asset from the GitHub release
    tagged `rust-v<version>` in the `openai/codex` repository using `gh`.
  - Runs `npm publish` on the downloaded tarball to publish `@openai/codex`.

Flags:
  - `--dry-run` delegates to `npm publish --dry-run`. The artifact is still
    downloaded so npm can inspect the archive contents without publishing.

Requirements:
  - GitHub CLI (`gh`) must be installed and authenticated to access the repo.
  - npm must be logged in with an account authorized to publish
    `@openai/codex`. This may trigger a browser for 2FA.
"""

import argparse
import os
import subprocess
import sys
import tempfile
from pathlib import Path


def run_checked(cmd: list[str], cwd: Path | None = None) -> None:
    """Run a subprocess command and raise if it fails."""
    proc = subprocess.run(cmd, cwd=str(cwd) if cwd else None)
    proc.check_returncode()


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Download the npm release artifact for a given version and publish it."
        )
    )
    parser.add_argument(
        "version",
        help="Release version to publish, e.g. 0.20.0 (without the 'v' prefix)",
    )
    parser.add_argument(
        "--dir",
        type=Path,
        help=(
            "Optional directory to download the artifact into. Defaults to a temporary directory."
        ),
    )
    parser.add_argument(
        "-n",
        "--dry-run",
        action="store_true",
        help="Delegate to `npm publish --dry-run` (still downloads the artifact).",
    )
    args = parser.parse_args()

    version: str = args.version.lstrip("v")
    tag = f"rust-v{version}"
    asset_name = f"codex-npm-{version}.tgz"

    download_dir_context_manager = (
        tempfile.TemporaryDirectory() if args.dir is None else None
    )
    # Use provided dir if set, else the temporary one created above
    download_dir: Path = args.dir if args.dir else Path(download_dir_context_manager.name)  # type: ignore[arg-type]
    download_dir.mkdir(parents=True, exist_ok=True)

    # 1) Download the artifact using gh
    repo = "openai/codex"
    gh_cmd = [
        "gh",
        "release",
        "download",
        tag,
        "--repo",
        repo,
        "--pattern",
        asset_name,
        "--dir",
        str(download_dir),
    ]
    print(f"Downloading {asset_name} from {repo}@{tag} into {download_dir}...")
    # Even in --dry-run we download so npm can inspect the tarball.
    run_checked(gh_cmd)

    artifact_path = download_dir / asset_name
    if not args.dry_run and not artifact_path.is_file():
        print(
            f"Error: expected artifact not found after download: {artifact_path}",
            file=sys.stderr,
        )
        return 1

    # 2) Publish to npm
    npm_cmd = ["npm", "publish"]
    if args.dry_run:
        npm_cmd.append("--dry-run")
    npm_cmd.append(str(artifact_path))

    # Ensure CI is unset so npm can open a browser for 2FA if needed.
    env = os.environ.copy()
    if env.get("CI"):
        env.pop("CI")

    print("Running:", " ".join(npm_cmd))
    proc = subprocess.run(npm_cmd, env=env)
    proc.check_returncode()

    print("Publish complete.")
    # Keep the temporary directory alive until here; it is cleaned up on exit
    return 0


if __name__ == "__main__":
    sys.exit(main())
