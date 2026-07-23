#!/usr/bin/env python3

import hashlib
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
MISMATCH_VERSION = "0.145.0"


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

    def test_exact_release_opt_out_uses_github_metadata_once(self) -> None:
        result, requests = run_installer(VERSION, use_mirror=False)

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

    def test_alpha_hotfix_release_is_valid(self) -> None:
        version = "0.145.0-alpha.23.1"
        result, requests = run_installer(version)

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(
            requests,
            [
                "https://api.github.com/repos/openai/codex/releases/tags/"
                f"rust-v{version}",
                "https://github.com/openai/codex/releases/download/"
                f"rust-v{version}/codex-package_SHA256SUMS",
            ],
        )
        self.assertIn(f"Resolved version: {version}", result.stdout)

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

    def test_compact_metadata_is_independent_of_field_order(self) -> None:
        result, requests = run_installer(
            "latest", metadata_json=release_metadata(compact=True, reorder=True)
        )

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

    def test_json_like_strings_and_nested_fields_do_not_define_assets(self) -> None:
        result, requests = run_installer(
            VERSION, metadata_json=legacy_release_metadata_with_decoys()
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(requests), 2)
        self.assertIn("/codex-npm-", requests[1])
        self.assertNotIn("codex-package_SHA256SUMS", requests[1])

    def test_macos_install_exposes_code_mode_host_beside_codex(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)

            result, _requests = run_installer_in(
                root,
                VERSION,
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            install_bin = root / "install-bin"
            current = root / "codex-home" / "packages" / "standalone" / "current"
            codex_path = install_bin / "codex"
            host_path = install_bin / "codex-code-mode-host"
            self.assertEqual(os.readlink(codex_path), str(current / "bin" / "codex"))
            self.assertEqual(
                os.readlink(host_path),
                str(current / "bin" / "codex-code-mode-host"),
            )
            self.assertTrue(os.access(host_path, os.X_OK))

    def test_releases_latest_installs_verified_package_by_default(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )

    def test_releases_unusable_metadata_falls_back_to_github(self) -> None:
        unusable_metadata = {
            "html": "<html>proxy error</html>",
            "empty": "",
            "malformed_json": '{"tag_name":',
            "missing_tag": json.dumps({"assets": []}),
            "missing_assets": json.dumps(
                {"tag_name": f"rust-v{VERSION}", "assets": []}
            ),
            "invalid_checksum_digest": json.dumps(
                {
                    "tag_name": f"rust-v{VERSION}",
                    "assets": [
                        {
                            "name": "codex-package-aarch64-apple-darwin.tar.gz",
                            "digest": "sha256:" + "a" * 64,
                        },
                        {
                            "name": "codex-package_SHA256SUMS",
                            "digest": "sha256:" + "z" * 64,
                        },
                    ],
                }
            ),
            "invalid_version": json.dumps({"tag_name": "rust-vinvalid"}),
        }

        for name, releases_metadata_json in unusable_metadata.items():
            with self.subTest(metadata=name):
                with tempfile.TemporaryDirectory() as temp_dir:
                    root = Path(temp_dir)
                    archive_path, checksum_path, metadata_json = create_package_release(
                        root
                    )

                    result, requests = run_installer_in(
                        root,
                        "latest",
                        metadata_json=metadata_json,
                        releases_metadata_json=releases_metadata_json,
                        archive_path=archive_path,
                        checksum_path=checksum_path,
                        force_macos=True,
                        use_mirror=None,
                    )

                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(
                        requests,
                        [
                            "https://releases.openai.com/codex/channels/latest",
                            "https://api.github.com/repos/openai/codex/releases/latest",
                            "https://github.com/openai/codex/releases/download/"
                            f"rust-v{VERSION}/codex-package_SHA256SUMS",
                            "https://github.com/openai/codex/releases/download/"
                            f"rust-v{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                        ],
                    )
                    self.assertIn("falling back to GitHub Releases", result.stderr)

    def test_releases_exact_metadata_version_mismatch_falls_back_to_github(
        self,
    ) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)
            releases_metadata = json.loads(metadata_json)
            releases_metadata["tag_name"] = f"rust-v{MISMATCH_VERSION}"

            result, requests = run_installer_in(
                root,
                VERSION,
                metadata_json=metadata_json,
                releases_metadata_json=json.dumps(releases_metadata),
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    f"https://releases.openai.com/codex/releases/{VERSION}/release.json",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn("falling back to GitHub Releases", result.stderr)

    def test_releases_asset_download_falls_back_to_github(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
                releases_mode="asset_fallback",
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn("retrying from GitHub Releases", result.stderr)

    def test_releases_corrupt_assets_fall_back_to_github(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
                releases_mode="corrupt_assets",
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn("checksum did not match expected digest", result.stderr)
            self.assertIn("retrying from GitHub Releases", result.stderr)

    def test_releases_wrong_checksum_digest_uses_github_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)
            mirror_metadata = json.loads(metadata_json)
            for release_asset in mirror_metadata["assets"]:
                if release_asset["name"] == "codex-package_SHA256SUMS":
                    release_asset["digest"] = "sha256:" + "0" * 64

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                releases_metadata_json=json.dumps(mirror_metadata),
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn("checksum did not match expected digest", result.stderr)

    def test_releases_incomplete_checksum_manifest_falls_back_to_github(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)
            mirror_checksum_path = root / "mirror-SHA256SUMS"
            mirror_checksum_path.write_text(
                f"{'a' * 64}  codex-package-other-platform.tar.gz\n",
                encoding="utf-8",
            )
            mirror_metadata = json.loads(metadata_json)
            for release_asset in mirror_metadata["assets"]:
                if release_asset["name"] == "codex-package_SHA256SUMS":
                    release_asset["digest"] = (
                        "sha256:"
                        + hashlib.sha256(mirror_checksum_path.read_bytes()).hexdigest()
                    )

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                releases_metadata_json=json.dumps(mirror_metadata),
                archive_path=archive_path,
                checksum_path=checksum_path,
                releases_checksum_path=mirror_checksum_path,
                force_macos=True,
                use_mirror=None,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn("retrying from GitHub Releases", result.stderr)

    def test_releases_corrupt_github_fallback_still_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(root)

            result, requests = run_installer_in(
                root,
                "latest",
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=None,
                releases_mode="corrupt_checksum_and_github",
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(
                requests,
                [
                    "https://releases.openai.com/codex/channels/latest",
                    f"https://releases.openai.com/codex/releases/{VERSION}/codex-package_SHA256SUMS",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-package_SHA256SUMS",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                ],
            )
            self.assertIn("checksum did not match expected digest", result.stderr)

    def test_releases_exact_rejects_wrong_binary_version(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, checksum_path, metadata_json = create_package_release(
                root,
                metadata_version=MISMATCH_VERSION,
            )

            result, requests = run_installer_in(
                root,
                MISMATCH_VERSION,
                metadata_json=metadata_json,
                archive_path=archive_path,
                checksum_path=checksum_path,
                force_macos=True,
                use_mirror=True,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(
                requests,
                [
                    f"https://releases.openai.com/codex/releases/{MISMATCH_VERSION}/release.json",
                    f"https://releases.openai.com/codex/releases/{MISMATCH_VERSION}/codex-package_SHA256SUMS",
                    f"https://releases.openai.com/codex/releases/{MISMATCH_VERSION}/codex-package-aarch64-apple-darwin.tar.gz",
                ],
            )
            self.assertIn(
                f"did not report expected version {MISMATCH_VERSION}",
                result.stderr,
            )
            self.assertNotIn("installed successfully", result.stdout)

    def test_releases_exact_legacy_fallback_reuses_offline_install(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive_path, metadata_json = create_legacy_release(root)

            first_result, first_requests = run_installer_in(
                root,
                VERSION,
                metadata_json=metadata_json,
                legacy_archive_path=archive_path,
                force_macos=True,
                use_mirror=True,
                releases_mode="channel_failure",
            )

            self.assertEqual(first_result.returncode, 0, first_result.stderr)
            self.assertEqual(
                first_requests,
                [
                    f"https://releases.openai.com/codex/releases/{VERSION}/release.json",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                    "https://github.com/openai/codex/releases/download/"
                    f"rust-v{VERSION}/codex-npm-darwin-arm64-{VERSION}.tgz",
                ],
            )

            (root / "requests.log").unlink()
            second_result, second_requests = run_installer_in(
                root,
                VERSION,
                metadata_json=metadata_json,
                force_macos=True,
                use_mirror=True,
                releases_mode="channel_failure",
            )

            self.assertEqual(second_result.returncode, 0, second_result.stderr)
            self.assertEqual(
                second_requests,
                [
                    f"https://releases.openai.com/codex/releases/{VERSION}/release.json",
                    "https://api.github.com/repos/openai/codex/releases/tags/"
                    f"rust-v{VERSION}",
                ],
            )
            self.assertNotIn("Downloading Codex CLI", second_result.stdout)


def run_installer(
    release: str,
    *,
    metadata_failure: bool = False,
    metadata_json: str | None = None,
    use_mirror: bool | None = False,
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    with tempfile.TemporaryDirectory() as temp_dir:
        return run_installer_in(
            Path(temp_dir),
            release,
            metadata_failure=metadata_failure,
            metadata_json=metadata_json,
            use_mirror=use_mirror,
        )


def run_installer_in(
    root: Path,
    release: str,
    *,
    metadata_failure: bool = False,
    metadata_json: str | None = None,
    releases_metadata_json: str | None = None,
    archive_path: Path | None = None,
    checksum_path: Path | None = None,
    releases_checksum_path: Path | None = None,
    legacy_archive_path: Path | None = None,
    force_macos: bool = False,
    use_mirror: bool | None = False,
    releases_mode: str = "",
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    bin_dir = root / "bin"
    bin_dir.mkdir(exist_ok=True)
    request_log = root / "requests.log"
    fake_curl = bin_dir / "curl"
    fake_curl.write_text(
        textwrap.dedent(
            """\
            #!/bin/sh
            url=""
            output=""
            previous=""
            for arg in "$@"; do
              case "$arg" in
                https://*) url="$arg" ;;
              esac
              if [ "$previous" = "-o" ]; then
                output="$arg"
              fi
              previous="$arg"
            done
            printf '%s\n' "$url" >>"$CODEX_TEST_REQUEST_LOG"

            case "$url" in
              https://api.github.com/*)
                if [ "$CODEX_TEST_METADATA_FAILURE" = "1" ]; then
                  echo "curl: (22) The requested URL returned error: 403" >&2
                  exit 22
                fi
                printf '%s\n' "$CODEX_TEST_METADATA_JSON"
                ;;
              https://releases.openai.com/codex/channels/latest|https://releases.openai.com/codex/releases/*/release.json)
                if [ "$CODEX_TEST_RELEASES_MODE" = "channel_failure" ]; then
                  exit 22
                fi
                printf '%s\n' "$CODEX_TEST_RELEASES_METADATA_JSON"
                ;;
              https://releases.openai.com/codex/releases/*/codex-package_SHA256SUMS)
                if [ "$CODEX_TEST_RELEASES_MODE" = "asset_fallback" ]; then
                  exit 22
                fi
                if [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_assets" ] ||
                  [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_checksum_and_github" ]; then
                  printf '<html>proxy error</html>\n' >"$output"
                  exit 0
                fi
                if [ -n "$CODEX_TEST_RELEASES_CHECKSUM_PATH" ]; then
                  cp "$CODEX_TEST_RELEASES_CHECKSUM_PATH" "$output"
                else
                  exit 22
                fi
                ;;
              https://releases.openai.com/codex/releases/*/codex-package-*.tar.gz)
                if [ "$CODEX_TEST_RELEASES_MODE" = "asset_fallback" ]; then
                  exit 22
                fi
                if [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_assets" ]; then
                  printf '<html>proxy error</html>\n' >"$output"
                  exit 0
                fi
                if [ -n "$CODEX_TEST_ARCHIVE_PATH" ]; then
                  cp "$CODEX_TEST_ARCHIVE_PATH" "$output"
                else
                  exit 22
                fi
                ;;
              https://github.com/openai/codex/releases/download/*/codex-package_SHA256SUMS)
                if [ "$CODEX_TEST_RELEASES_MODE" = "corrupt_checksum_and_github" ]; then
                  printf '<html>proxy error</html>\n' >"$output"
                  exit 0
                fi
                if [ -n "$CODEX_TEST_CHECKSUM_PATH" ]; then
                  cp "$CODEX_TEST_CHECKSUM_PATH" "$output"
                else
                  exit 22
                fi
                ;;
              https://github.com/openai/codex/releases/download/*/codex-package-*.tar.gz)
                if [ -n "$CODEX_TEST_ARCHIVE_PATH" ]; then
                  cp "$CODEX_TEST_ARCHIVE_PATH" "$output"
                else
                  exit 22
                fi
                ;;
              https://github.com/openai/codex/releases/download/*/codex-npm-*.tgz)
                if [ -n "$CODEX_TEST_LEGACY_ARCHIVE_PATH" ]; then
                  cp "$CODEX_TEST_LEGACY_ARCHIVE_PATH" "$output"
                else
                  exit 22
                fi
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
    if force_macos:
        fake_uname = bin_dir / "uname"
        fake_uname.write_text(
            "#!/bin/sh\n"
            'case "$1" in\n'
            "  -s) printf 'Darwin\\n' ;;\n"
            "  -m) printf 'arm64\\n' ;;\n"
            "esac\n",
            encoding="utf-8",
        )
        fake_uname.chmod(0o755)

    home = root / "home"
    home.mkdir(exist_ok=True)
    env = os.environ.copy()
    env.update(
        {
            "CODEX_HOME": str(root / "codex-home"),
            "CODEX_INSTALL_DIR": str(root / "install-bin"),
            "CODEX_NON_INTERACTIVE": "1",
            "CODEX_RELEASE": release,
            "CODEX_TEST_ARCHIVE_PATH": str(archive_path or ""),
            "CODEX_TEST_CHECKSUM_PATH": str(checksum_path or ""),
            "CODEX_TEST_RELEASES_CHECKSUM_PATH": str(
                releases_checksum_path or checksum_path or ""
            ),
            "CODEX_TEST_LEGACY_ARCHIVE_PATH": str(legacy_archive_path or ""),
            "CODEX_TEST_METADATA_FAILURE": "1" if metadata_failure else "0",
            "CODEX_TEST_METADATA_JSON": (
                metadata_json if metadata_json is not None else release_metadata()
            ),
            "CODEX_TEST_RELEASES_METADATA_JSON": (
                releases_metadata_json
                if releases_metadata_json is not None
                else metadata_json
                if metadata_json is not None
                else release_metadata()
            ),
            "CODEX_TEST_RELEASES_MODE": releases_mode,
            "CODEX_TEST_REQUEST_LOG": str(request_log),
            "HOME": str(home),
            "PATH": f"{bin_dir}:/usr/bin:/bin",
            "SHELL": "/bin/sh",
        }
    )
    if use_mirror is None:
        env.pop("CODEX_INSTALLER_USE_RELEASES_OPENAI_COM", None)
    else:
        env["CODEX_INSTALLER_USE_RELEASES_OPENAI_COM"] = (
            "TRUE" if use_mirror else "false"
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


def create_package_release(
    root: Path,
    *,
    metadata_version: str = VERSION,
) -> tuple[Path, Path, str]:
    package_dir = root / "package"
    (package_dir / "bin").mkdir(parents=True)
    (package_dir / "codex-path").mkdir()
    (package_dir / "codex-package.json").write_text("{}\n", encoding="utf-8")
    write_executable(
        package_dir / "bin" / "codex",
        f"#!/bin/sh\nprintf 'codex-cli {VERSION}\\n'\n",
    )
    write_executable(
        package_dir / "bin" / "codex-code-mode-host",
        "#!/bin/sh\nexit 0\n",
    )
    write_executable(package_dir / "codex-path" / "rg", "#!/bin/sh\nexit 0\n")

    asset = "codex-package-aarch64-apple-darwin.tar.gz"
    archive_path = root / asset
    with tarfile.open(archive_path, "w:gz") as archive:
        for path in package_dir.iterdir():
            archive.add(path, arcname=path.name)

    archive_digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    checksum_path = root / "codex-package_SHA256SUMS"
    checksum_path.write_text(f"{archive_digest}  {asset}\n", encoding="utf-8")
    checksum_digest = hashlib.sha256(checksum_path.read_bytes()).hexdigest()
    metadata_json = json.dumps(
        {
            "assets": [
                {"name": asset, "digest": f"sha256:{archive_digest}"},
                {
                    "name": "codex-package_SHA256SUMS",
                    "digest": f"sha256:{checksum_digest}",
                },
            ],
            "tag_name": f"rust-v{metadata_version}",
        },
        indent=2,
    )
    return archive_path, checksum_path, metadata_json


def create_legacy_release(root: Path) -> tuple[Path, str]:
    package_dir = root / "legacy-package"
    vendor_dir = package_dir / "package" / "vendor" / "aarch64-apple-darwin"
    (vendor_dir / "codex").mkdir(parents=True)
    (vendor_dir / "path").mkdir()
    write_executable(
        vendor_dir / "codex" / "codex",
        f"#!/bin/sh\nprintf 'codex-cli {VERSION}\\n'\n",
    )
    write_executable(vendor_dir / "path" / "rg", "#!/bin/sh\nexit 0\n")

    asset = f"codex-npm-darwin-arm64-{VERSION}.tgz"
    archive_path = root / asset
    with tarfile.open(archive_path, "w:gz") as archive:
        archive.add(package_dir / "package", arcname="package")

    archive_digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    metadata_json = json.dumps(
        {
            "assets": [{"name": asset, "digest": f"sha256:{archive_digest}"}],
            "tag_name": f"rust-v{VERSION}",
        },
        indent=2,
    )
    return archive_path, metadata_json


def write_executable(path: Path, contents: str) -> None:
    path.write_text(contents, encoding="utf-8")
    path.chmod(0o755)


def release_metadata(*, compact: bool = False, reorder: bool = False) -> str:
    assets = [
        asset_metadata(
            f"codex-package-{target}.tar.gz",
            f"sha256:{'a' * 64}",
            reorder=reorder,
        )
        for target in (
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
        )
    ]
    assets.append(
        asset_metadata(
            "codex-package_SHA256SUMS",
            f"sha256:{'b' * 64}",
            reorder=reorder,
        )
    )
    separators = (",", ":") if compact else None
    return json.dumps(
        {"assets": assets, "body": "braces: { } [ ]", "tag_name": f"rust-v{VERSION}"},
        indent=None if compact else 2,
        separators=separators,
    )


def asset_metadata(name: str, digest: str, *, reorder: bool) -> dict[str, str]:
    if reorder:
        return {"digest": digest, "name": name}
    return {"name": name, "digest": digest}


def legacy_release_metadata_with_decoys() -> str:
    fake_digest = f"sha256:{'0' * 64}"
    assets = [
        {
            "metadata": {
                "name": "codex-package-x86_64-unknown-linux-musl.tar.gz",
                "digest": fake_digest,
            },
            "digest": f"sha256:{'c' * 64}",
            "name": f"codex-npm-{target}-{VERSION}.tgz",
        }
        for target in ("darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64")
    ]
    return json.dumps(
        {
            "body": (
                f'fake: {{"name":"codex-package_SHA256SUMS","digest":"{fake_digest}"}}'
            ),
            "assets": assets,
            "tag_name": f"rust-v{VERSION}",
        },
        separators=(",", ":"),
    )


if __name__ == "__main__":
    unittest.main()
