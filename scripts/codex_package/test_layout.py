#!/usr/bin/env python3

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
    def test_app_server_package_places_code_mode_host_beside_entrypoint(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            package_dir = root / "package"
            package_dir.mkdir()
            rg_shim = touch_executable(root / "codex-rg")
            rg_shim.write_text("shim", encoding="utf-8")
            rg = touch_executable(root / "rg")
            rg.write_text("ripgrep", encoding="utf-8")
            inputs = PackageInputs(
                entrypoint_bin=touch_executable(root / "codex-app-server"),
                code_mode_host_bin=touch_executable(root / "codex-code-mode-host"),
                rg_shim_bin=rg_shim,
                rg_bin=rg,
                zsh_bin=None,
                bwrap_bin=touch_executable(root / "bwrap"),
                codex_command_runner_bin=None,
                codex_windows_sandbox_setup_bin=None,
            )

            build_package_dir(
                package_dir,
                "1.2.3",
                PACKAGE_VARIANTS["codex-app-server"],
                TARGET_SPECS["x86_64-unknown-linux-musl"],
                inputs,
            )
            validate_package_dir(
                package_dir,
                PACKAGE_VARIANTS["codex-app-server"],
                TARGET_SPECS["x86_64-unknown-linux-musl"],
                include_zsh=False,
            )

            self.assertTrue((package_dir / "bin" / "codex-code-mode-host").is_file())
            self.assertEqual(
                (package_dir / "codex-path" / "rg").read_text(encoding="utf-8"),
                "shim",
            )
            self.assertEqual(
                (package_dir / "codex-resources" / "rg").read_text(encoding="utf-8"),
                "ripgrep",
            )


def touch_executable(path: Path) -> Path:
    path.touch(mode=0o755)
    return path


if __name__ == "__main__":
    unittest.main()
