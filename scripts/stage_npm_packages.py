#!/usr/bin/env python3
"""Stage one or more Codex npm packages for release."""

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
from contextlib import contextmanager
from dataclasses import dataclass
import importlib.util
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path
from typing import Sequence


REPO_ROOT = Path(__file__).resolve().parent.parent
BUILD_SCRIPT = REPO_ROOT / "codex-cli" / "scripts" / "build_npm_package.py"
WORKFLOW_NAME = ".github/workflows/rust-release.yml"
GITHUB_REPO = "openai/codex"
BINARY_TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)

_SPEC = importlib.util.spec_from_file_location("codex_build_npm_package", BUILD_SCRIPT)
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {BUILD_SCRIPT}")
_BUILD_MODULE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_BUILD_MODULE)
PACKAGE_NATIVE_COMPONENTS = getattr(_BUILD_MODULE, "PACKAGE_NATIVE_COMPONENTS", {})
PACKAGE_EXPANSIONS = getattr(_BUILD_MODULE, "PACKAGE_EXPANSIONS", {})
CODEX_PLATFORM_PACKAGES = getattr(_BUILD_MODULE, "CODEX_PLATFORM_PACKAGES", {})
CODEX_PACKAGE_COMPONENT = getattr(
    _BUILD_MODULE, "CODEX_PACKAGE_COMPONENT", "codex-package"
)


@dataclass(frozen=True)
class BinaryComponent:
    artifact_prefix: str
    dest_dir: str
    binary_basename: str


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
    return os.environ.get("GITHUB_ACTIONS") == "true"


def _gha_escape(value: str) -> str:
    return value.replace("%", "%25").replace("\r", "%0D").replace("\n", "%0A")


@contextmanager
def _gha_group(title: str):
    if _gha_enabled():
        print(f"::group::{_gha_escape(title)}", flush=True)
    try:
        yield
    finally:
        if _gha_enabled():
            print("::endgroup::", flush=True)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version to stage (e.g. 0.1.0 or 0.1.0-alpha.1).",
    )
    parser.add_argument(
        "--package",
        dest="packages",
        action="append",
        required=True,
        help="Package name to stage. May be provided multiple times.",
    )
    parser.add_argument(
        "--workflow-url",
        help="Optional workflow URL to reuse for native artifacts.",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=None,
        help="Directory where npm tarballs should be written (default: dist/npm).",
    )
    parser.add_argument(
        "--keep-staging-dirs",
        action="store_true",
        help="Retain temporary staging directories instead of deleting them.",
    )
    return parser.parse_args()


def native_components_for_package(package: str) -> tuple[str, ...]:
    return tuple(sorted(PACKAGE_NATIVE_COMPONENTS.get(package, [])))


def collect_native_component_sets(packages: list[str]) -> list[tuple[str, ...]]:
    component_sets: list[tuple[str, ...]] = []
    seen: set[tuple[str, ...]] = set()
    for package in packages:
        components = native_components_for_package(package)
        if not components or components in seen:
            continue
        seen.add(components)
        component_sets.append(components)
    return component_sets


def expand_packages(packages: list[str]) -> list[str]:
    expanded: list[str] = []
    for package in packages:
        for expanded_package in PACKAGE_EXPANSIONS.get(package, [package]):
            if expanded_package in expanded:
                continue
            expanded.append(expanded_package)
    return expanded


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
            "--workflow",
            WORKFLOW_NAME,
            "--jq",
            "first(.[])",
        ],
        cwd=REPO_ROOT,
        text=True,
    )
    workflow = json.loads(stdout or "null")
    if not workflow:
        raise RuntimeError(
            f"Unable to find rust-release workflow for version {version}."
        )
    return workflow


def resolve_workflow_url(version: str, override: str | None) -> tuple[str, str | None]:
    if override:
        return override, None

    workflow = resolve_release_workflow(version)
    return workflow["url"], workflow.get("headSha")


def install_native_components(
    workflow_url: str,
    components: set[str],
    vendor_root: Path,
    artifacts_dir: Path,
) -> None:
    if not components:
        return

    vendor_dir = vendor_root / "vendor"
    vendor_dir.mkdir(parents=True, exist_ok=True)

    workflow_id = workflow_url.rstrip("/").split("/")[-1]
    print(f"Downloading native artifacts from workflow {workflow_id}...", flush=True)
    with _gha_group(f"Download native artifacts from workflow {workflow_id}"):
        artifacts_dir.mkdir(parents=True, exist_ok=True)
        install_from_workflow_artifacts(
            workflow_id,
            artifacts_dir,
            sorted(components),
            vendor_dir,
        )
    print(f"Installed native dependencies into {vendor_dir}", flush=True)


def install_from_workflow_artifacts(
    workflow_id: str,
    artifacts_dir: Path,
    components: Sequence[str],
    vendor_dir: Path,
) -> None:
    artifacts = select_target_artifacts(workflow_id, components)
    download_artifacts(workflow_id, artifacts_dir, artifacts)
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


def download_artifacts(
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
        subprocess.check_call(
            [
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
        )


def install_codex_package_archives(
    artifacts_dir: Path,
    vendor_dir: Path,
    targets: Sequence[str],
) -> None:
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
                install_single_codex_package_archive,
                artifacts_dir,
                vendor_dir,
                target,
            ): target
            for target in targets
        }
        for future in as_completed(futures):
            installed_path = future.result()
            print(f"  installed {installed_path}", flush=True)


def install_single_codex_package_archive(
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


def install_binary_components(
    artifacts_dir: Path,
    vendor_dir: Path,
    selected_components: Sequence[BinaryComponent],
) -> None:
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
                    install_single_binary,
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


def install_single_binary(
    artifacts_dir: Path,
    vendor_dir: Path,
    target: str,
    component: BinaryComponent,
) -> Path:
    artifact_subdir = artifact_dir_for_target(artifacts_dir, target)
    archive_path = binary_archive_path(
        artifact_subdir, component.artifact_prefix, target
    )

    dest_dir = vendor_dir / target / component.dest_dir
    dest_dir.mkdir(parents=True, exist_ok=True)

    binary_name = (
        f"{component.binary_basename}.exe"
        if "windows" in target
        else component.binary_basename
    )
    dest = dest_dir / binary_name
    dest.unlink(missing_ok=True)
    extract_zstd_archive(archive_path, dest)
    if "windows" not in target:
        dest.chmod(0o755)
    return dest


def binary_archive_path(artifact_dir: Path, artifact_prefix: str, target: str) -> Path:
    archive_names = [archive_name_for_target(artifact_prefix, target)]
    if artifact_dir.name == f"{target}-unsigned":
        archive_names.append(
            archive_name_for_target(artifact_prefix, f"{target}-unsigned")
        )

    for archive_name in archive_names:
        archive_path = artifact_dir / archive_name
        if archive_path.exists():
            return archive_path

    raise FileNotFoundError(
        f"Expected artifact not found: {artifact_dir / archive_names[0]}"
    )


def archive_name_for_target(artifact_prefix: str, target: str) -> str:
    if "windows" in target:
        return f"{artifact_prefix}-{target}.exe.zst"
    return f"{artifact_prefix}-{target}.zst"


def artifact_dir_for_target(artifacts_dir: Path, target: str) -> Path:
    for artifact_name in [target, f"{target}-unsigned"]:
        artifact_dir = artifacts_dir / artifact_name
        if artifact_dir.is_dir():
            return artifact_dir

    return artifacts_dir / target


def extract_zstd_archive(archive_path: Path, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)

    output_path = archive_path.parent / dest.name
    subprocess.check_call(
        ["zstd", "-f", "-d", str(archive_path), "-o", str(output_path)]
    )
    shutil.move(str(output_path), dest)


def format_bytes(size_in_bytes: int) -> str:
    value = float(size_in_bytes)
    for unit in ["B", "KiB", "MiB"]:
        if value < 1024:
            return f"{value:.1f} {unit}"
        value /= 1024
    return f"{value:.1f} GiB"


def run_command(cmd: list[str]) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=REPO_ROOT, check=True)


def tarball_name_for_package(package: str, version: str) -> str:
    if package in CODEX_PLATFORM_PACKAGES:
        platform = package.removeprefix("codex-")
        return f"codex-npm-{platform}-{version}.tgz"
    return f"{package}-npm-{version}.tgz"


def main() -> int:
    args = parse_args()

    output_dir = args.output_dir or (REPO_ROOT / "dist" / "npm")
    output_dir.mkdir(parents=True, exist_ok=True)

    runner_temp = Path(os.environ.get("RUNNER_TEMP", tempfile.gettempdir()))

    packages = expand_packages(list(args.packages))
    native_component_sets = collect_native_component_sets(packages)
    print("Expanded packages: " + ", ".join(packages), flush=True)
    if native_component_sets:
        component_sets = [
            "(" + ", ".join(components) + ")" for components in native_component_sets
        ]
        print(
            "Native component sets: " + ", ".join(component_sets),
            flush=True,
        )

    vendor_temp_roots: list[Path] = []
    vendor_src_by_components: dict[tuple[str, ...], Path] = {}
    artifacts_temp_root: Path | None = None
    resolved_head_sha: str | None = None

    final_messages = []

    try:
        if native_component_sets:
            workflow_url, resolved_head_sha = resolve_workflow_url(
                args.release_version, args.workflow_url
            )
            print(f"Using native artifacts from {workflow_url}", flush=True)
            artifacts_temp_root = Path(
                tempfile.mkdtemp(prefix="npm-native-artifacts-", dir=runner_temp)
            )
            print(f"Caching downloaded artifacts in {artifacts_temp_root}", flush=True)
            for components in native_component_sets:
                vendor_temp_root = Path(
                    tempfile.mkdtemp(prefix="npm-native-", dir=runner_temp)
                )
                vendor_temp_roots.append(vendor_temp_root)
                print(
                    "Installing native components "
                    + ", ".join(components)
                    + f" into {vendor_temp_root}",
                    flush=True,
                )
                install_native_components(
                    workflow_url,
                    set(components),
                    vendor_temp_root,
                    artifacts_temp_root,
                )
                vendor_src_by_components[components] = vendor_temp_root / "vendor"

        if resolved_head_sha:
            print(f"should `git checkout {resolved_head_sha}`", flush=True)

        for package in packages:
            staging_dir = Path(
                tempfile.mkdtemp(prefix=f"npm-stage-{package}-", dir=runner_temp)
            )
            pack_output = output_dir / tarball_name_for_package(
                package, args.release_version
            )
            print(f"Staging {package} in {staging_dir}", flush=True)

            cmd = [
                str(BUILD_SCRIPT),
                "--package",
                package,
                "--release-version",
                args.release_version,
                "--staging-dir",
                str(staging_dir),
                "--pack-output",
                str(pack_output),
            ]

            vendor_src = vendor_src_by_components.get(
                native_components_for_package(package)
            )
            if vendor_src is not None:
                cmd.extend(["--vendor-src", str(vendor_src)])

            try:
                run_command(cmd)
            finally:
                if not args.keep_staging_dirs:
                    shutil.rmtree(staging_dir, ignore_errors=True)

            final_messages.append(f"Staged {package} at {pack_output}")
    finally:
        if not args.keep_staging_dirs:
            for vendor_temp_root in vendor_temp_roots:
                shutil.rmtree(vendor_temp_root, ignore_errors=True)
        if artifacts_temp_root is not None:
            shutil.rmtree(artifacts_temp_root, ignore_errors=True)

    for msg in final_messages:
        print(msg, flush=True)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
