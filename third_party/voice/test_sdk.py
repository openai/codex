"""Exercise exported bytes, development aliases and moved pkg-config inputs."""

import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from runtime import digest
from sdk import MODULES, export_sdk


class SdkTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="voice SDK ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.prefix = self.root / "prefix"
        self.receipts = self.root / "receipts"
        self.output = self.root / "sdk"
        self.target = "aarch64-apple-darwin"
        for name in ("include/glib-2.0", "lib/glib-2.0/include", "lib/pkgconfig"):
            (self.prefix / name).mkdir(parents=True)
        (self.prefix / "include/glib-2.0/glib.h").write_text("/* SDK fixture */")
        (self.prefix / "lib/glib-2.0/include/glibconfig.h").write_text("/* target */")
        self.library = self.prefix / "lib/libfixture.0.dylib"
        self.library.write_bytes(b"receipt-verified fixture bytes")
        for module in (*MODULES, "zlib"):
            (self.prefix / f"lib/pkgconfig/{module}.pc").write_text(
                "prefix=${pcfiledir}/../..\n"
                f"Name: {module}\nDescription: SDK fixture\nVersion: 1.0\n"
                "Cflags: -I${prefix}/include\nLibs: -L${prefix}/lib -lfixture\n"
            )
        audio = self.prefix / "lib/pkgconfig/gstreamer-audio-1.0.pc"
        audio.write_text(
            audio.read_text() + "Requires.private: gstreamer-tag-1.0, zlib\n"
        )
        (self.receipts / "inspection").mkdir(parents=True)
        self.ci = {
            "target": self.target,
            "build_complete": True,
            "inspection_complete": True,
            "manifest_sha256": digest(Path(__file__).with_name("sources.json")),
            "commit": "a" * 40,
        }
        (self.receipts / "ci.json").write_text(json.dumps(self.ci))
        (self.receipts / "inspection/binaries.json").write_text(
            json.dumps(
                [
                    {
                        "path": "lib/libfixture.0.dylib",
                        "target": self.target,
                        "sha256": digest(self.library),
                    }
                ]
            )
        )

    def test_exported_bytes_and_receipt_survive_move_without_original(self):
        export_sdk(self.prefix, self.receipts, self.target, self.output)
        moved = self.root / "moved SDK"
        self.output.rename(moved)
        shutil.rmtree(self.prefix)
        manifest = json.loads((moved / "sdk.json").read_text())
        actual = {
            path.relative_to(moved).as_posix(): digest(path)
            for path in moved.rglob("*")
            if path.is_file() and path.name != "sdk.json"
        }
        self.assertEqual(
            manifest,
            {
                "schemaVersion": 1,
                "target": self.target,
                "sourceCommit": "a" * 40,
                "sourceManifestSha256": self.ci["manifest_sha256"],
                "files": [
                    {"path": name, "sha256": actual[name]} for name in sorted(actual)
                ],
            },
        )
        pkg_config = os.environ.get("VOICE_PKG_CONFIG") or shutil.which("pkg-config")
        if not pkg_config:
            self.skipTest("pkg-config required for the moved development-input probe")
        result = subprocess.check_output(
            [
                pkg_config,
                "--define-prefix",
                "--cflags",
                "--libs",
                "gstreamer-audio-1.0",
            ],
            env={
                **os.environ,
                "PKG_CONFIG_PATH": "",
                "PKG_CONFIG_LIBDIR": str(moved / "lib/pkgconfig"),
            },
            text=True,
        ).strip()
        self.assertEqual(
            shlex.split(result),
            [f"-I{moved.as_posix()}/include", f"-L{moved.as_posix()}/lib", "-lfixture"],
        )

    def test_native_mutation_and_wrong_target_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "sources and target"):
            export_sdk(self.prefix, self.receipts, "x86_64-apple-darwin", self.output)
        self.library.write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "does not match"):
            export_sdk(self.prefix, self.receipts, self.target, self.output)
        self.assertFalse(self.output.exists())

    def test_development_alias_is_materialized_and_escape_is_rejected(self):
        alias = self.prefix / "lib/libfixture.dylib"
        try:
            alias.symlink_to(self.library.name)
        except OSError:
            self.skipTest("host cannot create the SDK's development symlink")
        export_sdk(self.prefix, self.receipts, self.target, self.output)
        self.assertFalse((self.output / "lib/libfixture.dylib").is_symlink())
        self.assertEqual(
            (self.output / "lib/libfixture.dylib").read_bytes(),
            self.library.read_bytes(),
        )
        shutil.rmtree(self.output)
        alias.unlink()
        alias.symlink_to(self.receipts / "ci.json")
        with self.assertRaisesRegex(ValueError, "inside the prefix"):
            export_sdk(self.prefix, self.receipts, self.target, self.output)

    def test_nonrelocatable_metadata_and_failed_copy_leave_no_output(self):
        metadata = self.prefix / "lib/pkgconfig/gstreamer-1.0.pc"
        original = metadata.read_text()
        metadata.write_text(original.replace("${pcfiledir}/../..", "/old/build"))
        with self.assertRaisesRegex(ValueError, "rebuild the SDK"):
            export_sdk(self.prefix, self.receipts, self.target, self.output)
        metadata.write_text(original)
        with patch("sdk.shutil.copy2", side_effect=OSError("copy failed")):
            with self.assertRaises(OSError):
                export_sdk(self.prefix, self.receipts, self.target, self.output)
        self.assertFalse(self.output.exists())

    def test_output_cannot_overwrite_or_nest_inside_inputs(self):
        for output in (self.prefix, self.prefix / "sdk", self.receipts / "sdk"):
            with (
                self.subTest(output=output),
                self.assertRaisesRegex(ValueError, "fresh"),
            ):
                export_sdk(self.prefix, self.receipts, self.target, output)
