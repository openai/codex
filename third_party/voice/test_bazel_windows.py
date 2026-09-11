"""Exercise declared tool selection and the platform-independent payload copy action."""

import copy
import errno
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import bazel_windows

from bazel_copy import copy_payloads
from bazel_windows import selected_inputs


class WindowsInputsTests(unittest.TestCase):
    def test_adapter_preserves_host_architecture_without_developer_path(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "action.json"
            config.write_text("{}")
            for architecture in ("AMD64", "ARM64"):
                environment = {"PROCESSOR_ARCHITECTURE": architecture, "PATH": "unsafe"}
                with (
                    self.subTest(architecture=architecture),
                    patch.object(
                        bazel_windows, "os", SimpleNamespace(environ=environment)
                    ),
                    patch.object(
                        bazel_windows.sys, "argv", ["driver", "unknown", str(config)]
                    ),
                    patch.object(
                        bazel_windows,
                        "selected_inputs",
                        return_value={"target": "unused"},
                    ),
                    patch.object(
                        bazel_windows,
                        "build_environment",
                        return_value=({"PATH": "declared"}, {}),
                    ),
                ):
                    with self.assertRaisesRegex(
                        ValueError, "unknown Windows native action"
                    ):
                        bazel_windows.main()
                    self.assertEqual(
                        environment,
                        {
                            "PROCESSOR_ARCHITECTURE": architecture,
                            "PATH": "declared",
                            "HOME": environment["HOME"],
                            "USERPROFILE": environment["HOME"],
                        },
                    )

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repository = self.root / "external/installed tools"
        self.names = {
            "shell": "cygwin/bin/bash.exe",
            "make": "cygwin/bin/make.exe",
            "cygpath": "cygwin/bin/cygpath.exe",
            "automake": "cygwin/bin/automake-1.18",
            "pkg_config": "pkgconf-image/PFiles64/pkgconf 3.0.6/pkgconf.exe",
        }
        for name in self.names.values():
            path = self.repository / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"synthetic declared executable")
        self.manifest = self.repository / "voice-tools.json"
        self.metadata = {
            "schemaVersion": 1,
            "target": "aarch64-pc-windows-msvc",
            "cygwinArchitecture": "x86_64",
            "tools": self.names,
        }
        self.manifest.write_text(json.dumps(self.metadata))
        self.config = {
            "inputs": {
                "schemaVersion": 1,
                "target": "aarch64-pc-windows-msvc",
                "systemRoot": "C:/Windows",
                "tools": {
                    "cc": "external/msvc/cl.exe",
                    "pkg_config": str(
                        (self.repository / self.names["pkg_config"]).relative_to(
                            self.root
                        )
                    ),
                },
                "INCLUDE": ["external/sdk/include"],
                "LIB": ["external/sdk/lib"],
            },
            "manifest": str(self.manifest.relative_to(self.root)),
            "installed_files": [
                str((self.repository / name).relative_to(self.root))
                for name in self.names.values()
            ],
        }

    def test_paths_are_anchored_before_recipe_changes_directory(self):
        before = copy.deepcopy(self.config)
        expected = {
            **self.config["inputs"],
            "tools": {
                "cc": str(self.root / "external/msvc/cl.exe"),
                **{
                    name: str(self.repository / path)
                    for name, path in self.names.items()
                },
            },
            "INCLUDE": [str(self.root / "external/sdk/include")],
            "LIB": [str(self.root / "external/sdk/lib")],
        }
        self.assertEqual(selected_inputs(self.config, self.root), expected)
        self.assertEqual(self.config, before)

    def test_manifest_cannot_select_a_file_omitted_from_action_inputs(self):
        self.config["installed_files"].pop()
        with self.assertRaisesRegex(ValueError, "selection differs: pkg_config"):
            selected_inputs(self.config, self.root)

    def test_manifest_cannot_escape_installed_tree(self):
        outside = self.repository.parent / "outside.exe"
        outside.write_bytes(b"not part of the installed support tree")
        inside = self.repository / "cygwin/bin/other-bash.exe"
        try:
            inside.symlink_to(outside)
        except OSError as error:
            if error.errno not in (errno.EPERM, errno.EACCES):
                raise
            self.skipTest("creating symlinks requires OS permission")
        self.config["installed_files"].append(str(inside.relative_to(self.root)))
        self.metadata["tools"]["shell"] = str(inside.relative_to(self.repository))
        self.manifest.write_text(json.dumps(self.metadata))
        with self.assertRaisesRegex(ValueError, "selection differs: shell"):
            selected_inputs(self.config, self.root)

    def test_manifest_cannot_replace_the_build_script_executable(self):
        alternative = self.repository / "pkgconf-image/another/pkgconf.exe"
        alternative.parent.mkdir(parents=True)
        alternative.write_bytes(b"different declared executable")
        self.config["installed_files"].append(str(alternative.relative_to(self.root)))
        self.metadata["tools"]["pkg_config"] = str(
            alternative.relative_to(self.repository)
        )
        self.manifest.write_text(json.dumps(self.metadata))
        with self.assertRaisesRegex(ValueError, "selection differs: pkg_config"):
            selected_inputs(self.config, self.root)

    def test_wrong_target_or_emulation_contract_is_rejected(self):
        for key, value in (
            ("target", "x86_64-pc-windows-msvc"),
            ("cygwinArchitecture", "aarch64"),
        ):
            with self.subTest(key=key):
                self.manifest.write_text(json.dumps({**self.metadata, key: value}))
                with self.assertRaisesRegex(ValueError, "selected target"):
                    selected_inputs(self.config, self.root)

    def test_missing_declared_executable_does_not_fall_back_to_path(self):
        (self.repository / self.names["shell"]).unlink()
        with self.assertRaises(FileNotFoundError):
            selected_inputs(self.config, self.root)


class LibraryCopiesTests(unittest.TestCase):
    def test_import_library_and_dll_bytes_remain_distinct(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            dll = root / "source.dll"
            library = root / "source.lib"
            dll.write_bytes(b"DLL payload")
            library.write_bytes(b"import library payload")
            locator = root / "package/lib/search-path"
            copy_payloads(
                locator,
                [
                    dll,
                    root / "package/bin/audio.dll",
                    library,
                    root / "package/lib/audio.lib",
                ],
            )
            self.assertTrue(locator.is_dir())
            self.assertEqual(
                {
                    path.relative_to(root / "package").as_posix(): path.read_bytes()
                    for path in (root / "package").rglob("*")
                    if path.is_file()
                },
                {
                    "bin/audio.dll": b"DLL payload",
                    "lib/audio.lib": b"import library payload",
                },
            )

    def test_missing_payload_fails_the_action(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(FileNotFoundError):
                copy_payloads(
                    root / "lib/search-path",
                    [root / "missing.dll", root / "bin/audio.dll"],
                )


if __name__ == "__main__":
    unittest.main()
