#!/usr/bin/env python3

import json
import os
import stat
import subprocess
import sys
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import run_bazel_ci


class RunBazelCiTest(unittest.TestCase):
    def test_parse_args_preserves_bazel_args_and_targets(self) -> None:
        options, bazel_args, targets = run_bazel_ci.parse_args(
            [
                "--print-failed-test-logs",
                "--remote-download-toplevel",
                "--",
                "test",
                "--keep_going",
                "--",
                "//codex-rs/...",
                "-//third_party/v8:all",
            ]
        )

        self.assertEqual(
            options,
            run_bazel_ci.Options(
                print_failed_test_logs=True,
                remote_download_toplevel=True,
            ),
        )
        self.assertEqual(bazel_args, ["test", "--keep_going"])
        self.assertEqual(targets, ["//codex-rs/...", "-//third_party/v8:all"])

    def test_linux_remote_invocation_uses_shared_buildbuddy_wrapper(self) -> None:
        invocation = run_bazel_ci.build_invocation(
            run_bazel_ci.Options(remote_download_toplevel=True),
            ["test", "--keep_going"],
            ["//codex-rs/..."],
            {
                "BAZEL_OUTPUT_USER_ROOT": "/tmp/output",
                "BUILDBUDDY_API_KEY": "token",
                "CODEX_BAZEL_BIN": "fake-bazel",
            },
            pid=123,
        )

        self.assertEqual(
            invocation.command,
            [
                "fake-bazel",
                "--output_user_root=/tmp/output",
                "--noexperimental_remote_repo_contents_cache",
                "test",
                "--config=buildbuddy-generic-rbe",
                "--remote_header=x-buildbuddy-api-key=token",
                "--keep_going",
                "--config=ci-linux",
                "--remote_download_toplevel",
                "--",
                "//codex-rs/...",
            ],
        )

    def test_keyless_windows_cross_compile_falls_back_to_local_msvc(self) -> None:
        invocation = run_bazel_ci.build_invocation(
            run_bazel_ci.Options(windows_cross_compile=True),
            ["build"],
            ["//codex-rs/cli:codex"],
            {
                "CODEX_BAZEL_BIN": "fake-bazel",
                "CODEX_BAZEL_WINDOWS_PATH": r"C:\Windows",
                "RUNNER_OS": "Windows",
            },
            pid=123,
        )

        self.assertEqual(invocation.ci_config, "ci-windows")
        self.assertIn("--host_platform=//:local_windows_msvc", invocation.command)
        self.assertIn("--jobs=8", invocation.command)
        self.assertIn(r"--test_env=PATH=C:\Windows", invocation.command)
        self.assertEqual(invocation.child_env["MSYS2_ARG_CONV_EXCL"], "*")

    def test_remote_windows_cross_compile_uses_linux_build_environment(self) -> None:
        invocation = run_bazel_ci.build_invocation(
            run_bazel_ci.Options(windows_cross_compile=True),
            ["test"],
            ["//codex-rs/..."],
            {
                "BUILDBUDDY_API_KEY": "token",
                "CODEX_BAZEL_BIN": "fake-bazel",
                "CODEX_BAZEL_WINDOWS_PATH": r"C:\Windows",
                "RUNNER_OS": "Windows",
            },
            pid=123,
        )

        self.assertEqual(invocation.ci_config, "ci-windows-cross")
        self.assertIn("--host_platform=//:rbe", invocation.command)
        self.assertIn("--action_env=PATH=/usr/bin:/bin", invocation.command)
        self.assertNotIn(r"--action_env=PATH=C:\Windows", invocation.command)

    def test_action_failure_summary_keeps_rust_diagnostics(self) -> None:
        output = "\n".join(
            [
                "INFO: building",
                "ERROR: /tmp/BUILD:1:1 Rustc failed:",
                "error[E0308]: mismatched types",
                "  --> src/main.rs:1:1",
                "   |",
                " 1 | nope",
                "   | ^^^^",
                "note: expected unit",
                "Target //other:thing failed to build",
            ]
        )

        self.assertEqual(
            run_bazel_ci.action_failure_summary(output),
            "\n".join(
                [
                    "ERROR: /tmp/BUILD:1:1 Rustc failed:",
                    "error[E0308]: mismatched types",
                    "  --> src/main.rs:1:1",
                    "   |",
                    " 1 | nope",
                    "   | ^^^^",
                    "note: expected unit",
                ]
            ),
        )

    def test_failed_test_log_resolution_prefers_reported_path(self) -> None:
        console = (
            "FAIL: //codex-rs/core:core-tests (see "
            r"C:\tmp\core-tests\test.log)"
        )

        self.assertEqual(
            run_bazel_ci.failed_test_targets(console),
            ["//codex-rs/core:core-tests"],
        )
        self.assertEqual(
            run_bazel_ci.test_log_path(
                console,
                Path("bazel-testlogs"),
                "//codex-rs/core:core-tests",
            ),
            Path("C:/tmp/core-tests/test.log"),
        )

    def test_print_test_log_tails_only_reads_last_200_lines(self) -> None:
        with TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            test_log = temp / "test.log"
            test_log.write_text("".join(f"line-{index}\n" for index in range(250)))
            console_log = temp / "console.log"
            console_log.write_text(
                f"FAIL: //codex-rs/core:core-tests (see {test_log})\n"
            )
            invocation = run_bazel_ci.Invocation([], {}, "ci-linux", [], False)
            output = StringIO()

            with (
                patch.object(run_bazel_ci, "bazel_testlogs_dir", return_value=temp),
                redirect_stdout(output),
            ):
                run_bazel_ci.print_test_log_tails(console_log, invocation)

            self.assertNotIn("line-49\n", output.getvalue())
            self.assertIn("line-50\n", output.getvalue())
            self.assertIn("line-249\n", output.getvalue())

    def test_main_runs_bazel_and_preserves_exit_status(self) -> None:
        with TemporaryDirectory() as temp_dir:
            temp = Path(temp_dir)
            args_path = temp / "args.json"
            fake_bazel = temp / "fake-bazel"
            fake_bazel.write_text(
                "#!/usr/bin/env python3\n"
                "import json, os, sys\n"
                "open(os.environ['ARGS_PATH'], 'w').write(json.dumps(sys.argv[1:]))\n"
                "print('FAILED: fake failure')\n"
                "sys.exit(37)\n",
                encoding="utf-8",
            )
            fake_bazel.chmod(fake_bazel.stat().st_mode | stat.S_IXUSR)
            env = os.environ.copy()
            env.update(
                {
                    "ARGS_PATH": str(args_path),
                    "CODEX_BAZEL_BIN": str(fake_bazel),
                }
            )
            env.pop("BUILDBUDDY_API_KEY", None)

            result = subprocess.run(
                [
                    sys.executable,
                    str(Path(run_bazel_ci.__file__)),
                    "--print-failed-action-summary",
                    "--",
                    "build",
                    "--keep_going",
                    "--",
                    "//codex-rs/cli:codex",
                ],
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )

            self.assertEqual(result.returncode, 37, result.stderr)
            self.assertIn("Bazel failed action diagnostics:", result.stdout)
            self.assertEqual(
                json.loads(args_path.read_text()),
                [
                    "--noexperimental_remote_repo_contents_cache",
                    "build",
                    "--keep_going",
                    "--",
                    "//codex-rs/cli:codex",
                ],
            )


if __name__ == "__main__":
    unittest.main()
