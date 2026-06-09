#!/usr/bin/env python3

import json
import os
import stat
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import run_bazel_query_ci


class RunBazelQueryCiTest(unittest.TestCase):
    def test_query_command_reuses_startup_and_repository_cache_settings(self) -> None:
        expression = 'kind("rust_library rule", //codex-rs/...)'

        self.assertEqual(
            run_bazel_query_ci.query_command(
                ["--keep_going", "--output=label", "--", expression],
                {
                    "BAZEL_OUTPUT_USER_ROOT": "/tmp/output",
                    "BAZEL_REPO_CONTENTS_CACHE": "/tmp/contents",
                    "BAZEL_REPOSITORY_CACHE": "/tmp/repository",
                    "BUILDBUDDY_API_KEY": "token",
                    "CODEX_BAZEL_BIN": "fake-bazel",
                    "GITHUB_ACTIONS": "true",
                },
            ),
            [
                "fake-bazel",
                "--output_user_root=/tmp/output",
                "--noexperimental_remote_repo_contents_cache",
                "query",
                "--config=buildbuddy-generic",
                "--remote_header=x-buildbuddy-api-key=token",
                "--repo_contents_cache=/tmp/contents",
                "--repository_cache=/tmp/repository",
                "--keep_going",
                "--output=label",
                expression,
            ],
        )

    def test_query_command_requires_final_separator_and_expression(self) -> None:
        for args in ([], ["--", "expression", "extra"], ["expression"]):
            with self.subTest(args=args):
                with self.assertRaisesRegex(ValueError, "Usage:"):
                    run_bazel_query_ci.query_command(args, {})

    def test_main_runs_query_with_buildbuddy_configuration(self) -> None:
        with TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            args_path = temp / "args.json"
            fake_bazel = temp / "fake-bazel"
            fake_bazel.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "open(os.environ['ARGS_PATH'], 'w').write(json.dumps(sys.argv[1:]))\n"
                "print('//codex-rs/cli:codex')\n",
                encoding="utf-8",
            )
            fake_bazel.chmod(fake_bazel.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env.update(
                {
                    "ARGS_PATH": str(args_path),
                    "BUILDBUDDY_API_KEY": "token",
                    "CODEX_BAZEL_BIN": str(fake_bazel),
                    "GITHUB_ACTIONS": "true",
                    "GITHUB_REPOSITORY": "contributor/codex",
                }
            )

            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(run_bazel_query_ci.__file__)),
                    "--output=label",
                    "--",
                    "//codex-rs/...",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, "//codex-rs/cli:codex\n")
            self.assertEqual(
                json.loads(args_path.read_text()),
                [
                    "--noexperimental_remote_repo_contents_cache",
                    "query",
                    "--config=buildbuddy-generic",
                    "--remote_header=x-buildbuddy-api-key=token",
                    "--output=label",
                    "//codex-rs/...",
                ],
            )


if __name__ == "__main__":
    unittest.main()
