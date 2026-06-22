#!/usr/bin/env python3

import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import patch


FORMAT_SCRIPT_PATH = Path(__file__).resolve().parent.parent / "format.py"
FORMAT_SCRIPT_SPEC = importlib.util.spec_from_file_location(
    "codex_format_script",
    FORMAT_SCRIPT_PATH,
)
if FORMAT_SCRIPT_SPEC is None or FORMAT_SCRIPT_SPEC.loader is None:
    raise RuntimeError(f"could not load {FORMAT_SCRIPT_PATH}")
format_script = importlib.util.module_from_spec(FORMAT_SCRIPT_SPEC)
sys.modules[FORMAT_SCRIPT_SPEC.name] = format_script
FORMAT_SCRIPT_SPEC.loader.exec_module(format_script)


class BuildifierFormatterGroupTest(unittest.TestCase):
    def test_includes_top_level_codex_rule_files(self) -> None:
        repository_files = b"\0".join(
            [
                b".codex/rules/z.rules",
                b".codex/rules/a.rules",
                b".codex/rules/nested/ignored.rules",
                b"elsewhere.rules",
            ]
        )

        with patch.object(
            format_script.subprocess,
            "check_output",
            return_value=repository_files,
        ):
            group = format_script.buildifier_formatter_group(check=True)

        self.assertEqual(
            group.commands[0].args[4:],
            (".codex/rules/a.rules", ".codex/rules/z.rules"),
        )


if __name__ == "__main__":
    unittest.main()
