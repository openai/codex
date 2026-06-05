#!/usr/bin/env python3

import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.layout import build_package_dir
from codex_package.layout import validate_package_dir
from codex_package.targets import PACKAGE_VARIANTS
from codex_package.targets import PackageInputs
from codex_package.targets import TARGET_SPECS


class PackageLayoutTest(unittest.TestCase):
    def test_code_mode_host_is_packaged_and_required(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            package_dir = root / "package"
            package_dir.mkdir()
            entrypoint = touch_file(root / "codex")
            code_mode_host = touch_file(root / "codex-code-mode-host")
            rg = touch_file(root / "rg")
            spec = TARGET_SPECS["aarch64-apple-darwin"]
            variant = PACKAGE_VARIANTS["codex"]

            build_package_dir(
                package_dir,
                "0.0.0-test",
                variant,
                spec,
                PackageInputs(
                    entrypoint_bin=entrypoint,
                    code_mode_host_bin=code_mode_host,
                    rg_bin=rg,
                    zsh_bin=None,
                    bwrap_bin=None,
                    codex_command_runner_bin=None,
                    codex_windows_sandbox_setup_bin=None,
                ),
            )

            packaged_host = package_dir / "bin" / "codex-code-mode-host"
            self.assertTrue(packaged_host.is_file())
            self.assertTrue(packaged_host.stat().st_mode & 0o100)
            metadata = json.loads(
                (package_dir / "codex-package.json").read_text(encoding="utf-8")
            )
            self.assertEqual(metadata["codeModeHost"], "bin/codex-code-mode-host")
            validate_package_dir(
                package_dir,
                variant,
                spec,
                include_zsh=False,
            )

            packaged_host.unlink()
            with self.assertRaisesRegex(
                RuntimeError,
                "Missing package file: bin/codex-code-mode-host",
            ):
                validate_package_dir(
                    package_dir,
                    variant,
                    spec,
                    include_zsh=False,
                )


def touch_file(path: Path) -> Path:
    path.touch()
    return path


if __name__ == "__main__":
    unittest.main()
