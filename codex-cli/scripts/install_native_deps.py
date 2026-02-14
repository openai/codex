#!/usr/bin/env python3
"""Install Codex native binaries (Rust CLI plus ripgrep helpers)."""

import argparse
from contextlib import contextmanager
import json
import os
import shutil
import subprocess
import tarfile
import tempfile
import zipfile
from dataclasses import dataclass
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
import sys
from typing import Iterable, Sequence
from urllib.parse import urlparse
from urllib.request import urlopen

SCRIPT_DIR = Path(__file__).resolve().parent
CODEX_CLI_ROOT = SCRIPT_DIR.parent
DEFAULT_WORKFLOW_URL = "https://github.com/openai/codex/actions/runs/17952349351"  # rust-v0.40.0
VENDOR_DIR_NAME = "vendor"
RG_MANIFEST = CODEX_CLI_ROOT / "bin" / "rg"
NODE_VERSION_FILE = CODEX_CLI_ROOT.parent / "codex-rs" / "node-version.txt"
BINARY_TARGETS = (
    "x86_64-unknown-linux-musl",
    "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
    "aarch64-pc-windows-msvc",
)


@dataclass(frozen=True)
class BinaryComponent:
    artifact_prefix: str  # matches the artifact filename prefix (e.g. codex-<target>.zst)
    dest_dir: str  # directory under vendor/<target>/ where the binary is installed
    binary_basename: str  # executable name inside dest_dir (before optional .exe)
    targets: tuple[str, ...] | None = None  # limit installation to specific targets


WINDOWS_TARGETS = tuple(target for target in BINARY_TARGETS if "windows" in target)

BINARY_COMPONENTS = {
    "codex": BinaryComponent(
        artifact_prefix="codex",
        dest_dir="codex",
        binary_basename="codex",
    ),
    "codex-responses-api-proxy": BinaryComponent(
        artifact_prefix="codex-responses-api-proxy",
        dest_dir="codex-responses-api-proxy",
        binary_basename="codex-responses-api-proxy",
    ),
    "codex-windows-sandbox-setup": BinaryComponent(
        artifact_prefix="codex-windows-sandbox-setup",
        dest_dir="codex",
        binary_basename="codex-windows-sandbox-setup",
        targets=WINDOWS_TARGETS,
    ),
    "codex-command-runner": BinaryComponent(
        artifact_prefix="codex-command-runner",
        dest_dir="codex",
        binary_basename="codex-command-runner",
        targets=WINDOWS_TARGETS,
    ),
}

RG_TARGET_PLATFORM_PAIRS: list[tuple[str, str]] = [
    ("x86_64-unknown-linux-musl", "linux-x86_64"),
    ("aarch64-unknown-linux-musl", "linux-aarch64"),
    ("x86_64-apple-darwin", "macos-x86_64"),
    ("aarch64-apple-darwin", "macos-aarch64"),
    ("x86_64-pc-windows-msvc", "windows-x86_64"),
    ("aarch64-pc-windows-msvc", "windows-aarch64"),
]
RG_TARGET_TO_PLATFORM = {target: platform for target, platform in RG_TARGET_PLATFORM_PAIRS}
DEFAULT_RG_TARGETS = [target for target, _ in RG_TARGET_PLATFORM_PAIRS]

# Node distribution asset mapping: target -> (archive_format, filename, member_path)
NODE_ASSETS: dict[str, tuple[str, str, str]] = {
    "x86_64-apple-darwin": (
        "tar.gz",
        "node-v{ver}-darwin-x64.tar.gz",
        "node-v{ver}-darwin-x64/bin/node",
    ),
    "aarch64-apple-darwin": (
        "tar.gz",
        "node-v{ver}-darwin-arm64.tar.gz",
        "node-v{ver}-darwin-arm64/bin/node",
    ),
    "x86_64-unknown-linux-musl": (
        "tar.xz",
        "node-v{ver}-linux-x64.tar.xz",
        "node-v{ver}-linux-x64/bin/node",
    ),
    "aarch64-unknown-linux-musl": (
        "tar.xz",
        "node-v{ver}-linux-arm64.tar.xz",
        "node-v{ver}-linux-arm64/bin/node",
    ),
    "x86_64-pc-windows-msvc": (
        "zip",
        "node-v{ver}-win-x64.zip",
        "node-v{ver}-win-x64/node.exe",
    ),
    "aarch64-pc-windows-msvc": (
        "zip",
        "node-v{ver}-win-arm64.zip",
        "node-v{ver}-win-arm64/node.exe",
    ),
}
DEFAULT_NODE_TARGETS = tuple(NODE_ASSETS.keys())

# urllib.request.urlopen() defaults to no timeout (can hang indefinitely), which is painful in CI.
DOWNLOAD_TIMEOUT_SECS = 60


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
        choices=tuple(list(BINARY_COMPONENTS) + ["rg", "node"]),
        help=(
            "Limit installation to the specified components."
            " May be repeated. Defaults to codex, codex-windows-sandbox-setup,"
            " codex-command-runner, node, and rg."
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

    components = args.components or [
        "codex",
        "codex-windows-sandbox-setup",
        "codex-command-runner",
        "rg",
        "node",
    ]

    workflow_url = (args.workflow_url or DEFAULT_WORKFLOW_URL).strip()
    if not workflow_url:
        workflow_url = DEFAULT_WORKFLOW_URL

    workflow_id = workflow_url.rstrip("/").split("/")[-1]
    print(f"Downloading native artifacts from workflow {workflow_id}...")

    with _gha_group(f"Download native artifacts from workflow {workflow_id}"):
        with tempfile.TemporaryDirectory(prefix="codex-native-artifacts-") as artifacts_dir_str:
            artifacts_dir = Path(artifacts_dir_str)
            _download_artifacts(workflow_id, artifacts_dir)
            install_binary_components(
                artifacts_dir,
                vendor_dir,
                [BINARY_COMPONENTS[name] for name in components if name in BINARY_COMPONENTS],
            )

    if "rg" in components:
        with _gha_group("Fetch ripgrep binaries"):
            print("Fetching ripgrep binaries...")
            fetch_rg(vendor_dir, DEFAULT_RG_TARGETS, manifest_path=RG_MANIFEST)

    if "node" in components:
        with _gha_group("Fetch Node runtime"):
            print("Fetching Node runtime...")
            fetch_node(vendor_dir, load_node_version(), DEFAULT_NODE_TARGETS)

    print(f"Installed native dependencies into {vendor_dir}")
    return 0


def fetch_rg(
    vendor_dir: Path,
    targets: Sequence[str] | None = None,
    *,
    manifest_path: Path,
) -> list[Path]:
    """Download ripgrep binaries described by the DotSlash manifest."""

    if targets is None:
        targets = DEFAULT_RG_TARGETS

    if not manifest_path.exists():
        raise FileNotFoundError(f"DotSlash manifest not found: {manifest_path}")

    manifest = _load_manifest(manifest_path)
    platforms = manifest.get("platforms", {})

    vendor_dir.mkdir(parents=True, exist_ok=True)

    targets = list(targets)
    if not targets:
        return []

    task_configs: list[tuple[str, str, dict]] = []
    for target in targets:
        platform_key = RG_TARGET_TO_PLATFORM.get(target)
        if platform_key is None:
            raise ValueError(f"Unsupported ripgrep target '{target}'.")

        platform_info = platforms.get(platform_key)
        if platform_info is None:
            raise RuntimeError(f"Platform '{platform_key}' not found in manifest {manifest_path}.")

        task_configs.append((target, platform_key, platform_info))

    results: dict[str, Path] = {}
    max_workers = min(len(task_configs), max(1, (os.cpu_count() or 1)))

    print("Installing ripgrep binaries for targets: " + ", ".join(targets))

    with ThreadPoolExecutor(max_workers=max_workers) as executor:
        future_map = {
            executor.submit(
                _fetch_single_rg,
                vendor_dir,
                target,
                platform_key,
                platform_info,
                manifest_path,
            ): target
            for target, platform_key, platform_info in task_configs
        }

        for future in as_completed(future_map):
            target = future_map[future]
            try:
                results[target] = future.result()
            except Exception as exc:
                _gha_error(
                    title="ripgrep install failed",
                    message=f"target={target} error={exc!r}",
                )
                raise RuntimeError(f"Failed to install ripgrep for target {target}.") from exc
            print(f"  installed ripgrep for {target}")

    return [results[target] for target in targets]


def fetch_node(vendor_dir: Path, version: str, targets: Sequence[str]) -> None:
    version = version.strip().lstrip("v")
    vendor_dir.mkdir(parents=True, exist_ok=True)

    for target in targets:
        asset = NODE_ASSETS.get(target)
        if asset is None:
            raise RuntimeError(f"unsupported Node target {target}")
        archive_format, filename_tmpl, member_tmpl = asset
        filename = filename_tmpl.format(ver=version)
        member = member_tmpl.format(ver=version)
        url = f"https://nodejs.org/dist/v{version}/{filename}"
        dest_dir = vendor_dir / target / "node"
        dest_dir.mkdir(parents=True, exist_ok=True)
        binary_name = "node.exe" if "windows" in target else "node"
        dest = dest_dir / binary_name

        with tempfile.TemporaryDirectory() as tmp_dir_str:
            tmp_dir = Path(tmp_dir_str)
            download_path = tmp_dir / filename
            print(f"  downloading node {version} for {target} from {url}", flush=True)
            _download_file(url, download_path)
            dest.unlink(missing_ok=True)
            extract_archive(download_path, archive_format, member, dest)
        if "windows" not in target:
            dest.chmod(0o755)


def load_node_version() -> str:
    try:
        return NODE_VERSION_FILE.read_text(encoding="utf-8").strip()
    except OSError as exc:
        raise RuntimeError(f"failed to read node version from {NODE_VERSION_FILE}: {exc}") from exc


def _download_artifacts(workflow_id: str, dest_dir: Path) -> None:
    cmd = [
        "gh",
        "run",
        "download",
        "--dir",
        str(dest_dir),
        "--repo",
        "openai/codex",
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
        component_targets = list(component.targets or BINARY_TARGETS)

        print(
            f"Installing {component.binary_basename} binaries for targets: "
            + ", ".join(component_targets)
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
                print(f"  installed {installed_path}")


def _install_single_binary(
    artifacts_dir: Path,
    vendor_dir: Path,
    target: str,
    component: BinaryComponent,
) -> Path:
    artifact_subdir = artifacts_dir / target
    archive_name = _archive_name_for_target(component.artifact_prefix, target)
    archive_path = artifact_subdir / archive_name
    if not archive_path.exists():
        raise FileNotFoundError(f"Expected artifact not found: {archive_path}")

    dest_dir = vendor_dir / target / component.dest_dir
    dest_dir.mkdir(parents=True, exist_ok=True)

    binary_name = (
        f"{component.binary_basename}.exe" if "windows" in target else component.binary_basename
    )
    dest = dest_dir / binary_name
    dest.unlink(missing_ok=True)
    extract_archive(archive_path, "zst", None, dest)
    if "windows" not in target:
        dest.chmod(0o755)
    return dest


def _archive_name_for_target(artifact_prefix: str, target: str) -> str:
    if "windows" in target:
        return f"{artifact_prefix}-{target}.exe.zst"
    return f"{artifact_prefix}-{target}.zst"


def _fetch_single_rg(
    vendor_dir: Path,
    target: str,
    platform_key: str,
    platform_info: dict,
    manifest_path: Path,
) -> Path:
    providers = platform_info.get("providers", [])
    if not providers:
        raise RuntimeError(f"No providers listed for platform '{platform_key}' in {manifest_path}.")

    url = providers[0]["url"]
    archive_format = platform_info.get("format", "zst")
    archive_member = platform_info.get("path")
    digest = platform_info.get("digest")
    expected_size = platform_info.get("size")

    dest_dir = vendor_dir / target / "path"
    dest_dir.mkdir(parents=True, exist_ok=True)

    is_windows = platform_key.startswith("win")
    binary_name = "rg.exe" if is_windows else "rg"
    dest = dest_dir / binary_name

    with tempfile.TemporaryDirectory() as tmp_dir_str:
        tmp_dir = Path(tmp_dir_str)
        archive_filename = os.path.basename(urlparse(url).path)
        download_path = tmp_dir / archive_filename
        print(
            f"  downloading ripgrep for {target} ({platform_key}) from {url}",
            flush=True,
        )
        try:
            _download_file(url, download_path)
        except Exception as exc:
            _gha_error(
                title="ripgrep download failed",
                message=f"target={target} platform={platform_key} url={url} error={exc!r}",
            )
            raise RuntimeError(
                "Failed to download ripgrep "
                f"(target={target}, platform={platform_key}, format={archive_format}, "
                f"expected_size={expected_size!r}, digest={digest!r}, url={url}, dest={download_path})."
            ) from exc

        dest.unlink(missing_ok=True)
        try:
            extract_archive(download_path, archive_format, archive_member, dest)
        except Exception as exc:
            raise RuntimeError(
                "Failed to extract ripgrep "
                f"(target={target}, platform={platform_key}, format={archive_format}, "
                f"member={archive_member!r}, url={url}, archive={download_path})."
            ) from exc

    if not is_windows:
        dest.chmod(0o755)

    return dest


def _download_file(url: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.unlink(missing_ok=True)

    with urlopen(url, timeout=DOWNLOAD_TIMEOUT_SECS) as response, open(dest, "wb") as out:
        shutil.copyfileobj(response, out)


def extract_archive(
    archive_path: Path,
    archive_format: str,
    archive_member: str | None,
    dest: Path,
) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)

    if archive_format == "zst":
        output_path = archive_path.parent / dest.name
        subprocess.check_call(
            ["zstd", "-f", "-d", str(archive_path), "-o", str(output_path)]
        )
        shutil.move(str(output_path), dest)
        return

    if archive_format == "tar.gz":
        if not archive_member:
            raise RuntimeError("Missing 'path' for tar.gz archive in DotSlash manifest.")
        with tarfile.open(archive_path, "r:gz") as tar:
            try:
                member = tar.getmember(archive_member)
            except KeyError as exc:
                raise RuntimeError(
                    f"Entry '{archive_member}' not found in archive {archive_path}."
                ) from exc
            tar.extract(member, path=archive_path.parent, filter="data")
        extracted = archive_path.parent / archive_member
        shutil.move(str(extracted), dest)
        return

    if archive_format == "tar.xz":
        if not archive_member:
            raise RuntimeError("Missing 'path' for tar.xz archive in archive.")
        with tarfile.open(archive_path, "r:xz") as tar:
            try:
                member = tar.getmember(archive_member)
            except KeyError as exc:
                raise RuntimeError(
                    f"Entry '{archive_member}' not found in archive {archive_path}."
                ) from exc
            tar.extract(member, path=archive_path.parent, filter="data")
        extracted = archive_path.parent / archive_member
        shutil.move(str(extracted), dest)
        return

    if archive_format == "zip":
        if not archive_member:
            raise RuntimeError("Missing 'path' for zip archive in DotSlash manifest.")
        with zipfile.ZipFile(archive_path) as archive:
            try:
                with archive.open(archive_member) as src, open(dest, "wb") as out:
                    shutil.copyfileobj(src, out)
            except KeyError as exc:
                raise RuntimeError(
                    f"Entry '{archive_member}' not found in archive {archive_path}."
                ) from exc
        return

    raise RuntimeError(f"Unsupported archive format '{archive_format}'.")


def _load_manifest(manifest_path: Path) -> dict:
    cmd = ["dotslash", "--", "parse", str(manifest_path)]
    stdout = subprocess.check_output(cmd, text=True)
    try:
        manifest = json.loads(stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"Invalid DotSlash manifest output from {manifest_path}.") from exc

    if not isinstance(manifest, dict):
        raise RuntimeError(
            f"Unexpected DotSlash manifest structure for {manifest_path}: {type(manifest)!r}"
        )

    return manifest


if __name__ == "__main__":
    import sys

    sys.exit(main())
