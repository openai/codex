#!/usr/bin/env python3

from __future__ import annotations

import unittest

import run_bazel_with_buildbuddy


class RunBazelWithBuildBuddyTest(unittest.TestCase):
    def test_defaults_to_generic_buildbuddy_host(self) -> None:
        self.assertEqual(
            "buildbuddy-generic",
            run_bazel_with_buildbuddy.remote_config(
                ["build", "//codex-rs/cli:codex"],
                {},
            ),
        )
        self.assertEqual(
            [
                "build",
                "//codex-rs/cli:codex",
                "--config=buildbuddy-generic",
            ],
            run_bazel_with_buildbuddy.bazel_args_with_remote_config(
                ["build", "//codex-rs/cli:codex"],
                {},
            ),
        )

    def test_upstream_opt_in_selects_openai_rbe_before_target_separator(self) -> None:
        env = {
            "BUILDBUDDY_API_KEY": "token",
            "CODEX_BAZEL_USE_OPENAI_BUILDBUDDY_HOST": "true",
            "GITHUB_REPOSITORY": "openai/codex",
        }

        self.assertEqual(
            [
                "build",
                "--config=ci-linux",
                "--config=buildbuddy-openai-rbe",
                "--remote_header=x-buildbuddy-api-key=token",
                "--",
                "//codex-rs/cli:codex",
            ],
            run_bazel_with_buildbuddy.bazel_args_with_remote_config(
                ["build", "--config=ci-linux", "--", "//codex-rs/cli:codex"],
                env,
            ),
        )

    def test_fork_repository_cannot_select_openai_host(self) -> None:
        env = {
            "BUILDBUDDY_API_KEY": "fork-token",
            "CODEX_BAZEL_USE_OPENAI_BUILDBUDDY_HOST": "true",
            "GITHUB_REPOSITORY": "contributor/codex",
        }

        self.assertEqual(
            "buildbuddy-generic-rbe",
            run_bazel_with_buildbuddy.remote_config(["build", "--config=ci-v8"], env),
        )
        self.assertEqual(
            [
                "build",
                "--config=ci-v8",
                "//third_party/v8:release",
                "--config=buildbuddy-generic-rbe",
                "--remote_header=x-buildbuddy-api-key=fork-token",
            ],
            run_bazel_with_buildbuddy.bazel_args_with_remote_config(
                ["build", "--config=ci-v8", "//third_party/v8:release"],
                env,
            ),
        )

    def test_openai_opt_in_requires_repository_identity(self) -> None:
        for repository in (None, ""):
            env = {
                "BUILDBUDDY_API_KEY": "token",
                "CODEX_BAZEL_USE_OPENAI_BUILDBUDDY_HOST": "true",
            }
            if repository is not None:
                env["GITHUB_REPOSITORY"] = repository

            with self.subTest(repository=repository):
                with self.assertRaisesRegex(
                    RuntimeError, "GITHUB_REPOSITORY must be set"
                ):
                    run_bazel_with_buildbuddy.remote_config(["build"], env)

    def test_bazel_command_uses_configured_binary(self) -> None:
        self.assertEqual(
            ["fake-bazel", "info", "execution_root", "--config=buildbuddy-generic"],
            run_bazel_with_buildbuddy.bazel_command(
                "info",
                "execution_root",
                env={"CODEX_BAZEL_BIN": "fake-bazel"},
            ),
        )


if __name__ == "__main__":
    unittest.main()
