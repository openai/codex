#!/usr/bin/env python3

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


REPO_ROOT = Path(__file__).resolve().parents[2]
CODEX_CLI_ROOT = REPO_ROOT / "codex-cli"
NODE = shutil.which("node")

TARGET_BY_PLATFORM_ARCH = {
    "darwin/arm64": "aarch64-apple-darwin",
    "darwin/x64": "x86_64-apple-darwin",
    "linux/arm64": "aarch64-unknown-linux-musl",
    "linux/x64": "x86_64-unknown-linux-musl",
}


@unittest.skipIf(os.name == "nt", "launcher fixtures require Unix symlinks")
@unittest.skipIf(NODE is None, "node is required")
class NpmLauncherPackageManagerTest(unittest.TestCase):
    def setUp(self) -> None:
        assert NODE is not None
        self.node = Path(NODE).resolve()
        platform_arch = subprocess.check_output(
            [str(self.node), "-p", "`${process.platform}/${process.arch}`"],
            text=True,
        ).strip()
        if platform_arch not in TARGET_BY_PLATFORM_ARCH:
            self.skipTest(f"unsupported Node platform: {platform_arch}")
        self.target = TARGET_BY_PLATFORM_ARCH[platform_arch]

        self.temp_dir = tempfile.TemporaryDirectory(prefix="codex-npm-launcher-")
        self.addCleanup(self.temp_dir.cleanup)
        self.root = Path(self.temp_dir.name)
        self.probe = self.root / "probe.mjs"
        self.probe.write_text(
            """
console.log(JSON.stringify({
  npm: process.env.CODEX_MANAGED_BY_NPM ?? null,
  bun: process.env.CODEX_MANAGED_BY_BUN ?? null,
  pnpm: process.env.CODEX_MANAGED_BY_PNPM ?? null,
  packageRoot: process.env.CODEX_MANAGED_PACKAGE_ROOT ?? null,
}));
""".strip()
            + "\n",
            encoding="utf-8",
        )

    def test_detects_pnpm_virtual_store_global_install(self) -> None:
        layout_dir = self.root / "custom-global" / "5"
        package_root = (
            layout_dir
            / ".pnpm"
            / "@openai+codex@1.0.0"
            / "node_modules"
            / "@openai"
            / "codex"
        )
        entrypoint = self.create_codex_package(package_root)

        node_modules = layout_dir / "node_modules"
        self.create_pnpm_marker(node_modules)
        self.link_directory(package_root, node_modules / "@openai" / "codex")
        global_bin = self.root / "pnpm-home" / "codex"
        self.link_file(entrypoint, global_bin)

        self.assert_manager(global_bin, package_root, "pnpm")

    def test_detects_hoisted_pnpm_global_install(self) -> None:
        node_modules = self.root / "custom-global" / "5" / "node_modules"
        package_root = node_modules / "@openai" / "codex"
        entrypoint = self.create_codex_package(package_root)
        self.create_pnpm_marker(node_modules)
        global_bin = self.root / "pnpm-home" / "codex"
        self.link_file(entrypoint, global_bin)

        self.assert_manager(global_bin, package_root, "pnpm")

    def test_detects_pnpm_v11_linked_install_group(self) -> None:
        package_root = (
            self.root
            / "pnpm-home"
            / "store"
            / "v10"
            / "links"
            / "@openai+codex"
            / "node_modules"
            / "@openai"
            / "codex"
        )
        self.create_codex_package(package_root)

        node_modules = (
            self.root / "pnpm-home" / "global" / "v11" / "123-456" / "node_modules"
        )
        self.create_pnpm_marker(node_modules)
        public_package = node_modules / "@openai" / "codex"
        self.link_directory(package_root, public_package)

        self.assert_manager(public_package / "bin" / "codex.js", package_root, "pnpm")

    def test_rejects_unrelated_ancestor_pnpm_metadata(self) -> None:
        home = self.root / "home"
        package_root = (
            home / ".npm-global" / "lib" / "node_modules" / "@openai" / "codex"
        )
        entrypoint = self.create_codex_package(package_root)
        npm_bin = home / ".npm-global" / "bin" / "codex"
        self.link_file(entrypoint, npm_bin)

        unrelated_node_modules = home / "node_modules"
        self.create_pnpm_marker(unrelated_node_modules)
        (unrelated_node_modules / "@openai" / "codex").mkdir(parents=True)

        self.assert_manager(npm_bin, package_root, "npm")

    def create_codex_package(self, package_root: Path) -> Path:
        bin_dir = package_root / "bin"
        bin_dir.mkdir(parents=True)
        entrypoint = bin_dir / "codex.js"
        shutil.copy2(CODEX_CLI_ROOT / "bin" / "codex.js", entrypoint)
        shutil.copy2(CODEX_CLI_ROOT / "package.json", package_root / "package.json")

        vendor_bin = package_root / "vendor" / self.target / "bin" / "codex"
        vendor_bin.parent.mkdir(parents=True)
        vendor_bin.symlink_to(self.node)
        return entrypoint

    def create_pnpm_marker(self, node_modules: Path) -> None:
        node_modules.mkdir(parents=True, exist_ok=True)
        (node_modules / ".modules.yaml").write_text(
            "packageManager: pnpm@10.0.0\n",
            encoding="utf-8",
        )

    def link_directory(self, target: Path, link: Path) -> None:
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(target, target_is_directory=True)

    def link_file(self, target: Path, link: Path) -> None:
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(target)

    def assert_manager(
        self,
        entrypoint: Path,
        package_root: Path,
        expected_manager: str,
    ) -> None:
        result = subprocess.run(
            [str(self.node), str(entrypoint), str(self.probe)],
            cwd=self.root,
            env={"PATH": os.environ.get("PATH", "")},
            text=True,
            capture_output=True,
            timeout=10,
        )
        self.assertEqual(
            result.returncode,
            0,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        actual = json.loads(result.stdout)
        expected = {
            "npm": "1" if expected_manager == "npm" else None,
            "bun": "1" if expected_manager == "bun" else None,
            "pnpm": "1" if expected_manager == "pnpm" else None,
            "packageRoot": str(package_root.resolve()),
        }
        self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
