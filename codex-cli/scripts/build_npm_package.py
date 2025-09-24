#!/usr/bin/env python3
"""Stage and optionally package the @openai/codex npm module."""

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
CODEX_CLI_ROOT = SCRIPT_DIR.parent
REPO_ROOT = CODEX_CLI_ROOT.parent
GITHUB_REPO = "openai/codex"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Build or stage the Codex CLI npm package.")
    parser.add_argument(
        "--version",
        help="Version number to write to package.json inside the staged package.",
    )
    parser.add_argument(
        "--release-version",
        help=(
            "Version to stage for npm release. When provided, the script also resolves the "
            "matching rust-release workflow unless --workflow-url is supplied."
        ),
    )
    parser.add_argument(
        "--workflow-url",
        help="Optional GitHub Actions workflow run URL used to download native binaries.",
    )
    parser.add_argument(
        "--staging-dir",
        type=Path,
        help=(
            "Directory to stage the package contents. Defaults to a new temporary directory "
            "if omitted. The directory must be empty when provided."
        ),
    )
    parser.add_argument(
        "--tmp",
        dest="staging_dir",
        type=Path,
        help=argparse.SUPPRESS,
    )
    parser.add_argument(
        "--pack-output",
        type=Path,
        help="Path where the generated npm tarball should be written.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    version = args.version
    release_version = args.release_version
    if release_version:
        if version and version != release_version:
            raise RuntimeError("--version and --release-version must match when both are provided.")
        version = release_version

    if not version:
        raise RuntimeError("Must specify --version or --release-version.")

    staging_dir, created_temp = prepare_staging_dir(args.staging_dir)

    try:
        stage_sources(staging_dir, version)

        workflow_url = args.workflow_url
        resolved_head_sha: str | None = None
        if not workflow_url:
            if release_version:
                workflow = resolve_release_workflow(version)
                workflow_url = workflow["url"]
                resolved_head_sha = workflow.get("headSha")
            else:
                workflow_url = resolve_latest_alpha_workflow_url()
        elif release_version:
            try:
                workflow = resolve_release_workflow(version)
                resolved_head_sha = workflow.get("headSha")
            except Exception:
                resolved_head_sha = None

        if release_version and resolved_head_sha:
            print(f"should `git checkout {resolved_head_sha}`")

        if not workflow_url:
            raise RuntimeError("Unable to determine workflow URL for native binaries.")

        install_native_binaries(staging_dir, workflow_url)

        if release_version:
            staging_dir_str = str(staging_dir)
            print(
                f"Staged version {version} for release in {staging_dir_str}\n\n"
                "Verify the CLI:\n"
                f"    node {staging_dir_str}/bin/codex.js --version\n"
                f"    node {staging_dir_str}/bin/codex.js --help\n\n"
            )
        else:
            print(f"Staged package in {staging_dir}")

        if args.pack_output is not None:
            output_path = run_npm_pack(staging_dir, args.pack_output)
            print(f"npm pack output written to {output_path}")
    finally:
        if created_temp:
            # Preserve the staging directory for further inspection.
            pass

    return 0


def prepare_staging_dir(staging_dir: Path | None) -> tuple[Path, bool]:
    if staging_dir is not None:
        staging_dir = staging_dir.resolve()
        staging_dir.mkdir(parents=True, exist_ok=True)
        if any(staging_dir.iterdir()):
            raise RuntimeError(f"Staging directory {staging_dir} is not empty.")
        return staging_dir, False

    temp_dir = Path(tempfile.mkdtemp(prefix="codex-npm-stage-"))
    return temp_dir, True


def stage_sources(staging_dir: Path, version: str) -> None:
    bin_dir = staging_dir / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)

    shutil.copy2(CODEX_CLI_ROOT / "bin" / "codex.js", bin_dir / "codex.js")
    rg_manifest = CODEX_CLI_ROOT / "bin" / "rg"
    if rg_manifest.exists():
        shutil.copy2(rg_manifest, bin_dir / "rg")

    readme_src = REPO_ROOT / "README.md"
    if readme_src.exists():
        shutil.copy2(readme_src, staging_dir / "README.md")

    with open(CODEX_CLI_ROOT / "package.json", "r", encoding="utf-8") as fh:
        package_json = json.load(fh)
    package_json["version"] = version

    with open(staging_dir / "package.json", "w", encoding="utf-8") as out:
        json.dump(package_json, out, indent=2)
        out.write("\n")


def install_native_binaries(staging_dir: Path, workflow_url: str | None) -> None:
    cmd = ["./scripts/install_native_deps.py"]
    if workflow_url:
        cmd.extend(["--workflow-url", workflow_url])
    cmd.append(str(staging_dir))
    subprocess.check_call(cmd, cwd=CODEX_CLI_ROOT)


def resolve_latest_alpha_workflow_url() -> str:
    version = determine_latest_alpha_version()
    workflow_url = fetch_workflow_url_for_version(version)
    if not workflow_url:
        raise RuntimeError(f"Unable to locate workflow for version {version}.")
    return workflow_url


def determine_latest_alpha_version() -> str:
    releases = list_releases()
    best_key: tuple[int, int, int, int] | None = None
    best_version: str | None = None
    pattern = re.compile(r"^rust-v(\d+)\.(\d+)\.(\d+)-alpha\.(\d+)$")
    for release in releases:
        tag = release.get("tag_name", "")
        match = pattern.match(tag)
        if not match:
            continue
        key = tuple(int(match.group(i)) for i in range(1, 5))
        if best_key is None or key > best_key:
            best_key = key
            best_version = (
                f"{match.group(1)}.{match.group(2)}.{match.group(3)}-alpha.{match.group(4)}"
            )

    if best_version is None:
        raise RuntimeError("No alpha releases found when resolving workflow URL.")
    return best_version


def list_releases() -> list[dict]:
    stdout = subprocess.check_output(
        ["gh", "api", f"/repos/{GITHUB_REPO}/releases?per_page=100"],
        text=True,
    )
    try:
        releases = json.loads(stdout or "[]")
    except json.JSONDecodeError as exc:
        raise RuntimeError("Unable to parse releases JSON.") from exc
    if not isinstance(releases, list):
        raise RuntimeError("Unexpected response when listing releases.")
    return releases


def fetch_workflow_url_for_version(version: str) -> str | None:
    ref = f"rust-v{version}"
    stdout = subprocess.check_output(
        [
            "gh",
            "run",
            "list",
            "--branch",
            ref,
            "--limit",
            "20",
            "--json",
            "workflowName,url",
        ],
        text=True,
    )

    try:
        runs = json.loads(stdout or "[]")
    except json.JSONDecodeError as exc:
        raise RuntimeError("Unable to parse workflow run listing.") from exc

    for run in runs:
        if run.get("workflowName") == "rust-release":
            url = run.get("url")
            if url:
                return url
    return None


def resolve_release_workflow(version: str) -> dict:
    stdout = subprocess.check_output(
        [
            "gh",
            "run",
            "list",
            "--branch",
            f"rust-v{version}",
            "--json",
            "workflowName,url,headSha",
            "--jq",
            'first(.[] | select(.workflowName == "rust-release"))',
        ],
        text=True,
    )
    workflow = json.loads(stdout)
    if not workflow:
        raise RuntimeError(f"Unable to find rust-release workflow for version {version}.")
    return workflow


def run_npm_pack(staging_dir: Path, output_path: Path) -> Path:
    output_path = output_path.resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory(prefix="codex-npm-pack-") as pack_dir_str:
        pack_dir = Path(pack_dir_str)
        stdout = subprocess.check_output(
            ["npm", "pack", "--json", "--pack-destination", str(pack_dir)],
            cwd=staging_dir,
            text=True,
        )
        try:
            pack_output = json.loads(stdout)
        except json.JSONDecodeError as exc:
            raise RuntimeError("Failed to parse npm pack output.") from exc

        if not pack_output:
            raise RuntimeError("npm pack did not produce an output tarball.")

        tarball_name = pack_output[0].get("filename") or pack_output[0].get("name")
        if not tarball_name:
            raise RuntimeError("Unable to determine npm pack output filename.")

        tarball_path = pack_dir / tarball_name
        if not tarball_path.exists():
            raise RuntimeError(f"Expected npm pack output not found: {tarball_path}")

        shutil.move(str(tarball_path), output_path)

    return output_path


if __name__ == "__main__":
    import sys

    sys.exit(main())
