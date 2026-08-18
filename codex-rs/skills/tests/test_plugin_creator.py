"""Local-only regression tests for the bundled plugin-creator scripts.

Run with:
    python3 -B -m unittest discover -s codex-rs/skills/tests -p 'test_*.py'
"""

import json
import runpy
import subprocess
import sys
import tempfile
import types
import unittest
from pathlib import Path
from unittest.mock import patch


SCRIPTS = (
    Path(__file__).resolve().parents[1]
    / "src"
    / "assets"
    / "samples"
    / "plugin-creator"
    / "scripts"
)


class PluginCreatorSecurityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.marketplace_path = self.root / "marketplace.json"
        self.plugin_root = self.root / "plugins" / "demo"

    def run_script(self, script: str, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, "-B", str(SCRIPTS / script), *arguments],
            capture_output=True,
            text=True,
            check=False,
        )

    def write_marketplace(self, name: object) -> None:
        self.marketplace_path.write_text(
            json.dumps({"name": name, "plugins": []}),
            encoding="utf-8",
        )

    def write_plugin(self, name: object) -> Path:
        manifest_path = self.plugin_root / ".codex-plugin" / "plugin.json"
        manifest_path.parent.mkdir(parents=True, exist_ok=True)
        manifest_path.write_text(
            json.dumps({"name": name, "version": "1.0.0"}),
            encoding="utf-8",
        )
        return manifest_path

    def plugin_validation_errors(self, name: str) -> list[str]:
        with (
            patch.dict(sys.modules, {"yaml": types.ModuleType("yaml")}),
            patch.object(sys, "path", [str(SCRIPTS), *sys.path]),
            patch.object(sys, "dont_write_bytecode", True),
        ):
            scaffold = runpy.run_path(str(SCRIPTS / "create_basic_plugin.py"))
            validator = runpy.run_path(str(SCRIPTS / "validate_plugin.py"))

        manifest = scaffold["build_plugin_json"]("demo", with_mcp=False, with_apps=False)
        manifest["name"] = name
        errors: list[str] = []
        validator["validate_manifest_shape"](self.plugin_root, manifest, errors)
        return errors

    def test_marketplace_reader_accepts_valid_names(self) -> None:
        for name in ("personal", "team-local", "team_local_123", "ABC123_-", "_", "-"):
            with self.subTest(name=name):
                self.write_marketplace(name)
                result = self.run_script(
                    "read_marketplace_name.py",
                    "--marketplace-path",
                    str(self.marketplace_path),
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(result.stdout, f"{name}\n")

    def test_marketplace_reader_rejects_unsafe_names_without_stdout(self) -> None:
        unsafe_names = (
            "team;id",
            "team\nlocal",
            "team local",
            " team",
            "team ",
            "team'local",
            'team"local',
            "team$(id)",
            "team`id`",
            "team&local",
            "team|local",
            "team<local",
            "team>local",
            "team.local",
            "équipe",
            "",
        )
        for name in unsafe_names:
            with self.subTest(name=name):
                self.write_marketplace(name)
                result = self.run_script(
                    "read_marketplace_name.py",
                    "--marketplace-path",
                    str(self.marketplace_path),
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")

    def test_marketplace_reader_rejects_missing_and_nonstring_names(self) -> None:
        for payload in ({}, {"name": None}, {"name": 123}, {"name": ["team"]}, []):
            with self.subTest(payload=payload):
                self.marketplace_path.write_text(json.dumps(payload), encoding="utf-8")
                result = self.run_script(
                    "read_marketplace_name.py",
                    "--marketplace-path",
                    str(self.marketplace_path),
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")

    def test_marketplace_reader_rejects_malformed_json(self) -> None:
        self.marketplace_path.write_text("{", encoding="utf-8")
        result = self.run_script(
            "read_marketplace_name.py",
            "--marketplace-path",
            str(self.marketplace_path),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "")

    def test_plugin_validator_accepts_canonical_names(self) -> None:
        for name in ("demo", "Team_Name-1", "team.tools", "team.tools_v2"):
            with self.subTest(name=name):
                self.assertEqual(self.plugin_validation_errors(name), [])

    def test_plugin_validator_rejects_unsafe_names(self) -> None:
        unsafe_names = (
            "safe;id",
            "$(id)",
            "safe\nid",
            "safe name",
            "safe`id`",
            ".hidden",
            "trailing.",
            "safe..name",
            "équipe",
        )
        for name in unsafe_names:
            with self.subTest(name=name):
                errors = self.plugin_validation_errors(name)
                self.assertTrue(errors)
                self.assertTrue(any("name" in error for error in errors))

    def test_cachebuster_rejects_unsafe_plugin_names_without_writing(self) -> None:
        for name in ("safe;id", "$(id)", "safe\nid", ".hidden", "safe..name"):
            with self.subTest(name=name):
                manifest_path = self.write_plugin(name)
                original = manifest_path.read_bytes()
                result = self.run_script("update_plugin_cachebuster.py", str(self.plugin_root))
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
                self.assertEqual(manifest_path.read_bytes(), original)

    def test_cachebuster_accepts_dotted_plugin_names(self) -> None:
        manifest_path = self.write_plugin("demo.tools")
        result = self.run_script(
            "update_plugin_cachebuster.py",
            str(self.plugin_root),
            "--cachebuster",
            "safe-token",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        self.assertEqual(manifest["name"], "demo.tools")
        self.assertEqual(manifest["version"], "1.0.0+codex.safe-token")

    def test_scaffold_rejects_invalid_marketplace_before_creating_files(self) -> None:
        self.write_marketplace("team;id")
        original = self.marketplace_path.read_bytes()
        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.plugin_root.exists())
        self.assertEqual(self.marketplace_path.read_bytes(), original)

    def test_scaffold_force_preserves_files_when_marketplace_is_invalid(self) -> None:
        self.write_marketplace("team;id")
        plugin_manifest = self.write_plugin("demo")
        mcp_manifest = self.plugin_root / ".mcp.json"
        app_manifest = self.plugin_root / ".app.json"
        mcp_manifest.write_text('{"mcpServers":{"existing":{}}}', encoding="utf-8")
        app_manifest.write_text('{"apps":{"existing":{}}}', encoding="utf-8")
        originals = {
            path: path.read_bytes()
            for path in (self.marketplace_path, plugin_manifest, mcp_manifest, app_manifest)
        }

        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
            "--with-mcp",
            "--with-apps",
            "--with-skills",
            "--force",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.plugin_root / "skills").exists())
        for path, original in originals.items():
            with self.subTest(path=path):
                self.assertEqual(path.read_bytes(), original)

    def test_scaffold_force_preserves_files_when_plugins_field_is_invalid(self) -> None:
        self.marketplace_path.write_text(
            json.dumps({"name": "team-local", "plugins": {}}),
            encoding="utf-8",
        )
        plugin_manifest = self.write_plugin("demo")
        mcp_manifest = self.plugin_root / ".mcp.json"
        app_manifest = self.plugin_root / ".app.json"
        mcp_manifest.write_text('{"mcpServers":{"existing":{}}}', encoding="utf-8")
        app_manifest.write_text('{"apps":{"existing":{}}}', encoding="utf-8")
        originals = {
            path: path.read_bytes()
            for path in (self.marketplace_path, plugin_manifest, mcp_manifest, app_manifest)
        }

        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
            "--with-mcp",
            "--with-apps",
            "--with-skills",
            "--force",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.plugin_root / "skills").exists())
        for path, original in originals.items():
            with self.subTest(path=path):
                self.assertEqual(path.read_bytes(), original)

    def test_scaffold_rejects_duplicate_marketplace_entry_before_creating_files(self) -> None:
        self.marketplace_path.write_text(
            json.dumps({"name": "team-local", "plugins": [{"name": "demo"}]}),
            encoding="utf-8",
        )
        original = self.marketplace_path.read_bytes()
        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(self.plugin_root.exists())
        self.assertEqual(self.marketplace_path.read_bytes(), original)

    def test_scaffold_accepts_existing_valid_marketplace(self) -> None:
        self.write_marketplace("team-local_123")
        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        marketplace = json.loads(self.marketplace_path.read_text(encoding="utf-8"))
        self.assertEqual(marketplace["name"], "team-local_123")
        self.assertEqual(marketplace["plugins"][0]["name"], "demo")

    def test_scaffold_creates_missing_personal_marketplace(self) -> None:
        self.assertFalse(self.marketplace_path.exists())
        result = self.run_script(
            "create_basic_plugin.py",
            "demo",
            "--path",
            str(self.root / "plugins"),
            "--with-marketplace",
            "--marketplace-path",
            str(self.marketplace_path),
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        marketplace = json.loads(self.marketplace_path.read_text(encoding="utf-8"))
        self.assertEqual(marketplace["name"], "personal")
        self.assertEqual(marketplace["plugins"][0]["name"], "demo")


if __name__ == "__main__":
    unittest.main()
