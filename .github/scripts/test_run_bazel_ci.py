#!/usr/bin/env python3

from __future__ import annotations

import os
import subprocess
import tempfile
import textwrap
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
RUN_BAZEL_CI = REPO_ROOT / ".github" / "scripts" / "run-bazel-ci.sh"


class RunBazelCiTest(unittest.TestCase):
    def test_local_fallback_disables_remote_services(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            fake_bin = temp_path / "bin"
            fake_bin.mkdir()
            args_path = temp_path / "bazel-args.txt"
            fake_bazel = fake_bin / "bazel"
            fake_bazel.write_text(
                textwrap.dedent(
                    f"""\
                    #!/usr/bin/env bash
                    printf '%s\n' "$@" > {args_path}
                    """
                ),
                encoding="utf-8",
            )
            fake_bazel.chmod(0o755)

            env = os.environ.copy()
            env.pop("BUILDBUDDY_API_KEY", None)
            env["PATH"] = f"{fake_bin}:{env['PATH']}"

            subprocess.run(
                [
                    str(RUN_BAZEL_CI),
                    "--",
                    "build",
                    "--",
                    "//codex-rs/protocol:protocol",
                ],
                cwd=REPO_ROOT,
                env=env,
                check=True,
                text=True,
                capture_output=True,
            )

            self.assertEqual(
                args_path.read_text(encoding="utf-8").splitlines(),
                [
                    "--noexperimental_remote_repo_contents_cache",
                    "build",
                    "--remote_cache=",
                    "--remote_executor=",
                    "--experimental_remote_downloader=",
                    "--",
                    "//codex-rs/protocol:protocol",
                ],
            )


if __name__ == "__main__":
    unittest.main()
