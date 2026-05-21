#!/usr/bin/env python3
"""Install Codex package archives and native helper binaries."""

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
REPO_ROOT = CODEX_CLI_ROOT.parent
DEFAULT_WORKFLOW_URL = "https://github.com/openai/codex/actions/runs/26131514935"  # rust-v0.132.0
VENDOR_DIR_NAME = "vendor"
RG_MANIFEST = REPO_ROOT / "scripts" / "codex_package" / "rg"
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
    targets: tuple[str, ...] | None = None  # limit installation to specific targets


WINDOWS_TARGETS = tuple(target for target in BINARY_TARGETS if "windows" in target)
LINUX_TARGETS = tuple(target for target in BINARY_TARGETS if "linux" in target)

BINARY_COMPONENTS = {
    "bwrap": BinaryComponent(
        artifact_prefix="bwrap",
        dest_dir="codex-resources",
        binary_basename="bwrap",
        targets=LINUX_TARGETS,
    ),
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
        choices=tuple([CODEX_PACKAGE_COMPONENT, *BINARY_COMPONENTS, "rg"]),
        help=(
            "Limit installation to the specified components."
            " May be repeated. Defaults to codex-package and codex-responses-api-proxy."
        ),
    )
    parser.add_argument(
        "--allow-legacy-codex-package",
        action="store_true",
        help=(
            "Allow codex-package to be synthesized from legacy per-binary artifacts "
            "when package archives are missing. Intended for CI compatibility only; "
            "release staging should not use this. Automatically enabled for the "
            "built-in default workflow."
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
    use_default_workflow = not workflow_override
    workflow_url = workflow_override or DEFAULT_WORKFLOW_URL

    workflow_id = workflow_url.rstrip("/").split("/")[-1]
    print(f"Downloading native artifacts from workflow {workflow_id}...")

    with _gha_group(f"Download native artifacts from workflow {workflow_id}"):
        with tempfile.TemporaryDirectory(prefix="codex-native-artifacts-") as artifacts_dir_str:
            artifacts_dir = Path(artifacts_dir_str)
            _download_artifacts(workflow_id, artifacts_dir)
            if CODEX_PACKAGE_COMPONENT in components:
                try:
                    install_codex_package_archives(artifacts_dir, vendor_dir, BINARY_TARGETS)
                except FileNotFoundError:
                    if not (args.allow_legacy_codex_package or use_default_workflow):
                        raise
                    install_legacy_codex_package_layouts(
                        artifacts_dir,
                        vendor_dir,
                        BINARY_TARGETS,
                        manifest_path=RG_MANIFEST,
                    )
            install_binary_components(
                artifacts_dir,
                vendor_dir,
                [BINARY_COMPONENTS[name] for name in components if name in BINARY_COMPONENTS],
            )

    if "rg" in components:
        with _gha_group("Fetch ripgrep binaries"):
            print("Fetching ripgrep binaries...")
            fetch_rg(vendor_dir, DEFAULT_RG_TARGETS, manifest_path=RG_MANIFEST)

    print(f"Installed native dependencies into {vendor_dir}")
    return 0


def install_codex_package_archives(
    artifacts_dir: Path,
    vendor_dir: Path,
    targets: Sequence[str],
) -> None:
    targets = list(targets)
    if not targets:
        return

    print("Installing Codex package archives for targets: " + ", ".join(targets))
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
            print(f"  installed {installed_path}")


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


def install_legacy_codex_package_layouts(
    artifacts_dir: Path,
    vendor_dir: Path,
    targets: Sequence[str],
    *,
    manifest_path: Path,
) -> None:
    targets = list(targets)
    print(
        "Synthesizing Codex package layouts from legacy artifacts for targets: "
        + ", ".join(targets)
    )
    with tempfile.TemporaryDirectory(prefix="codex-legacy-package-") as legacy_vendor_dir_str:
        legacy_vendor_dir = Path(legacy_vendor_dir_str)
        install_binary_components(
            artifacts_dir,
            legacy_vendor_dir,
            [
                BINARY_COMPONENTS["codex"],
                BINARY_COMPONENTS["bwrap"],
                BINARY_COMPONENTS["codex-windows-sandbox-setup"],
                BINARY_COMPONENTS["codex-command-runner"],
            ],
        )
        fetch_rg(legacy_vendor_dir, targets, manifest_path=manifest_path)

        for target in targets:
            dest_dir = vendor_dir / target
            if dest_dir.exists():
                shutil.rmtree(dest_dir)
            _build_legacy_codex_package_layout(legacy_vendor_dir / target, dest_dir, target)
            print(f"  synthesized {dest_dir}")


def _build_legacy_codex_package_layout(
    legacy_target_dir: Path,
    package_dir: Path,
    target: str,
) -> None:
    is_windows = "windows" in target
    exe_suffix = ".exe" if is_windows else ""
    package_dir.mkdir(parents=True)

    bin_dir = package_dir / "bin"
    resources_dir = package_dir / "codex-resources"
    path_dir = package_dir / "codex-path"
    bin_dir.mkdir()
    resources_dir.mkdir()
    path_dir.mkdir()

    shutil.copy2(
        legacy_target_dir / "codex" / f"codex{exe_suffix}",
        bin_dir / f"codex{exe_suffix}",
    )
    shutil.copy2(
        legacy_target_dir / "path" / f"rg{exe_suffix}",
        path_dir / f"rg{exe_suffix}",
    )

    if is_windows:
        for helper in [
            "codex-command-runner.exe",
            "codex-windows-sandbox-setup.exe",
        ]:
            shutil.copy2(legacy_target_dir / "codex" / helper, resources_dir / helper)
    elif "linux" in target:
        shutil.copy2(legacy_target_dir / "codex-resources" / "bwrap", resources_dir / "bwrap")

    write_json(
        package_dir / "codex-package.json",
        {
            "layoutVersion": 1,
            "version": "unknown",
            "target": target,
            "variant": "codex",
            "entrypoint": f"bin/codex{exe_suffix}",
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        },
    )


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
    artifact_subdir = artifact_dir_for_target(artifacts_dir, target)
    archive_path = legacy_binary_archive_path(artifact_subdir, component.artifact_prefix, target)

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


def legacy_binary_archive_path(artifact_dir: Path, artifact_prefix: str, target: str) -> Path:
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


def write_json(path: Path, value: object) -> None:
    with open(path, "w", encoding="utf-8") as out:
        json.dump(value, out, indent=2)
        out.write("\n")


if __name__ == "__main__":
    import sys

    sys.exit(main())
