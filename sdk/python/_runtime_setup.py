from __future__ import annotations

import importlib
import importlib.util
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import urllib.error
import urllib.request
import zipfile
from pathlib import Path

PACKAGE_NAME = "codex-cli-bin"
RUNTIME_VERSION_ENV_VAR = "CODEX_PYTHON_RUNTIME_VERSION"
REPO_SLUG = "openai/codex"


class RuntimeSetupError(RuntimeError):
    pass


def configured_runtime_version() -> str | None:
    value = os.environ.get(RUNTIME_VERSION_ENV_VAR)
    if value is None:
        return None
    normalized = value.strip()
    if not normalized:
        raise RuntimeSetupError(
            f"{RUNTIME_VERSION_ENV_VAR} is set but empty. "
            "Set it to a release version like 0.115.0-alpha.11."
        )
    return normalized


def required_runtime_version() -> str:
    version = configured_runtime_version()
    if version is not None:
        return version
    raise RuntimeSetupError(
        f"Set {RUNTIME_VERSION_ENV_VAR}=<version> so repo-local examples and real "
        "integration coverage can install the pinned codex-cli-bin runtime package."
    )


def ensure_runtime_package_installed(
    python_executable: str | Path,
    sdk_python_dir: Path,
    runtime_version: str | None = None,
    install_target: Path | None = None,
) -> str:
    requested_version = runtime_version or configured_runtime_version()
    installed_version = None
    if install_target is None:
        installed_version = _installed_runtime_version(python_executable)
    normalized_requested = (
        _normalized_package_version(requested_version) if requested_version is not None else None
    )
    if requested_version is None:
        if install_target is not None:
            raise RuntimeSetupError(
                f"{RUNTIME_VERSION_ENV_VAR} is required when installing {PACKAGE_NAME} "
                "into an isolated target directory."
            )
        if installed_version is None:
            raise RuntimeSetupError(
                f"Unable to locate {PACKAGE_NAME} in {python_executable}.\n"
                f"Install {PACKAGE_NAME} first, or set {RUNTIME_VERSION_ENV_VAR}=<version> "
                "to download a matching release codex artifact and install a local runtime wheel."
            )
        return installed_version

    if installed_version is not None and _normalized_package_version(installed_version) == normalized_requested:
        return requested_version

    with tempfile.TemporaryDirectory(prefix="codex-python-runtime-") as temp_root_str:
        temp_root = Path(temp_root_str)
        archive_path = _download_release_archive(requested_version, temp_root)
        runtime_binary = _extract_runtime_binary(archive_path, temp_root)
        staged_runtime_dir = _stage_runtime_package(
            sdk_python_dir,
            requested_version,
            runtime_binary,
            temp_root / "runtime-stage",
        )
        _install_runtime_package(python_executable, staged_runtime_dir, install_target)

    if install_target is not None:
        return requested_version

    if Path(python_executable).resolve() == Path(sys.executable).resolve():
        importlib.invalidate_caches()

    installed_version = _installed_runtime_version(python_executable)
    if installed_version is None or _normalized_package_version(installed_version) != normalized_requested:
        raise RuntimeSetupError(
            f"Expected {PACKAGE_NAME} {requested_version} in {python_executable}, "
            f"but found {installed_version!r} after installation."
        )
    return requested_version


def platform_asset_name() -> str:
    system = platform.system().lower()
    machine = platform.machine().lower()

    if system == "darwin":
        if machine in {"arm64", "aarch64"}:
            return "codex-aarch64-apple-darwin.tar.gz"
        if machine in {"x86_64", "amd64"}:
            return "codex-x86_64-apple-darwin.tar.gz"
    elif system == "linux":
        if machine in {"aarch64", "arm64"}:
            return "codex-aarch64-unknown-linux-musl.tar.gz"
        if machine in {"x86_64", "amd64"}:
            return "codex-x86_64-unknown-linux-musl.tar.gz"
    elif system == "windows":
        if machine in {"aarch64", "arm64"}:
            return "codex-aarch64-pc-windows-msvc.exe.zip"
        if machine in {"x86_64", "amd64"}:
            return "codex-x86_64-pc-windows-msvc.exe.zip"

    raise RuntimeSetupError(
        f"Unsupported runtime artifact platform: system={platform.system()!r}, "
        f"machine={platform.machine()!r}"
    )


def runtime_binary_name() -> str:
    return "codex.exe" if platform.system().lower() == "windows" else "codex"


def _installed_runtime_version(python_executable: str | Path) -> str | None:
    snippet = (
        "import importlib.metadata, json, sys\n"
        "try:\n"
        "    from codex_cli_bin import bundled_codex_path\n"
        "    bundled_codex_path()\n"
        "    print(json.dumps({'version': importlib.metadata.version('codex-cli-bin')}))\n"
        "except Exception:\n"
        "    sys.exit(1)\n"
    )
    result = subprocess.run(
        [str(python_executable), "-c", snippet],
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        return None
    return json.loads(result.stdout)["version"]


def _release_metadata(version: str) -> dict[str, object]:
    url = f"https://api.github.com/repos/{REPO_SLUG}/releases/tags/rust-v{version}"
    request = urllib.request.Request(
        url,
        headers=_github_api_headers("application/vnd.github+json"),
    )
    try:
        with urllib.request.urlopen(request) as response:
            return json.load(response)
    except urllib.error.HTTPError as exc:
        raise RuntimeSetupError(
            f"Failed to resolve release metadata for rust-v{version} from {REPO_SLUG}: "
            f"{exc.code} {exc.reason}"
        ) from exc


def _download_release_archive(version: str, temp_root: Path) -> Path:
    asset_name = platform_asset_name()
    metadata = _release_metadata(version)
    assets = metadata.get("assets")
    if not isinstance(assets, list):
        raise RuntimeSetupError(f"Release rust-v{version} returned malformed assets metadata.")
    asset = next(
        (
            item
            for item in assets
            if isinstance(item, dict) and item.get("name") == asset_name
        ),
        None,
    )
    if asset is None:
        raise RuntimeSetupError(
            f"Release rust-v{version} does not contain asset {asset_name} for this platform."
        )

    archive_path = temp_root / asset_name
    api_url = asset.get("url")
    browser_download_url = asset.get("browser_download_url")
    if not isinstance(api_url, str):
        api_url = None
    if not isinstance(browser_download_url, str):
        browser_download_url = None

    if api_url is not None:
        token = _github_token()
        if token is not None:
            request = urllib.request.Request(
                api_url,
                headers=_github_api_headers("application/octet-stream"),
            )
            try:
                with urllib.request.urlopen(request) as response, archive_path.open("wb") as fh:
                    shutil.copyfileobj(response, fh)
                return archive_path
            except urllib.error.HTTPError:
                pass

    if browser_download_url is not None:
        request = urllib.request.Request(
            browser_download_url,
            headers={"User-Agent": "codex-python-runtime-setup"},
        )
        try:
            with urllib.request.urlopen(request) as response, archive_path.open("wb") as fh:
                shutil.copyfileobj(response, fh)
            return archive_path
        except urllib.error.HTTPError:
            pass

    if shutil.which("gh") is None:
        raise RuntimeSetupError(
            f"Unable to download {asset_name} for rust-v{version}. "
            "Provide GH_TOKEN/GITHUB_TOKEN or install/authenticate GitHub CLI."
        )

    try:
        subprocess.run(
            [
                "gh",
                "release",
                "download",
                f"rust-v{version}",
                "--repo",
                REPO_SLUG,
                "--pattern",
                asset_name,
                "--dir",
                str(temp_root),
            ],
            check=True,
            text=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as exc:
        raise RuntimeSetupError(
            f"gh release download failed for rust-v{version} asset {asset_name}.\n"
            f"STDOUT:\n{exc.stdout}\nSTDERR:\n{exc.stderr}"
        ) from exc
    return archive_path


def _extract_runtime_binary(archive_path: Path, temp_root: Path) -> Path:
    extract_dir = temp_root / "extracted"
    extract_dir.mkdir(parents=True, exist_ok=True)
    if archive_path.name.endswith(".tar.gz"):
        with tarfile.open(archive_path, "r:gz") as tar:
            try:
                tar.extractall(extract_dir, filter="data")
            except TypeError:
                tar.extractall(extract_dir)
    elif archive_path.suffix == ".zip":
        with zipfile.ZipFile(archive_path) as zip_file:
            zip_file.extractall(extract_dir)
    else:
        raise RuntimeSetupError(f"Unsupported release archive format: {archive_path.name}")

    binary_name = runtime_binary_name()
    archive_stem = archive_path.name.removesuffix(".tar.gz").removesuffix(".zip")
    candidates = [
        path
        for path in extract_dir.rglob("*")
        if path.is_file()
        and (
            path.name == binary_name
            or path.name == archive_stem
            or path.name.startswith("codex-")
        )
    ]
    if not candidates:
        raise RuntimeSetupError(
            f"Failed to find {binary_name} in extracted runtime archive {archive_path.name}."
        )
    return candidates[0]


def _stage_runtime_package(
    sdk_python_dir: Path,
    runtime_version: str,
    runtime_binary: Path,
    staging_dir: Path,
) -> Path:
    script_module = _load_update_script_module(sdk_python_dir)
    return script_module.stage_python_runtime_package(  # type: ignore[no-any-return]
        staging_dir,
        runtime_version,
        runtime_binary.resolve(),
    )


def _install_runtime_package(
    python_executable: str | Path,
    staged_runtime_dir: Path,
    install_target: Path | None,
) -> None:
    args = [
        str(python_executable),
        "-m",
        "pip",
        "install",
        "--force-reinstall",
        "--no-deps",
    ]
    if install_target is not None:
        install_target.mkdir(parents=True, exist_ok=True)
        args.extend(["--target", str(install_target)])
    args.append(str(staged_runtime_dir))
    try:
        subprocess.run(
            args,
            check=True,
            text=True,
            capture_output=True,
        )
    except subprocess.CalledProcessError as exc:
        raise RuntimeSetupError(
            f"Failed to install {PACKAGE_NAME} into {python_executable} from {staged_runtime_dir}.\n"
            f"STDOUT:\n{exc.stdout}\nSTDERR:\n{exc.stderr}"
        ) from exc


def _load_update_script_module(sdk_python_dir: Path):
    script_path = sdk_python_dir / "scripts" / "update_sdk_artifacts.py"
    spec = importlib.util.spec_from_file_location("update_sdk_artifacts", script_path)
    if spec is None or spec.loader is None:
        raise RuntimeSetupError(f"Failed to load {script_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _github_api_headers(accept: str) -> dict[str, str]:
    headers = {
        "Accept": accept,
        "User-Agent": "codex-python-runtime-setup",
    }
    token = _github_token()
    if token is not None:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def _github_token() -> str | None:
    for env_name in ("GH_TOKEN", "GITHUB_TOKEN"):
        token = os.environ.get(env_name)
        if token:
            return token
    return None


def _normalized_package_version(version: str) -> str:
    return version.strip().replace("-alpha.", "a").replace("-beta.", "b")


__all__ = [
    "PACKAGE_NAME",
    "RUNTIME_VERSION_ENV_VAR",
    "RuntimeSetupError",
    "configured_runtime_version",
    "ensure_runtime_package_installed",
    "platform_asset_name",
    "required_runtime_version",
]
