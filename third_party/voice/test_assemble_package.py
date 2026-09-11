"""Exercise private package copies, target pairing, provenance, and failure cleanup."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from assemble_package import assemble
from package_runtime import runtime_files
from release_runtime import seal
from release_runtime import stage
from runtime import PLUGINS, digest, required_library_paths


class AssembleTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="voice package ")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.package = self.root / "app"
        (self.package / "bin").mkdir(parents=True)
        (self.package / "codex-resources").mkdir()
        (self.package / "codex-path").mkdir()
        self.commit = "a" * 40
        self.metadata = {
            "layoutVersion": 1,
            "version": f"0.0.0+{self.commit}",
            "target": "aarch64-unknown-linux-musl",
            "variant": "codex",
            "entrypoint": "bin/codex",
            "resourcesDir": "codex-resources",
            "pathDir": "codex-path",
        }
        (self.package / "codex-package.json").write_text(json.dumps(self.metadata))
        (self.package / "bin/codex").write_bytes(b"unchanged app")
        self.helper = self.root / "helper.exe"
        self.helper.write_bytes(b"private helper")
        self.helper.chmod(0o755)
        self.output = self.root / "installed copy"

    def make_runtime(
        self, target="aarch64-unknown-linux-gnu", plugin="lib/gstreamer-1.0/libgst{}.so"
    ):
        root = self.root / target
        root.mkdir()
        libraries = []
        for name in PLUGINS:
            path = root / plugin.format(name)
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(f"prepared {name}".encode())
            libraries.append(
                {"path": path.relative_to(root).as_posix(), "sha256": digest(path)}
            )
        plugins = [record["path"] for record in libraries]
        for name in required_library_paths(target):
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"prepared required library")
            libraries.append({"path": name, "sha256": digest(path)})
        manifest = {
            "schemaVersion": 1,
            "developmentOnly": True,
            "target": target,
            "sourceCommit": "b" * 40,
            "sourceManifestSha256": digest(Path(__file__).with_name("sources.json")),
            "plugins": plugins,
            "libraries": libraries,
        }
        (root / "runtime.json").write_text(json.dumps(manifest))
        return root, manifest

    def test_cli_packages_each_runtime_layout_without_changing_bytes(self):
        for target, plugin in (
            ("aarch64-unknown-linux-gnu", "lib/gstreamer-1.0/libgst{}.so"),
            ("aarch64-apple-darwin", "plugins/libgst{}.dylib"),
            ("aarch64-pc-windows-msvc", "bin/gst{}.dll"),
        ):
            with self.subTest(target=target):
                runtime, receipt = self.make_runtime(target, plugin)
                (runtime / "unlisted-file").write_bytes(b"must not ship")
                windows = target.endswith("windows-msvc")
                entrypoint = "bin/codex.exe" if windows else "bin/codex"
                self.metadata.update(target=target, entrypoint=entrypoint)
                (self.package / entrypoint).write_bytes(b"unchanged app")
                (self.package / "codex-package.json").write_text(
                    json.dumps(self.metadata)
                )
                output = self.root / (target + " packaged")
                subprocess.run(
                    [
                        sys.executable,
                        str(Path(__file__).with_name("assemble_package.py").resolve()),
                        "--package",
                        str(self.package),
                        "--helper",
                        str(self.helper),
                        "--voice-target",
                        target,
                        "--build-commit",
                        self.commit,
                        "--output",
                        str(output),
                        "--runtime",
                        str(runtime),
                    ],
                    check=True,
                    capture_output=True,
                    cwd=self.root,
                    env={**os.environ, "PYTHONSAFEPATH": "1"},
                )
                voice = output / "codex-resources/voice"
                self.assertFalse((voice / "unlisted-file").exists())
                expected = {r["path"]: r["sha256"] for r in receipt["libraries"]}
                expected["runtime.json"] = digest(runtime / "runtime.json")
                self.assertEqual(
                    {name: digest(voice / name) for name in expected}, expected
                )
                self.assertEqual(
                    {name: digest(runtime / name) for name in expected}, expected
                )
                manifest = json.loads((voice / "manifest.json").read_text())
                helper_name = "codex-voice-host.exe" if windows else "codex-voice-host"
                self.assertEqual(
                    manifest["sha256"],
                    {
                        entrypoint: digest(self.package / entrypoint),
                        f"codex-resources/voice/bin/{helper_name}": digest(self.helper),
                        **{
                            f"codex-resources/voice/{name}": value
                            for name, value in expected.items()
                        },
                    },
                )

    def test_rejects_stale_runtime_without_gio_for_each_platform(self):
        for target, plugin, gio in (
            (
                "aarch64-unknown-linux-gnu",
                "lib/gstreamer-1.0/libgst{}.so",
                "lib/libgio-2.0.so.0",
            ),
            (
                "aarch64-apple-darwin",
                "plugins/libgst{}.dylib",
                "lib/libgio-2.0.0.dylib",
            ),
            ("aarch64-pc-windows-msvc", "bin/gst{}.dll", "bin/gio-2.0-0.dll"),
        ):
            with self.subTest(target=target):
                root, receipt = self.make_runtime(target, plugin)
                receipt["libraries"] = [
                    r for r in receipt["libraries"] if r["path"] != gio
                ]
                (root / gio).unlink()
                (root / "runtime.json").write_text(json.dumps(receipt))
                with self.assertRaisesRegex(ValueError, "required libraries"):
                    runtime_files(root.resolve(), target)

    def test_alpha_package_preserves_release_version_and_matching_build(self):
        self.commit = "b" * 40
        target = "aarch64-apple-darwin"
        runtime, _ = self.make_runtime(target, "plugins/libgst{}.dylib")
        staged = self.root / "staged"
        stage(runtime, staged, target)
        signed_library = staged / "lib/libgio-2.0.0.dylib"
        signed_library.write_bytes(signed_library.read_bytes() + b"signed")
        seal(staged, target)
        self.metadata["version"] = "0.154.0-alpha.8"
        self.metadata["target"] = target
        (self.package / "codex-package.json").write_text(json.dumps(self.metadata))
        assemble(
            self.package,
            self.helper,
            target,
            self.commit,
            self.output,
            runtime=staged,
            release_version="0.154.0-alpha.8",
        )
        manifest = json.loads(
            (self.output / "codex-resources/voice/manifest.json").read_text()
        )
        self.assertEqual(manifest["appVersion"], "0.154.0-alpha.8")
        self.assertEqual(manifest["buildCommit"], self.commit)
        notice_root = self.output / "codex-resources/voice"
        for source in (Path(__file__).with_name("licenses")).iterdir():
            relative = f"codex-resources/voice/licenses/{source.name}"
            self.assertEqual(
                (notice_root / "licenses" / source.name).read_bytes(),
                source.read_bytes(),
            )
            self.assertEqual(manifest["sha256"][relative], digest(source))
        self.assertEqual(
            (self.output / "codex-resources/voice/lib/libgio-2.0.0.dylib").read_bytes(),
            signed_library.read_bytes(),
        )

        self.output.rename(self.root / "previous output")
        with self.assertRaisesRegex(ValueError, "package version"):
            assemble(
                self.package,
                self.helper,
                target,
                self.commit,
                self.output,
                runtime=staged,
                release_version="0.154.0-alpha.7",
            )

        with self.assertRaisesRegex(ValueError, "runtime receipt"):
            assemble(
                self.package,
                self.helper,
                target,
                self.commit,
                self.output,
                runtime=runtime,
                release_version="0.154.0-alpha.8",
            )

    def test_alpha_receipt_requires_matching_signed_hashes(self):
        target = "x86_64-apple-darwin"
        runtime, _ = self.make_runtime(target, "plugins/libgst{}.dylib")
        secret = self.root / "outside-secret"
        secret.write_bytes(b"must not enter signing artifacts")
        (runtime / "unlisted-file").write_bytes(b"unlisted")
        try:
            (runtime / "unlisted-secret").symlink_to(secret)
        except OSError:
            # Windows runners may not grant symlink creation to this process.
            pass
        staged = self.root / "staged"
        stage(runtime, staged, target)
        self.assertFalse((staged / "unlisted-file").exists())
        self.assertFalse((staged / "unlisted-secret").exists())
        seal(staged, target)
        self.assertTrue(runtime_files(staged.resolve(), target, public_release=True))
        with self.assertRaisesRegex(ValueError, "runtime receipt"):
            runtime_files(staged.resolve(), target)
        (staged / "lib/libgio-2.0.0.dylib").write_bytes(b"tampered")
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            runtime_files(staged.resolve(), target, public_release=True)

    def test_beta_and_stable_release_versions_package_the_same_runtime(self):
        self.commit = "b" * 40
        target = "aarch64-apple-darwin"
        runtime, _ = self.make_runtime(target, "plugins/libgst{}.dylib")
        staged = self.root / "staged"
        stage(runtime, staged, target)
        seal(staged, target)
        self.metadata["target"] = target
        for version in ("0.154.0-beta.2", "0.154.0"):
            with self.subTest(version=version):
                self.metadata["version"] = version
                (self.package / "codex-package.json").write_text(
                    json.dumps(self.metadata)
                )
                output = self.root / f"package-{version}"
                assemble(
                    self.package,
                    self.helper,
                    target,
                    self.commit,
                    output,
                    runtime=staged,
                    release_version=version,
                )
                manifest = json.loads(
                    (output / "codex-resources/voice/manifest.json").read_text()
                )
                self.assertEqual(manifest["appVersion"], version)

    def test_linux_release_pairs_musl_app_with_gnu_voice_runtime(self):
        self.commit = "b" * 40
        target = "aarch64-unknown-linux-gnu"
        runtime, _ = self.make_runtime(target)
        staged = self.root / "staged"
        stage(runtime, staged, target)
        seal(staged, target)
        for version in ("0.154.0-alpha.8", "0.154.0-beta.2", "0.154.0"):
            with self.subTest(version=version):
                self.metadata["version"] = version
                (self.package / "codex-package.json").write_text(
                    json.dumps(self.metadata)
                )
                output = self.root / f"linux-{version}"
                assemble(
                    self.package,
                    self.helper,
                    target,
                    self.commit,
                    output,
                    runtime=staged,
                    release_version=version,
                )
                voice = output / "codex-resources/voice"
                manifest = json.loads((voice / "manifest.json").read_text())
                self.assertEqual(manifest["appTarget"], "aarch64-unknown-linux-musl")
                self.assertEqual(manifest["voiceTarget"], target)
                self.assertEqual(
                    manifest["sha256"]["codex-resources/voice/runtime.json"],
                    digest(staged / "runtime.json"),
                )

    def test_windows_release_packages_signed_receipt_and_exe_helper(self):
        self.commit = "b" * 40
        for target in ("x86_64-pc-windows-msvc", "aarch64-pc-windows-msvc"):
            with self.subTest(target=target):
                runtime, _ = self.make_runtime(target, "bin/gst{}.dll")
                staged = self.root / f"signed-{target}"
                stage(runtime, staged, target)
                library = staged / "bin/gio-2.0-0.dll"
                library.write_bytes(library.read_bytes() + b"signed")
                seal(staged, target)
                self.metadata.update(
                    target=target,
                    entrypoint="bin/codex.exe",
                    version="0.154.0-beta.2",
                )
                (self.package / "bin/codex.exe").write_bytes(b"unchanged app")
                (self.package / "codex-package.json").write_text(
                    json.dumps(self.metadata)
                )
                output = self.root / f"windows-{target}"
                assemble(
                    self.package,
                    self.helper,
                    target,
                    self.commit,
                    output,
                    runtime=staged,
                    release_version="0.154.0-beta.2",
                )
                voice = output / "codex-resources/voice"
                self.assertEqual(
                    (voice / "bin/codex-voice-host.exe").read_bytes(),
                    self.helper.read_bytes(),
                )
                self.assertEqual(
                    (voice / "bin/gio-2.0-0.dll").read_bytes(), library.read_bytes()
                )
                self.assertTrue(
                    runtime_files(voice.resolve(), target, public_release=True)
                )

    def test_rejects_invalid_runtime_receipts_before_creating_package(self):
        runtime, original = self.make_runtime()
        changes = [
            {"target": "x86_64-unknown-linux-gnu"},
            {"developmentOnly": False},
            {"sourceManifestSha256": "0" * 64},
            {"sourceCommit": "dev"},
            {"plugins": original["plugins"][:-1]},
            {"libraries": []},
            {"libraries": original["libraries"] * 20},
            {"libraries": original["libraries"] + [original["libraries"][0]]},
        ]
        for name in (
            "../outside.so",
            "lib/../outside.so",
            "bin/codex-voice-host",
            "lib/evil:stream.so",
        ):
            changes.append({"libraries": [{**original["libraries"][0], "path": name}]})
        for change in changes:
            with self.subTest(change=change), self.assertRaises(ValueError):
                (runtime / "runtime.json").write_text(
                    json.dumps({**original, **change})
                )
                assemble(
                    self.package,
                    self.helper,
                    original["target"],
                    self.commit,
                    self.output,
                    runtime=runtime,
                )
            self.assertFalse(self.output.exists())

    def test_rejects_runtime_digest_changes_and_nested_output(self):
        runtime, receipt = self.make_runtime()
        nested = runtime / "new package"
        with self.assertRaisesRegex(ValueError, "outside the runtime"):
            assemble(
                self.package,
                self.helper,
                receipt["target"],
                self.commit,
                nested,
                runtime=runtime,
            )
        self.assertFalse(nested.exists())
        (runtime / receipt["plugins"][0]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "digest mismatch"):
            assemble(
                self.package,
                self.helper,
                receipt["target"],
                self.commit,
                self.output,
                runtime=runtime,
            )
        self.assertFalse(self.output.exists())

    def test_rejects_runtime_inside_input_package(self):
        runtime, receipt = self.make_runtime()
        nested = self.package / "runtime-staging"
        runtime.rename(nested)
        (nested / "unlisted-file").write_bytes(b"must not ship")
        sources = [nested, self.package]
        alias = self.package.with_name("APP") / "runtime-staging"
        if alias.exists():
            sources.append(alias)
        for source in sources:
            with (
                self.subTest(source=source),
                self.assertRaisesRegex(ValueError, "outside the input package"),
            ):
                assemble(
                    self.package,
                    self.helper,
                    receipt["target"],
                    self.commit,
                    self.output,
                    runtime=source,
                )
            self.assertFalse(self.output.exists())

    def test_rejects_symlinked_runtime_library_directories(self):
        runtime, receipt = self.make_runtime()
        outside = self.root / "outside"
        (runtime / "lib/gstreamer-1.0").rename(outside)
        try:
            (runtime / "lib/gstreamer-1.0").symlink_to(
                outside, target_is_directory=True
            )
        except OSError as error:
            self.skipTest(f"symlink creation unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "regular files"):
            assemble(
                self.package,
                self.helper,
                receipt["target"],
                self.commit,
                self.output,
                runtime=runtime,
            )
        self.assertFalse(self.output.exists())

    def test_copy_revalidation_removes_only_new_output(self):
        runtime, receipt = self.make_runtime()
        original_copy = shutil.copy2
        for changed_name in (receipt["plugins"][0], "runtime.json"):

            def changed_copy(source, destination, **kwargs):
                result = original_copy(source, destination, **kwargs)
                if source == runtime.resolve() / changed_name:
                    Path(destination).write_bytes(b"changed after validation")
                return result

            with (
                self.subTest(changed_name=changed_name),
                patch("assemble_package.shutil.copy2", changed_copy),
            ):
                with self.assertRaisesRegex(ValueError, "changed during copying"):
                    assemble(
                        self.package,
                        self.helper,
                        receipt["target"],
                        self.commit,
                        self.output,
                        runtime=runtime,
                    )
            self.assertFalse(self.output.exists())
            self.assertEqual(
                json.loads((runtime / "runtime.json").read_text()), receipt
            )
            self.assertEqual(
                (self.package / "bin/codex").read_bytes(), b"unchanged app"
            )

    def test_copies_app_unchanged_and_records_distinct_linux_targets(self):
        runtime, receipt = self.make_runtime()
        assemble(
            self.package,
            self.helper,
            "aarch64-unknown-linux-gnu",
            self.commit,
            self.output,
            runtime=runtime,
        )
        self.assertEqual((self.output / "bin/codex").read_bytes(), b"unchanged app")
        self.assertEqual((self.package / "bin/codex").read_bytes(), b"unchanged app")
        self.assertFalse((self.package / "codex-resources/voice").exists())
        self.assertEqual(
            (self.output / "codex-package.json").read_bytes(),
            (self.package / "codex-package.json").read_bytes(),
        )
        self.assertEqual(
            json.loads(
                (self.output / "codex-resources/voice/manifest.json").read_text()
            ),
            {
                "schemaVersion": 1,
                "buildCommit": self.commit,
                "appTarget": self.metadata["target"],
                "voiceTarget": "aarch64-unknown-linux-gnu",
                "appVersion": self.metadata["version"],
                "sha256": {
                    "bin/codex": hashlib.sha256(b"unchanged app").hexdigest(),
                    "codex-resources/voice/bin/codex-voice-host": hashlib.sha256(
                        b"private helper"
                    ).hexdigest(),
                    **{
                        f"codex-resources/voice/{r['path']}": r["sha256"]
                        for r in receipt["libraries"]
                    },
                    "codex-resources/voice/runtime.json": digest(
                        runtime / "runtime.json"
                    ),
                },
            },
        )

    def test_rejects_incompatible_targets_and_unstamped_or_mixed_builds(self):
        runtime, _ = self.make_runtime()
        for target, commit in [
            ("aarch64-unknown-linux-musl", self.commit),
            ("x86_64-unknown-linux-gnu", self.commit),
            ("aarch64-unknown-linux-gnu", "dev"),
            ("aarch64-unknown-linux-gnu", "b" * 40),
        ]:
            with (
                self.subTest(target=target, commit=commit),
                self.assertRaises(ValueError),
            ):
                assemble(
                    self.package,
                    self.helper,
                    target,
                    commit,
                    self.output,
                    runtime=runtime,
                )
            self.assertFalse(self.output.exists())

    def test_assembles_matching_gnu_linux_app_and_helper_targets(self):
        for architecture in ("aarch64", "x86_64"):
            target = f"{architecture}-unknown-linux-gnu"
            with self.subTest(target=target):
                self.metadata["target"] = target
                (self.package / "codex-package.json").write_text(
                    json.dumps(self.metadata)
                )
                runtime, receipt = self.make_runtime(target)
                output = self.root / (target + " packaged")
                assemble(
                    self.package,
                    self.helper,
                    target,
                    self.commit,
                    output,
                    runtime=runtime,
                )
                manifest = json.loads(
                    (output / "codex-resources/voice/manifest.json").read_text()
                )
                self.assertEqual(
                    manifest,
                    {
                        "schemaVersion": 1,
                        "buildCommit": self.commit,
                        "appTarget": target,
                        "voiceTarget": target,
                        "appVersion": self.metadata["version"],
                        "sha256": {
                            "bin/codex": hashlib.sha256(b"unchanged app").hexdigest(),
                            "codex-resources/voice/bin/codex-voice-host": hashlib.sha256(
                                b"private helper"
                            ).hexdigest(),
                            **{
                                f"codex-resources/voice/{r['path']}": r["sha256"]
                                for r in receipt["libraries"]
                            },
                            "codex-resources/voice/runtime.json": digest(
                                runtime / "runtime.json"
                            ),
                        },
                    },
                )
                self.assertEqual((output / "bin/codex").read_bytes(), b"unchanged app")
                self.assertEqual(
                    (
                        output / "codex-resources/voice/bin/codex-voice-host"
                    ).read_bytes(),
                    self.helper.read_bytes(),
                )

    def test_never_replaces_existing_or_nested_outputs(self):
        runtime, _ = self.make_runtime()
        for output in (self.package, self.package / "nested", self.helper):
            with self.subTest(output=output), self.assertRaises(ValueError):
                assemble(
                    self.package,
                    self.helper,
                    "aarch64-unknown-linux-gnu",
                    self.commit,
                    output,
                    runtime=runtime,
                )
        self.assertEqual(self.helper.read_bytes(), b"private helper")
        self.assertFalse((self.package / "nested").exists())

    def test_cleans_only_its_new_copy_on_failure(self):
        runtime, _ = self.make_runtime()
        with patch("assemble_package.shutil.copy2", side_effect=OSError("copy failed")):
            with self.assertRaises(OSError):
                assemble(
                    self.package,
                    self.helper,
                    "aarch64-unknown-linux-gnu",
                    self.commit,
                    self.output,
                    runtime=runtime,
                )
        self.assertFalse(self.output.exists())
        self.assertEqual((self.package / "bin/codex").read_bytes(), b"unchanged app")


if __name__ == "__main__":
    unittest.main()
