#!/usr/bin/env python3

import os
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import run_bazel


class RunBazelCiTest(unittest.TestCase):
    def test_cache_permissions_and_run_arguments(self) -> None:
        with TemporaryDirectory() as temp_dir:
            credentials = Path(temp_dir) / "job credentials.json"
            credentials.touch()
            for event, ref, authenticated, mode in (
                ("push", "refs/heads/main", True, "write"),
                ("workflow_dispatch", "refs/heads/main", True, "read"),
                ("push", "refs/tags/rust-v1.0.0", True, "read"),
                ("pull_request", "refs/pull/1/merge", False, "local"),
            ):
                with self.subTest(event=event, ref=ref, authenticated=authenticated):
                    env = {
                        "GITHUB_EVENT_NAME": event,
                        "GITHUB_REF": ref,
                        "CODEX_BAZEL_GCS_BUCKET": "example-public",
                    }
                    if authenticated:
                        env["CODEX_BAZEL_GCS_CREDENTIALS"] = str(credentials)
                    flags = []
                    if mode != "local":
                        flags = [
                            "--remote_cache=https://storage.googleapis.com/example-public",
                            f"--google_credentials={credentials}",
                        ]
                    if mode == "read":
                        flags.append("--remote_upload_local_results=false")
                    self.assertEqual(
                        run_bazel.bazel_args_with_remote_config(
                            [
                                "run",
                                "--config=ci-linux",
                                "//:binary",
                                "--",
                                "--user-flag",
                            ],
                            env,
                        ),
                        ["run", "//:binary", *flags, "--", "--user-flag"],
                    )

    def test_macos_cache_connection_limit_respects_explicit_setting(self) -> None:
        with TemporaryDirectory() as temp_dir:
            credentials = Path(temp_dir) / "credentials.json"
            credentials.touch()
            env = {
                "RUNNER_OS": "macOS",
                "GITHUB_EVENT_NAME": "push",
                "GITHUB_REF": "refs/heads/main",
                "CODEX_BAZEL_GCS_BUCKET": "example-public",
                "CODEX_BAZEL_GCS_CREDENTIALS": str(credentials),
            }
            for supplied in (
                [],
                ["--remote_max_connections=8"],
                ["--remote_max_connections", "8"],
            ):
                with self.subTest(supplied=supplied):
                    configured = run_bazel.bazel_args_with_remote_config(
                        ["build", *supplied, "//:binary"], env
                    )
                    expected = [
                        "build",
                        *supplied,
                        "//:binary",
                        "--remote_cache=https://storage.googleapis.com/example-public",
                        f"--google_credentials={credentials}",
                    ]
                    if not supplied:
                        expected.append("--remote_max_connections=20")
                    self.assertEqual(configured, expected)

    def test_bazel_command_uses_configured_binary_locally(self) -> None:
        self.assertEqual(
            run_bazel.bazel_command(
                "info",
                "execution_root",
                env={"CODEX_BAZEL_BIN": "fake-bazel"},
            ),
            ["fake-bazel", "info", "execution_root"],
        )

    def test_bazel_command_normalizes_github_actions_startup_options(self) -> None:
        env = {
            "BAZEL_OUTPUT_USER_ROOT": "/tmp/bazel-output",
            "GITHUB_ACTIONS": "true",
        }

        self.assertEqual(
            run_bazel.bazel_command("build", "//codex-rs/...", env=env),
            [
                "bazel",
                "--output_user_root=/tmp/bazel-output",
                "--noexperimental_remote_repo_contents_cache",
                "build",
                "//codex-rs/...",
                "--disk_cache=",
            ],
        )
        self.assertEqual(
            run_bazel.bazel_command(
                "--experimental_remote_repo_contents_cache",
                "build",
                "//codex-rs/...",
                env=env,
            ),
            [
                "bazel",
                "--output_user_root=/tmp/bazel-output",
                "--experimental_remote_repo_contents_cache",
                "build",
                "//codex-rs/...",
                "--disk_cache=",
            ],
        )

    def test_bazel_command_uses_configured_local_caches(self) -> None:
        env = {
            "BAZEL_REPO_CONTENTS_CACHE": "/tmp/bazel-repo-contents",
            "BAZEL_REPOSITORY_CACHE": "/tmp/bazel-repository",
        }

        self.assertEqual(
            run_bazel.bazel_command(
                "build",
                "--config=local",
                "//codex-rs/...",
                env=env,
            ),
            [
                "bazel",
                "build",
                "--config=local",
                "//codex-rs/...",
                "--repo_contents_cache=/tmp/bazel-repo-contents",
                "--repository_cache=/tmp/bazel-repository",
            ],
        )

    def test_bazel_command_adds_local_caches_before_separator(self) -> None:
        self.assertEqual(
            run_bazel.bazel_command(
                "build",
                "//codex-rs/...",
                "--",
                "--program-arg",
                env={"BAZEL_REPOSITORY_CACHE": "/tmp/bazel-repository"},
            ),
            [
                "bazel",
                "build",
                "//codex-rs/...",
                "--repository_cache=/tmp/bazel-repository",
                "--",
                "--program-arg",
            ],
        )

    def test_main_preserves_spaced_argument_and_child_exit_status(self) -> None:
        spaced_arg = (
            r"--test_env=PATH=C:\Program Files\PowerShell\7;C:\Program Files\Git\bin"
        )
        child_code = (
            f"import sys; sys.exit(37 if sys.argv[1] == {spaced_arg!r} else 91)"
        )
        env = os.environ.copy()
        env["CODEX_BAZEL_BIN"] = sys.executable
        env.pop("BAZEL_OUTPUT_USER_ROOT", None)
        env.pop("GITHUB_ACTIONS", None)

        result = subprocess.run(
            [
                sys.executable,
                str(Path(run_bazel.__file__)),
                "-c",
                child_code,
                spaced_arg,
            ],
            env=env,
            check=False,
            capture_output=True,
            text=True,
        )

        self.assertEqual(result.returncode, 37, result.stderr)


if __name__ == "__main__":
    unittest.main()
