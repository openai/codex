#!/usr/bin/env python3

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


BUILD_SCRIPT = Path(__file__).resolve().parent / "build_npm_package.py"
SPEC = importlib.util.spec_from_file_location("codex_build_npm_package", BUILD_SCRIPT)
if SPEC is None or SPEC.loader is None:
    raise RuntimeError(f"Unable to load module from {BUILD_SCRIPT}")

build_npm_package = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(build_npm_package)


class StageSourcesTest(unittest.TestCase):
    def test_codex_meta_package_omits_ripgrep_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            staging_dir = Path(temp_dir)

            build_npm_package.stage_sources(staging_dir, "0.1.0", "codex")

            with open(staging_dir / "package.json", encoding="utf-8") as fh:
                package_json = json.load(fh)

            self.assertTrue((staging_dir / "bin" / "codex.js").is_file())
            self.assertFalse((staging_dir / "bin" / "rg").exists())
            self.assertEqual(package_json["files"], ["bin/codex.js"])


if __name__ == "__main__":
    unittest.main()
