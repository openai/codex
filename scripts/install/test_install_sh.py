#!/usr/bin/env python3

import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import textwrap
import unittest


INSTALL_SCRIPT = Path(__file__).with_name("install.sh")
VERSION = "0.142.5"
LEGACY_VERSION = "0.125.0"


class InstallShTest(unittest.TestCase):
    def test_metadata_fetch_failure_is_not_reported_as_missing_assets(self) -> None:
        result, requests = run_installer(VERSION, metadata_failure=True)

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://api.github.com/repos/openai/codex/releases/tags/"
                f"rust-v{VERSION}"
            ],
        )
        self.assertIn(
            f"Could not fetch GitHub release metadata for Codex {VERSION}",
            result.stderr,
        )
        self.assertNotIn("Could not find Codex package", result.stderr)

    def test_exact_release_fetches_metadata_once(self) -> None:
        result, requests = run_installer(VERSION)

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://api.github.com/repos/openai/codex/releases/tags/"
                f"rust-v{VERSION}",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{VERSION}/codex-package_SHA256SUMS",
            ],
        )
        self.assertIn(f"Resolved version: {VERSION}", result.stdout)

    def test_latest_release_reuses_version_metadata(self) -> None:
        result, requests = run_installer("latest")

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://api.github.com/repos/openai/codex/releases/latest",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{VERSION}/codex-package_SHA256SUMS",
            ],
        )
        self.assertIn(f"Resolved version: {VERSION}", result.stdout)

    def test_releases_channel_unavailable_falls_back_to_github(self) -> None:
        result, requests = run_installer(
            "latest", use_releases=True, releases_mode="channel_failure"
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://releases.openai.com/codex/channels/latest",
                "https://api.github.com/repos/openai/codex/releases/latest",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{VERSION}/codex-package_SHA256SUMS",
            ],
        )
        self.assertIn("falling back to GitHub Releases", result.stderr)

    def test_releases_integrity_failure_does_not_fall_back(self) -> None:
        result, requests = run_installer(
            "latest", use_releases=True, releases_mode="corrupt_package"
        )

        self.assertNotEqual(result.returncode, 0)
        package = f"codex-package-{current_target()}.tar.gz"
        self.assertEqual(
            requests,
            [
                "https://releases.openai.com/codex/channels/latest",
                f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                f"https://releases.openai.com/codex/releases/{VERSION}/{package}",
            ],
        )
        self.assertIn("checksum did not match", result.stderr)

    def test_releases_latest_installs_verified_package(self) -> None:
        result, requests = run_installer("latest", use_releases=True)

        package = f"codex-package-{current_target()}.tar.gz"
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            requests,
            [
                "https://releases.openai.com/codex/channels/latest",
                f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                f"https://releases.openai.com/codex/releases/{VERSION}/{package}",
            ],
        )

    def test_releases_latest_rejects_corrupt_checksum_manifest(self) -> None:
        result, requests = run_installer(
            "latest", use_releases=True, releases_mode="corrupt_manifest"
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://releases.openai.com/codex/channels/latest",
                f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
            ],
        )
        self.assertIn("checksum did not match", result.stderr)

    def test_exact_releases_path_discovers_legacy_github_asset(self) -> None:
        result, requests = run_installer(
            LEGACY_VERSION, use_releases=True, releases_mode="legacy"
        )

        target = current_target()
        package = f"codex-package-{target}.tar.gz"
        legacy_package = f"codex-npm-{current_npm_tag()}-{LEGACY_VERSION}.tgz"
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            requests,
            [
                f"https://releases.openai.com/codex/releases/{LEGACY_VERSION}/codex-package_SHA256SUMS",
                f"https://releases.openai.com/codex/releases/{LEGACY_VERSION}/{package}",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{LEGACY_VERSION}/{package}",
                "https://api.github.com/repos/openai/codex/releases/tags/"
                f"rust-v{LEGACY_VERSION}",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{LEGACY_VERSION}/{legacy_package}",
            ],
        )
        self.assertIn("retrying from GitHub Releases", result.stderr)
        self.assertIn("checking GitHub release metadata", result.stderr)


def run_installer(
    release: str,
    *,
    metadata_failure: bool = False,
    use_releases: bool = False,
    releases_mode: str = "",
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        bin_dir = root / "bin"
        bin_dir.mkdir()
        (root / "home").mkdir()
        package_archive = root / "package.tar.gz"
        legacy_archive = root / "legacy.tgz"
        create_package_archive(package_archive, VERSION)
        create_legacy_archive(legacy_archive, LEGACY_VERSION)
        package_digest = sha256_file(package_archive)
        legacy_digest = sha256_file(legacy_archive)
        checksum_file = root / "codex-package_SHA256SUMS"
        checksum_file.write_text(
            checksum_manifest(package_digest),
            encoding="utf-8",
        )
        checksum_digest = sha256_file(checksum_file)
        request_log = root / "requests.log"
        fake_curl = bin_dir / "curl"
        fake_curl.write_text(
            textwrap.dedent(
                """\
                #!/bin/sh
                url=""
                output=""
                next_is_output="false"
                for arg in "$@"; do
                  case "$arg" in
                    https://*) url="$arg" ;;
                  esac
                  if [ "$next_is_output" = "true" ]; then
                    output="$arg"
                    next_is_output="false"
                  elif [ "$arg" = "-o" ]; then
                    next_is_output="true"
                  fi
                done
                printf '%s\n' "$url" >>"$CODEX_TEST_REQUEST_LOG"

                case "$url" in
                  https://api.github.com/*)
                    if [ "$CODEX_TEST_METADATA_FAILURE" = "1" ]; then
                      echo "curl: (22) The requested URL returned error: 403" >&2
                      exit 22
                    fi
                    if [ "$CODEX_TEST_RELEASES_MODE" = "legacy" ]; then
                      printf '%s\n' "$CODEX_TEST_LEGACY_METADATA_JSON"
                    else
                      printf '%s\n' "$CODEX_TEST_GITHUB_METADATA_JSON"
                    fi
                    ;;
                  https://releases.openai.com/codex/channels/latest)
                    if [ "$CODEX_TEST_RELEASES_MODE" = "channel_failure" ]; then
                      exit 22
                    fi
                    printf '%s\n' "$CODEX_TEST_LATEST_METADATA_JSON"
                    ;;
                  https://releases.openai.com/codex/releases/*/codex-package_SHA256SUMS)
                    if [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_manifest" ]; then
                      printf 'corrupt' >"$output"
                    else
                      cp "$CODEX_TEST_CHECKSUM_FILE" "$output"
                    fi
                    ;;
                  https://releases.openai.com/codex/releases/*/codex-package-*.tar.gz)
                    if [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_package" ]; then
                      printf 'corrupt' >"$output"
                      exit 0
                    fi
                    if [ "$CODEX_TEST_RELEASES_MODE" = "legacy" ]; then
                      exit 22
                    fi
                    cp "$CODEX_TEST_PACKAGE_ARCHIVE" "$output"
                    ;;
                  https://github.com/openai/codex/releases/download/*/codex-package_SHA256SUMS)
                    exit 22
                    ;;
                  https://github.com/openai/codex/releases/download/*/codex-package-*.tar.gz)
                    exit 22
                    ;;
                  https://github.com/openai/codex/releases/download/*/codex-npm-*.tgz)
                    cp "$CODEX_TEST_LEGACY_ARCHIVE" "$output"
                    ;;
                  *)
                    exit 22
                    ;;
                esac
                """
            ),
            encoding="utf-8",
        )
        fake_curl.chmod(0o755)

        env = os.environ.copy()
        env.update(
            {
                "CODEX_HOME": str(root / "codex-home"),
                "CODEX_INSTALL_DIR": str(root / "install-bin"),
                "CODEX_NON_INTERACTIVE": "1",
                "CODEX_RELEASE": release,
                "CODEX_INSTALLER_USE_RELEASES_OPENAI_COM": (
                    "TRUE" if use_releases else "0"
                ),
                "CODEX_TEST_CHECKSUM_FILE": str(checksum_file),
                "CODEX_TEST_GITHUB_METADATA_JSON": release_metadata(
                    VERSION,
                    package_digest,
                    checksum_digest,
                    "https://github.com/openai/codex/releases/download",
                ),
                "CODEX_TEST_LEGACY_ARCHIVE": str(legacy_archive),
                "CODEX_TEST_LEGACY_METADATA_JSON": legacy_release_metadata(
                    LEGACY_VERSION,
                    legacy_digest,
                ),
                "CODEX_TEST_METADATA_FAILURE": "1" if metadata_failure else "0",
                "CODEX_TEST_PACKAGE_ARCHIVE": str(package_archive),
                "CODEX_TEST_RELEASES_MODE": releases_mode,
                "CODEX_TEST_REQUEST_LOG": str(request_log),
                "CODEX_TEST_LATEST_METADATA_JSON": release_metadata(
                    VERSION,
                    package_digest,
                    checksum_digest,
                    "https://releases.openai.com/codex/releases",
                ),
                "HOME": str(root / "home"),
                "PATH": f"{bin_dir}:/usr/bin:/bin",
                "SHELL": "/bin/sh",
            }
        )
        result = subprocess.run(
            ["/bin/sh", str(INSTALL_SCRIPT)],
            capture_output=True,
            check=False,
            env=env,
            text=True,
        )
        requests = (
            request_log.read_text(encoding="utf-8").splitlines()
            if request_log.exists()
            else []
        )
        return result, requests


def release_metadata(
    version: str,
    package_digest: str,
    checksum_digest: str,
    download_base_url: str,
) -> str:
    release_path = f"rust-v{version}" if "github.com" in download_base_url else version
    asset_base_url = f"{download_base_url}/{release_path}"
    assets = [
        {
            "name": f"codex-package-{target}.tar.gz",
            "digest": f"sha256:{package_digest}",
            "browser_download_url": f"{asset_base_url}/codex-package-{target}.tar.gz",
        }
        for target in (
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
        )
    ]
    assets.append(
        {
            "name": "codex-package_SHA256SUMS",
            "digest": f"sha256:{checksum_digest}",
            "browser_download_url": f"{asset_base_url}/codex-package_SHA256SUMS",
        }
    )
    return json.dumps(
        {"tag_name": f"rust-v{version}", "assets": assets},
        indent=2,
    )


def legacy_release_metadata(version: str, digest: str) -> str:
    asset = f"codex-npm-{current_npm_tag()}-{version}.tgz"
    return json.dumps(
        {
            "tag_name": f"rust-v{version}",
            "assets": [
                {
                    "name": asset,
                    "digest": f"sha256:{digest}",
                    "browser_download_url": (
                        "https://github.com/openai/codex/releases/download/"
                        f"rust-v{version}/{asset}"
                    ),
                }
            ],
        },
        indent=2,
    )


def current_target() -> str:
    os_name = subprocess.run(
        ["uname", "-s"], capture_output=True, check=True, text=True
    ).stdout.strip()
    architecture = subprocess.run(
        ["uname", "-m"], capture_output=True, check=True, text=True
    ).stdout.strip()
    arch = "aarch64" if architecture in ("arm64", "aarch64") else "x86_64"
    return (
        f"{arch}-apple-darwin" if os_name == "Darwin" else f"{arch}-unknown-linux-musl"
    )


def current_npm_tag() -> str:
    target = current_target()
    return {
        "aarch64-apple-darwin": "darwin-arm64",
        "x86_64-apple-darwin": "darwin-x64",
        "aarch64-unknown-linux-musl": "linux-arm64",
        "x86_64-unknown-linux-musl": "linux-x64",
    }[target]


def checksum_manifest(package_digest: str) -> str:
    return "".join(
        f"{package_digest}  codex-package-{target}.tar.gz\n"
        for target in (
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
            "aarch64-pc-windows-msvc",
            "x86_64-pc-windows-msvc",
        )
    )


def create_package_archive(path: Path, version: str) -> None:
    executable = f"#!/bin/sh\necho 'codex-cli {version}'\n".encode()
    with tarfile.open(path, "w:gz") as archive:
        add_archive_file(archive, "codex-package.json", b"{}\n")
        add_archive_file(archive, "bin/codex", executable, mode=0o755)
        add_archive_file(archive, "bin/codex-code-mode-host", executable, mode=0o755)
        add_archive_file(archive, "codex-path/rg", executable, mode=0o755)
        add_archive_file(archive, "codex-resources/bwrap", executable, mode=0o755)


def create_legacy_archive(path: Path, version: str) -> None:
    target = current_target()
    executable = f"#!/bin/sh\necho 'codex-cli {version}'\n".encode()
    with tarfile.open(path, "w:gz") as archive:
        add_archive_file(
            archive,
            f"package/vendor/{target}/codex/codex",
            executable,
            mode=0o755,
        )
        add_archive_file(
            archive,
            f"package/vendor/{target}/path/rg",
            executable,
            mode=0o755,
        )
        add_archive_file(
            archive,
            f"package/vendor/{target}/codex-resources/bwrap",
            executable,
            mode=0o755,
        )


def add_archive_file(
    archive: tarfile.TarFile,
    name: str,
    content: bytes,
    *,
    mode: int = 0o644,
) -> None:
    info = tarfile.TarInfo(name)
    info.size = len(content)
    info.mode = mode
    archive.addfile(info, io.BytesIO(content))


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


if __name__ == "__main__":
    unittest.main()
