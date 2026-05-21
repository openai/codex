#!/usr/bin/env python3
"""Install Codex package archives and native helper binaries."""

import argparse
from contextlib import contextmanager
import os
import shutil
import subprocess
import tarfile
import tempfile
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Sequence

SCRIPT_DIR = Path(__file__).resolve().parent
CODEX_CLI_ROOT = SCRIPT_DIR.parent
DEFAULT_WORKFLOW_URL = "https://github.com/openai/codex/actions/runs/26201494185"  # rust-v0.133.0-alpha.4
GITHUB_REPO = "openai/codex"
VENDOR_DIR_NAME = "vendor"
BINARY_TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)
CODEX_PACKAGE_COMPONENT = "codex-package"


@dataclass(frozen=True)
class BinaryComponent:
    artifact_prefix: str  # matches the artifact filename prefix (e.g. codex-<target>.zst)
    dest_dir: str  # directory under vendor/<target>/ where the binary is installed
    binary_basename: str  # executable name inside dest_dir (before optional .exe)


@dataclass(frozen=True)
class WorkflowArtifact:
    name: str
    size_in_bytes: int


BINARY_COMPONENTS = {
    "codex-responses-api-proxy": BinaryComponent(
        artifact_prefix="codex-responses-api-proxy",
        dest_dir="codex-responses-api-proxy",
        binary_basename="codex-responses-api-proxy",
    ),
}


def _gha_enabled() -> bool:
    # GitHub Actions supports "workflow commands" (e.g. ::group:: / ::error::) that make logs
    # much easier to scan: groups collapse noisy sections and error annotations surface the
    # failure in the UI without changing the actual exception/traceback output.
    return os.environ.get("GITHUB_ACTIONS") == "true"


def _gha_escape(value: str) -> str:
    # Workflow commands require percent/newline escaping.
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


def _gha_error(*, title: str, message: str) -> None:
    # Emit a GitHub Actions error annotation. This does not replace stdout/stderr logs; it just
    # adds a prominent summary line to the job UI so the root cause is easier to spot.
    if not _gha_enabled():
        return
    print(
        f"::error title={_gha_escape(title)}::{_gha_escape(message)}",
        flush=True,
    )


@contextmanager
def _gha_group(title: str):
    # Wrap a block in a collapsible log group on GitHub Actions. Outside of GHA this is a no-op
    # so local output remains unchanged.
    if _gha_enabled():
        print(f"::group::{_gha_escape(title)}", flush=True)
    try:
        yield
    finally:
        if _gha_enabled():
            print("::endgroup::", flush=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Install native Codex binaries.")
    parser.add_argument(
        "--workflow-url",
        help=(
            "GitHub Actions workflow URL that produced the artifacts. Defaults to a "
            "known good run when omitted."
        ),
    )
    parser.add_argument(
        "--component",
        dest="components",
        action="append",
        choices=tuple([CODEX_PACKAGE_COMPONENT, *BINARY_COMPONENTS]),
        help=(
            "Limit installation to the specified components."
            " May be repeated. Defaults to codex-package and codex-responses-api-proxy."
        ),
    )
    parser.add_argument(
        "--artifacts-dir",
        type=Path,
        help=(
            "Directory used to cache downloaded workflow artifacts. Defaults to a "
            "temporary directory."
        ),
    )
    parser.add_argument(
        "root",
        nargs="?",
        type=Path,
        help=(
            "Directory containing package.json for the staged package. If omitted, the "
            "repository checkout is used."
        ),
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()

    codex_cli_root = (args.root or CODEX_CLI_ROOT).resolve()
    vendor_dir = codex_cli_root / VENDOR_DIR_NAME
    vendor_dir.mkdir(parents=True, exist_ok=True)

    components = args.components or [CODEX_PACKAGE_COMPONENT, "codex-responses-api-proxy"]

    workflow_override = (args.workflow_url or "").strip()
    workflow_url = workflow_override or DEFAULT_WORKFLOW_URL

    workflow_id = workflow_url.rstrip("/").split("/")[-1]
    print(f"Downloading native artifacts from workflow {workflow_id}...", flush=True)

    with _gha_group(f"Download native artifacts from workflow {workflow_id}"):
        if args.artifacts_dir is not None:
            artifacts_dir = args.artifacts_dir.resolve()
            artifacts_dir.mkdir(parents=True, exist_ok=True)
            install_from_workflow_artifacts(workflow_id, artifacts_dir, components, vendor_dir)
        else:
            with tempfile.TemporaryDirectory(prefix="codex-native-artifacts-") as artifacts_dir_str:
                artifacts_dir = Path(artifacts_dir_str)
                install_from_workflow_artifacts(
                    workflow_id,
                    artifacts_dir,
                    components,
                    vendor_dir,
                )

    print(f"Installed native dependencies into {vendor_dir}", flush=True)
    return 0


def install_from_workflow_artifacts(
    workflow_id: str,
    artifacts_dir: Path,
    components: Sequence[str],
    vendor_dir: Path,
) -> None:
    artifact_names = select_target_artifacts(workflow_id, components)
    _download_artifacts(workflow_id, artifacts_dir, artifact_names)
    if CODEX_PACKAGE_COMPONENT in components:
        install_codex_package_archives(artifacts_dir, vendor_dir, BINARY_TARGETS)
    install_binary_components(
        artifacts_dir,
        vendor_dir,
        [BINARY_COMPONENTS[name] for name in components if name in BINARY_COMPONENTS],
    )


def select_target_artifacts(
    workflow_id: str,
    components: Sequence[str],
) -> list[WorkflowArtifact]:
    needs_target_artifacts = CODEX_PACKAGE_COMPONENT in components or any(
        component in BINARY_COMPONENTS for component in components
    )
    if not needs_target_artifacts:
        return []

    artifacts_by_name = {
        artifact.name: artifact for artifact in list_workflow_artifacts(workflow_id)
    }
    selected_artifacts: list[WorkflowArtifact] = []
    for target in BINARY_TARGETS:
        for artifact_name in [target, f"{target}-unsigned"]:
            artifact = artifacts_by_name.get(artifact_name)
            if artifact is not None:
                selected_artifacts.append(artifact)
                break
        else:
            raise FileNotFoundError(
                f"Expected workflow artifact not found for target {target}"
            )

    return selected_artifacts


def list_workflow_artifacts(workflow_id: str) -> list[WorkflowArtifact]:
    stdout = subprocess.check_output(
        [
            "gh",
            "api",
            f"repos/{GITHUB_REPO}/actions/runs/{workflow_id}/artifacts",
            "--paginate",
            "--jq",
            ".artifacts[] | [.name, .size_in_bytes] | @tsv",
        ],
        text=True,
    )
    artifacts: list[WorkflowArtifact] = []
    for line in stdout.splitlines():
        name, size_in_bytes = line.split("\t", 1)
        artifacts.append(WorkflowArtifact(name=name, size_in_bytes=int(size_in_bytes)))
    return artifacts


def install_codex_package_archives(
    artifacts_dir: Path,
    vendor_dir: Path,
    targets: Sequence[str],
) -> None:
    targets = list(targets)
    if not targets:
        return

    print(
        "Installing Codex package archives for targets: " + ", ".join(targets),
        flush=True,
    )
    max_workers = min(len(targets), max(1, (os.cpu_count() or 1)))
    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(
                _install_single_codex_package_archive,
                artifacts_dir,
                vendor_dir,
                target,
            ): target
            for target in targets
        }
        for future in as_completed(futures):
            installed_path = future.result()
            print(f"  installed {installed_path}", flush=True)


def _install_single_codex_package_archive(
    artifacts_dir: Path,
    vendor_dir: Path,
    target: str,
) -> Path:
    artifact_subdir = artifact_dir_for_target(artifacts_dir, target)
    archive_path = artifact_subdir / f"codex-package-{target}.tar.gz"
    if not archive_path.exists():
        raise FileNotFoundError(f"Expected package archive not found: {archive_path}")

    dest_dir = vendor_dir / target
    if dest_dir.exists():
        shutil.rmtree(dest_dir)
    dest_dir.mkdir(parents=True, exist_ok=True)

    with tarfile.open(archive_path, "r:gz") as archive:
        archive.extractall(dest_dir, filter="data")

    return dest_dir


def _download_artifacts(
    workflow_id: str,
    dest_dir: Path,
    artifacts: Sequence[WorkflowArtifact],
) -> None:
    total_bytes = sum(artifact.size_in_bytes for artifact in artifacts)
    print(
        f"Downloading {len(artifacts)} artifacts ({format_bytes(total_bytes)})",
        flush=True,
    )
    for artifact in artifacts:
        artifact_dir = dest_dir / artifact.name
        if artifact_dir.is_dir() and any(artifact_dir.iterdir()):
            print(
                f"  using cached {artifact.name} ({format_bytes(artifact.size_in_bytes)})",
                flush=True,
            )
            continue

        artifact_dir.mkdir(parents=True, exist_ok=True)
        print(
            f"  downloading {artifact.name} ({format_bytes(artifact.size_in_bytes)})",
            flush=True,
        )
        cmd = [
            "gh",
            "run",
            "download",
            "--name",
            artifact.name,
            "--dir",
            str(artifact_dir),
            "--repo",
            GITHUB_REPO,
            workflow_id,
        ]
        subprocess.check_call(cmd)


def install_binary_components(
    artifacts_dir: Path,
    vendor_dir: Path,
    selected_components: Sequence[BinaryComponent],
) -> None:
    if not selected_components:
        return

    for component in selected_components:
        component_targets = list(BINARY_TARGETS)

        print(
            f"Installing {component.binary_basename} binaries for targets: "
            + ", ".join(component_targets),
            flush=True,
        )
        max_workers = min(len(component_targets), max(1, (os.cpu_count() or 1)))
        with ThreadPoolExecutor(max_workers=max_workers) as executor:
            futures = {
                executor.submit(
                    _install_single_binary,
                    artifacts_dir,
                    vendor_dir,
                    target,
                    component,
                ): target
                for target in component_targets
            }
            for future in as_completed(futures):
                installed_path = future.result()
                print(f"  installed {installed_path}", flush=True)


def format_bytes(size_in_bytes: int) -> str:
    value = float(size_in_bytes)
    for unit in ["B", "KiB", "MiB"]:
        if value < 1024:
            return f"{value:.1f} {unit}"
        value /= 1024
    return f"{value:.1f} GiB"


def _install_single_binary(
    artifacts_dir: Path,
    vendor_dir: Path,
    target: str,
    component: BinaryComponent,
) -> Path:
    artifact_subdir = artifact_dir_for_target(artifacts_dir, target)
    archive_path = binary_archive_path(artifact_subdir, component.artifact_prefix, target)

    dest_dir = vendor_dir / target / component.dest_dir
    dest_dir.mkdir(parents=True, exist_ok=True)

    binary_name = (
        f"{component.binary_basename}.exe" if "windows" in target else component.binary_basename
    )
    dest = dest_dir / binary_name
    dest.unlink(missing_ok=True)
    extract_zstd_archive(archive_path, dest)
    if "windows" not in target:
        dest.chmod(0o755)
    return dest


def _archive_name_for_target(artifact_prefix: str, target: str) -> str:
    if "windows" in target:
        return f"{artifact_prefix}-{target}.exe.zst"
    return f"{artifact_prefix}-{target}.zst"


def binary_archive_path(artifact_dir: Path, artifact_prefix: str, target: str) -> Path:
    archive_names = [_archive_name_for_target(artifact_prefix, target)]
    if artifact_dir.name == f"{target}-unsigned":
        archive_names.append(_archive_name_for_target(artifact_prefix, f"{target}-unsigned"))

    for archive_name in archive_names:
        archive_path = artifact_dir / archive_name
        if archive_path.exists():
            return archive_path

    raise FileNotFoundError(f"Expected artifact not found: {artifact_dir / archive_names[0]}")


def artifact_dir_for_target(artifacts_dir: Path, target: str) -> Path:
    for artifact_name in [target, f"{target}-unsigned"]:
        artifact_dir = artifacts_dir / artifact_name
        if artifact_dir.is_dir():
            return artifact_dir

    return artifacts_dir / target


def extract_zstd_archive(archive_path: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)

    output_path = archive_path.parent / dest.name
    subprocess.check_call(["zstd", "-f", "-d", str(archive_path), "-o", str(output_path)])
    shutil.move(str(output_path), dest)


if __name__ == "__main__":
    import sys

    sys.exit(main())
