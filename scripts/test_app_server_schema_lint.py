#!/usr/bin/env python3

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))

import app_server_schema_lint as lint


class AppServerSchemaLintTest(unittest.TestCase):
    def test_builds_input_from_merge_base_and_current_files(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_current_files(root)
            outputs = {
                ("merge-base", "HEAD", "base-ref"): "abc123\n",
                (
                    "ls-tree",
                    "--name-only",
                    "abc123",
                    "--",
                    lint.KNOWN_BREAKAGES_PATH,
                ): f"{lint.KNOWN_BREAKAGES_PATH}\n",
                (
                    "show",
                    f"abc123:{lint.KNOWN_BREAKAGES_PATH}",
                ): "version = 1\n# baseline\n",
                (
                    "show",
                    f"abc123:{lint.SCHEMA_PATH}",
                ): '{"oneOf": [{"before": true}]}',
            }

            payload = lint.build_lint_input(
                root,
                "base-ref",
                git=lambda _root, *args: outputs[args],
            )

        self.assertEqual(
            payload,
            {
                "before": {"oneOf": [{"before": True}]},
                "after": {"oneOf": [{"after": True}]},
                "beforeKnownBreakages": "version = 1\n# baseline\n",
                "afterKnownBreakages": "version = 1\n# current\n",
            },
        )

    def test_missing_baseline_log_uses_empty_log(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            write_current_files(root)
            outputs = {
                ("merge-base", "HEAD", "origin/main"): "abc123\n",
                (
                    "ls-tree",
                    "--name-only",
                    "abc123",
                    "--",
                    lint.KNOWN_BREAKAGES_PATH,
                ): "",
                (
                    "show",
                    f"abc123:{lint.SCHEMA_PATH}",
                ): '{"oneOf": []}',
            }

            payload = lint.build_lint_input(
                root,
                "origin/main",
                git=lambda _root, *args: outputs[args],
            )

        self.assertEqual(payload["beforeKnownBreakages"], lint.EMPTY_KNOWN_BREAKAGES)

    def test_base_ref_defaults_and_trims_override(self) -> None:
        self.assertEqual(lint.schema_base_ref({}), "origin/main")
        self.assertEqual(
            lint.schema_base_ref({"CODEX_SCHEMA_BASE_REF": "  feature/base  "}),
            "feature/base",
        )

    def test_bazel_exit_code_and_payload_are_preserved(self) -> None:
        payload = {"before": {}, "after": {}}
        with mock.patch.object(lint.subprocess, "run") as run:
            run.return_value = subprocess.CompletedProcess([], 23)

            returncode = lint.run_schema_evolution(Path("/repo"), payload)

        self.assertEqual(returncode, 23)
        run.assert_called_once_with(
            ["bazel", "run", lint.SCHEMA_EVOLUTION_TARGET],
            check=False,
            cwd=Path("/repo"),
            input=json.dumps(payload, separators=(",", ":")) + "\n",
            encoding="utf-8",
        )


def write_current_files(root: Path) -> None:
    schema = root / lint.SCHEMA_PATH
    schema.parent.mkdir(parents=True)
    schema.write_text('{"oneOf": [{"after": true}]}', encoding="utf-8")
    log = root / lint.KNOWN_BREAKAGES_PATH
    log.write_text("version = 1\n# current\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
