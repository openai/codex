#!/usr/bin/env python3

import json
import os
from pathlib import Path
import subprocess
import tempfile
import textwrap
import unittest


INSTALL_SCRIPT = Path(__file__).with_name("install.sh")
VERSION = "0.142.5"


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


def run_installer(
    release: str, *, metadata_failure: bool = False
) -> tuple[subprocess.CompletedProcess[str], list[str]]:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        bin_dir = root / "bin"
        bin_dir.mkdir()
        request_log = root / "requests.log"
        fake_curl = bin_dir / "curl"
        fake_curl.write_text(
            textwrap.dedent(
                """\
                #!/bin/sh
                url=""
                for arg in "$@"; do
                  case "$arg" in
                    https://*) url="$arg" ;;
                  esac
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
                "CODEX_TEST_METADATA_FAILURE": "1" if metadata_failure else "0",
                "CODEX_TEST_METADATA_JSON": release_metadata(),
                "CODEX_TEST_REQUEST_LOG": str(request_log),
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


def release_metadata() -> str:
    assets = [
        {
            "name": f"codex-package-{target}.tar.gz",
            "digest": f"sha256:{'a' * 64}",
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
            "digest": f"sha256:{'b' * 64}",
        }
    )
    return json.dumps(
        {"tag_name": f"rust-v{VERSION}", "assets": assets},
        indent=2,
    )


if __name__ == "__main__":
    unittest.main()
