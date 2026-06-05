#!/usr/bin/env python3

import json
import os
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory


REPO_ROOT = Path(__file__).resolve().parents[2]
RELEASE_SCRIPT = REPO_ROOT / "scripts" / "list-bazel-release-targets.sh"
CLIPPY_SCRIPT = REPO_ROOT / "scripts" / "list-bazel-clippy-targets.sh"


class BazelTargetListsTest(unittest.TestCase):
    def run_selector(
        self,
        script: Path,
        *args: str,
        production_targets: tuple[str, ...] = (
            "//codex-rs/cli:codex",
            "//codex-rs/core:core",
        ),
        test_targets: tuple[str, ...] = (
            "//codex-rs/core:core-unit-tests-bin",
            "//codex-rs/core:core-all-test-bin",
            "//codex-rs/core:core-all-test-windows-cross-bin",
        ),
        fail_query_containing: str = "",
    ) -> tuple[subprocess.CompletedProcess[str], list[list[str]]]:
        with TemporaryDirectory() as temp_dir:
            temp_path = Path(temp_dir)
            query_log = temp_path / "queries.jsonl"
            fake_query = temp_path / "fake-bazel-query.py"
            fake_query.write_text(
                """#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path

with Path(os.environ["QUERY_LOG"]).open("a", encoding="utf-8") as query_log:
    query_log.write(json.dumps(sys.argv[1:]) + "\\n")

query = sys.argv[-1]
if os.environ["FAIL_QUERY_CONTAINING"] and os.environ["FAIL_QUERY_CONTAINING"] in query:
    raise SystemExit("injected query failure")
if "rust_(binary|library|proc_macro) rule" in query:
    print(os.environ["PRODUCTION_TARGETS"])
elif "rust_test rule" in query:
    print(os.environ["TEST_TARGETS"])
else:
    raise SystemExit(f"unexpected query: {query}")
""",
                encoding="utf-8",
            )
            fake_query.chmod(0o755)

            env = os.environ.copy()
            env.update(
                {
                    "CODEX_BAZEL_QUERY_SCRIPT": str(fake_query),
                    "FAIL_QUERY_CONTAINING": fail_query_containing,
                    "PRODUCTION_TARGETS": "\n".join(production_targets),
                    "QUERY_LOG": str(query_log),
                    "TEST_TARGETS": "\n".join(test_targets),
                }
            )
            result = subprocess.run(
                ["bash", str(script), *args],
                cwd=REPO_ROOT,
                env=env,
                check=False,
                capture_output=True,
                text=True,
            )
            queries = [
                json.loads(line)
                for line in query_log.read_text(encoding="utf-8").splitlines()
            ]
            return result, queries

    def test_release_targets_are_sorted_explicit_production_rules(self) -> None:
        result, queries = self.run_selector(
            RELEASE_SCRIPT,
            production_targets=(
                "//codex-rs/core:core",
                "//codex-rs/cli:codex",
            ),
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            ["//codex-rs/cli:codex", "//codex-rs/core:core"],
        )
        self.assertEqual(len(queries), 1)
        self.assertIn("rust_(binary|library|proc_macro) rule", queries[0][-1])
        self.assertIn("//visibility:public", queries[0][-1])
        self.assertNotIn("rust_test rule", queries[0][-1])

    def test_release_targets_fail_when_discovery_is_empty(self) -> None:
        result, _ = self.run_selector(RELEASE_SCRIPT, production_targets=())

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No Bazel release-build targets found.", result.stderr)

    def test_native_clippy_adds_manual_native_test_binaries(self) -> None:
        result, queries = self.run_selector(CLIPPY_SCRIPT)

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            [
                "//codex-rs/cli:codex",
                "//codex-rs/core:core",
                "//codex-rs/core:core-all-test-bin",
                "//codex-rs/core:core-unit-tests-bin",
            ],
        )
        self.assertEqual(len(queries), 2)
        self.assertIn("rust_test rule", queries[1][-1])

    def test_cross_clippy_never_queries_or_returns_test_rules(self) -> None:
        result, queries = self.run_selector(
            CLIPPY_SCRIPT,
            "--windows-cross-compile",
        )

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.splitlines(),
            ["//codex-rs/cli:codex", "//codex-rs/core:core"],
        )
        self.assertEqual(len(queries), 1)
        self.assertNotIn("rust_test rule", queries[0][-1])

    def test_native_clippy_prints_nothing_when_test_discovery_fails(self) -> None:
        result, queries = self.run_selector(
            CLIPPY_SCRIPT,
            fail_query_containing="rust_test rule",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")
        self.assertEqual(len(queries), 2)


if __name__ == "__main__":
    unittest.main()
