import hashlib
import io
import sys
import tempfile
import unittest
from collections.abc import Iterator
from contextlib import contextmanager
from pathlib import Path
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package import v8
from codex_package.targets import TARGET_SPECS, TargetSpec


class FetchCodexV8ArtifactsTest(unittest.TestCase):
    version = "150.4.0"

    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    @contextmanager
    def release(
        self,
        target: str,
        *,
        line_ending: bytes = b"\n",
        trusted_digest: str | None = None,
        trusted_name: str | None = None,
        create_pins: bool = True,
    ) -> Iterator[tuple[TargetSpec, MagicMock, str]]:
        spec = TARGET_SPECS[target]
        profile = v8.V8_ARTIFACT_PROFILE
        archive_name = (
            f"rusty_v8_{profile}_{target}.lib.gz"
            if spec.is_windows
            else f"librusty_v8_{profile}_{target}.a.gz"
        )
        binding_name = f"src_binding_{profile}_{target}.rs"
        manifest_name = f"rusty_v8_{profile}_{target}.sha256"
        archive = b"trusted V8 archive"
        binding = b"trusted V8 binding"
        manifest = (
            line_ending.join(
                (
                    f"{hashlib.sha256(archive).hexdigest()}  {archive_name}".encode(),
                    f"{hashlib.sha256(binding).hexdigest()}  {binding_name}".encode(),
                )
            )
            + line_ending
        )
        payloads = {
            manifest_name: manifest,
            archive_name: archive,
            binding_name: binding,
        }

        if create_pins:
            pins = (
                self.root / "third_party/v8/rusty_v8_150_4_0_release_manifests.sha256"
            )
            pins.parent.mkdir(parents=True)
            digest = trusted_digest or hashlib.sha256(manifest).hexdigest()
            name = trusted_name or manifest_name
            pins.write_bytes(f"{digest}  {name}".encode() + line_ending)

        def download(_url: str, destination: Path) -> None:
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payloads[destination.name])

        with (
            patch.object(v8, "REPO_ROOT", self.root),
            patch.object(v8, "download_file", side_effect=download) as download_file,
        ):
            yield spec, download_file, manifest_name

    def test_fetches_artifacts_after_authenticating_manifest(self) -> None:
        with self.release("x86_64-unknown-linux-gnu") as (
            spec,
            download,
            manifest_name,
        ):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )

            self.assertEqual(artifacts.archive.read_bytes(), b"trusted V8 archive")
            self.assertEqual(artifacts.binding.read_bytes(), b"trusted V8 binding")
            self.assertEqual(download.call_args_list[0].args[1].name, manifest_name)
            self.assertEqual(download.call_count, 3)

    def test_authenticates_windows_manifest_with_crlf(self) -> None:
        with self.release("x86_64-pc-windows-msvc", line_ending=b"\r\n") as (
            spec,
            download,
            _manifest_name,
        ):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )

            self.assertEqual(artifacts.archive.read_bytes(), b"trusted V8 archive")
            self.assertEqual(artifacts.binding.read_bytes(), b"trusted V8 binding")
            self.assertEqual(download.call_count, 3)

    def test_rejects_tampered_manifest_before_downloading_artifacts(self) -> None:
        with self.release("x86_64-unknown-linux-gnu", trusted_digest="0" * 64) as (
            spec,
            download,
            manifest_name,
        ):
            with self.assertRaisesRegex(
                RuntimeError, "does not match its trusted SHA-256"
            ):
                v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )

            download.assert_called_once()
            self.assertEqual(download.call_args.args[1].name, manifest_name)

    def test_rejects_missing_manifest_pin_before_downloading_artifacts(self) -> None:
        with self.release(
            "x86_64-unknown-linux-gnu", trusted_name="another-target.sha256"
        ) as (spec, download, manifest_name):
            with self.assertRaisesRegex(RuntimeError, "has no trusted SHA-256"):
                v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )

            download.assert_called_once()
            self.assertEqual(download.call_args.args[1].name, manifest_name)

    def test_rejects_missing_pin_file_before_downloading_artifacts(self) -> None:
        with self.release("x86_64-unknown-linux-gnu", create_pins=False) as (
            spec,
            download,
            manifest_name,
        ):
            with self.assertRaises(FileNotFoundError):
                v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )

            download.assert_called_once()
            self.assertEqual(download.call_args.args[1].name, manifest_name)

    def test_verified_cache_needs_no_network(self) -> None:
        with self.release("x86_64-unknown-linux-gnu") as (spec, download, _):
            first = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            download.reset_mock()
            download.side_effect = AssertionError("unexpected network request")

            second = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            self.assertEqual(second, first)
            download.assert_not_called()

    def test_missing_or_corrupt_manifest_refresh_retains_valid_artifacts(self) -> None:
        with self.release("x86_64-unknown-linux-gnu") as (spec, download, name):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            manifest = artifacts.archive.parent / name
            expected = manifest.read_bytes()
            for contents in (None, b"corrupt"):
                with self.subTest(contents=contents):
                    if contents is None:
                        manifest.unlink()
                    else:
                        manifest.write_bytes(contents)
                    download.reset_mock()
                    refreshed = v8.fetch_codex_v8_artifacts(
                        spec, version=self.version, cache_root=self.root / "cache"
                    )
                    self.assertEqual(refreshed, artifacts)
                    self.assertEqual(manifest.read_bytes(), expected)
                    download.assert_called_once()
                    self.assertEqual(download.call_args.args[1], manifest)

    def test_cached_manifest_still_authenticates_both_artifacts(self) -> None:
        with self.release("x86_64-unknown-linux-gnu") as (spec, download, _):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            artifacts.archive.write_bytes(b"corrupt archive")
            artifacts.binding.write_bytes(b"corrupt binding")
            download.reset_mock()
            repaired = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            self.assertEqual(repaired, artifacts)
            self.assertEqual(
                (repaired.archive.read_bytes(), repaired.binding.read_bytes()),
                (b"trusted V8 archive", b"trusted V8 binding"),
            )
            self.assertEqual(
                [call.args[1] for call in download.call_args_list],
                [artifacts.archive, artifacts.binding],
            )

    def test_changed_repository_pin_rejects_old_cache_and_bad_refresh(self) -> None:
        with self.release("x86_64-unknown-linux-gnu") as (spec, download, name):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            pins = (
                self.root / "third_party/v8/rusty_v8_150_4_0_release_manifests.sha256"
            )
            pins.write_text(f"{'0' * 64}  {name}\n")
            download.reset_mock()
            with self.assertRaisesRegex(
                RuntimeError, "does not match its trusted SHA-256"
            ):
                v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )
            download.assert_called_once()
            self.assertEqual(
                download.call_args.args[1], artifacts.archive.parent / name
            )
            self.assertFalse((artifacts.archive.parent / name).exists())
            self.assertEqual(artifacts.archive.read_bytes(), b"trusted V8 archive")
            self.assertEqual(artifacts.binding.read_bytes(), b"trusted V8 binding")

    @unittest.skipIf(sys.platform == "win32", "requires unprivileged symlinks")
    def test_symlink_manifest_is_replaced_without_writing_through_link(self) -> None:
        actual_download = v8.download_file
        with self.release("x86_64-unknown-linux-gnu") as (spec, download, name):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            manifest = artifacts.archive.parent / name
            expected = manifest.read_bytes()
            backing = manifest.with_suffix(".original")
            manifest.rename(backing)
            manifest.symlink_to(backing)
            download.reset_mock()
            download.side_effect = actual_download
            with patch.object(v8, "urlopen", return_value=io.BytesIO(expected)):
                refreshed = v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )
            self.assertEqual(refreshed, artifacts)
            self.assertFalse(manifest.is_symlink())
            self.assertEqual(
                (manifest.read_bytes(), backing.read_bytes()), (expected, expected)
            )
            download.assert_called_once()

    def test_interrupted_refresh_preserves_cache(self) -> None:
        actual_download = v8.download_file

        class InterruptedResponse(io.BytesIO):
            def read(self, size=-1):
                if self.tell():
                    raise OSError("interrupted download")
                return super().read(size)

        with self.release("x86_64-unknown-linux-gnu") as (spec, download, name):
            artifacts = v8.fetch_codex_v8_artifacts(
                spec, version=self.version, cache_root=self.root / "cache"
            )
            manifest = artifacts.archive.parent / name
            manifest.write_bytes(b"stale manifest")
            download.reset_mock()
            download.side_effect = actual_download
            with (
                patch.object(
                    v8, "urlopen", return_value=InterruptedResponse(b"partial")
                ),
                self.assertRaisesRegex(OSError, "interrupted download"),
            ):
                v8.fetch_codex_v8_artifacts(
                    spec, version=self.version, cache_root=self.root / "cache"
                )
            self.assertEqual(manifest.read_bytes(), b"stale manifest")
            self.assertEqual(artifacts.archive.read_bytes(), b"trusted V8 archive")
            self.assertEqual(artifacts.binding.read_bytes(), b"trusted V8 binding")
            self.assertFalse(list(manifest.parent.glob("*.tmp")))

    def test_source_and_paired_overrides_do_not_touch_cache(self) -> None:
        spec = TARGET_SPECS["x86_64-unknown-linux-gnu"]
        for environ in (
            {"V8_FROM_SOURCE": "1"},
            {
                "RUSTY_V8_ARCHIVE": "archive.a",
                "RUSTY_V8_SRC_BINDING_PATH": "binding.rs",
            },
        ):
            with (
                self.subTest(environ=environ),
                patch.object(
                    v8,
                    "fetch_codex_v8_artifacts",
                    side_effect=AssertionError("unexpected fetch"),
                ) as fetch,
            ):
                self.assertEqual(
                    v8.resolve_codex_v8_cargo_env(spec, environ=environ), {}
                )
                fetch.assert_not_called()


if __name__ == "__main__":
    unittest.main()
