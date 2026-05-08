#!/usr/bin/env python3

from __future__ import annotations

import textwrap
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

import rusty_v8_bazel
import rusty_v8_module_bazel


class RustyV8BazelTest(unittest.TestCase):
    def test_artifact_bazel_configs_always_enable_upstream_libcxx(self) -> None:
        self.assertEqual(
            ["rusty-v8-upstream-libcxx"],
            rusty_v8_bazel.artifact_bazel_configs(),
        )
        self.assertEqual(
            ["rusty-v8-upstream-libcxx", "v8-release-compat"],
            rusty_v8_bazel.artifact_bazel_configs(["v8-release-compat"]),
        )
        self.assertEqual(
            ["rusty-v8-upstream-libcxx", "v8-release-compat"],
            rusty_v8_bazel.artifact_bazel_configs(
                ["rusty-v8-upstream-libcxx", "v8-release-compat"]
            ),
        )

    def test_release_pair_labels_and_staged_names_distinguish_sandbox_artifacts(self) -> None:
        self.assertEqual(
            "//third_party/v8:rusty_v8_release_pair_x86_64_unknown_linux_musl",
            rusty_v8_bazel.release_pair_label("x86_64-unknown-linux-musl"),
        )
        self.assertEqual(
            "//third_party/v8:rusty_v8_sandbox_release_pair_x86_64_unknown_linux_musl",
            rusty_v8_bazel.release_pair_label("x86_64-unknown-linux-musl", sandbox=True),
        )
        self.assertEqual(
            "//third_party/v8:rusty_v8_sandbox_release_pair_x86_64_apple_darwin",
            rusty_v8_bazel.release_pair_label("x86_64-apple-darwin", sandbox=True),
        )
        self.assertEqual(
            "librusty_v8_release_x86_64-unknown-linux-musl.a.gz",
            rusty_v8_bazel.staged_archive_name(
                "x86_64-unknown-linux-musl",
                Path("libv8.a"),
                rusty_v8_bazel.RELEASE_ARTIFACT_PROFILE,
            ),
        )
        self.assertEqual(
            "rusty_v8_ptrcomp_sandbox_release_x86_64-pc-windows-msvc.lib.gz",
            rusty_v8_bazel.staged_archive_name(
                "x86_64-pc-windows-msvc",
                Path("v8.a"),
                rusty_v8_bazel.SANDBOX_ARTIFACT_PROFILE,
            ),
        )
        self.assertEqual(
            "src_binding_ptrcomp_sandbox_release_x86_64-unknown-linux-musl.rs",
            rusty_v8_bazel.staged_binding_name(
                "x86_64-unknown-linux-musl",
                rusty_v8_bazel.SANDBOX_ARTIFACT_PROFILE,
            ),
        )
        self.assertEqual(
            "rusty_v8_ptrcomp_sandbox_release_x86_64-unknown-linux-musl.sha256",
            rusty_v8_bazel.staged_checksums_name(
                "x86_64-unknown-linux-musl",
                rusty_v8_bazel.SANDBOX_ARTIFACT_PROFILE,
            ),
        )

    def test_stage_artifacts(self) -> None:
        with TemporaryDirectory() as source_dir, TemporaryDirectory() as output_dir:
            source_root = Path(source_dir)
            archive = source_root / "librusty_v8.a"
            binding = source_root / "src_binding.rs"
            archive.write_bytes(b"archive")
            binding.write_text("binding")

            rusty_v8_bazel.stage_artifacts(
                "aarch64-apple-darwin",
                archive,
                binding,
                Path(output_dir),
                sandbox=True,
            )

            self.assertEqual(
                {
                    "librusty_v8_ptrcomp_sandbox_release_aarch64-apple-darwin.a.gz",
                    "src_binding_ptrcomp_sandbox_release_aarch64-apple-darwin.rs",
                    "rusty_v8_ptrcomp_sandbox_release_aarch64-apple-darwin.sha256",
                },
                {path.name for path in Path(output_dir).iterdir()},
            )

    def test_ensure_bazel_output_files_rebuilds_existing_outputs(self) -> None:
        with TemporaryDirectory() as output_dir:
            output = Path(output_dir) / "libv8.a"
            output.write_bytes(b"archive")

            with (
                patch.object(rusty_v8_bazel, "bazel_build") as bazel_build,
                patch.object(
                    rusty_v8_bazel,
                    "bazel_output_files",
                    return_value=[output],
                ) as bazel_output_files,
            ):
                self.assertEqual(
                    [output],
                    rusty_v8_bazel.ensure_bazel_output_files(
                        "macos_arm64",
                        ["//third_party/v8:pair"],
                        "opt",
                        ["rusty-v8-upstream-libcxx"],
                    ),
                )

            bazel_build.assert_called_once_with(
                "macos_arm64",
                ["//third_party/v8:pair"],
                "opt",
                ["rusty-v8-upstream-libcxx"],
            )
            bazel_output_files.assert_called_once_with(
                "macos_arm64",
                ["//third_party/v8:pair"],
                "opt",
                ["rusty-v8-upstream-libcxx"],
            )

    def test_update_module_bazel_replaces_and_inserts_sha256(self) -> None:
        module_bazel = textwrap.dedent(
            """\
            http_file(
                name = "rusty_v8_146_4_0_x86_64_unknown_linux_gnu_archive",
                downloaded_file_path = "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                sha256 = "0000000000000000000000000000000000000000000000000000000000000000",
                urls = [
                    "https://example.test/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                ],
            )

            http_file(
                name = "rusty_v8_146_4_0_x86_64_unknown_linux_musl_binding",
                downloaded_file_path = "src_binding_release_x86_64-unknown-linux-musl.rs",
                urls = [
                    "https://example.test/src_binding_release_x86_64-unknown-linux-musl.rs",
                ],
            )

            http_file(
                name = "rusty_v8_145_0_0_x86_64_unknown_linux_gnu_archive",
                downloaded_file_path = "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                sha256 = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                urls = [
                    "https://example.test/old.gz",
                ],
            )
            """
        )
        checksums = {
            "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz": (
                "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            "src_binding_release_x86_64-unknown-linux-musl.rs": (
                "2222222222222222222222222222222222222222222222222222222222222222"
            ),
        }

        updated = rusty_v8_module_bazel.update_module_bazel_text(
            module_bazel,
            checksums,
            "146.4.0",
        )

        self.assertEqual(
            textwrap.dedent(
                """\
                http_file(
                    name = "rusty_v8_146_4_0_x86_64_unknown_linux_gnu_archive",
                    downloaded_file_path = "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                    sha256 = "1111111111111111111111111111111111111111111111111111111111111111",
                    urls = [
                        "https://example.test/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                    ],
                )

                http_file(
                    name = "rusty_v8_146_4_0_x86_64_unknown_linux_musl_binding",
                    downloaded_file_path = "src_binding_release_x86_64-unknown-linux-musl.rs",
                    sha256 = "2222222222222222222222222222222222222222222222222222222222222222",
                    urls = [
                        "https://example.test/src_binding_release_x86_64-unknown-linux-musl.rs",
                    ],
                )

                http_file(
                    name = "rusty_v8_145_0_0_x86_64_unknown_linux_gnu_archive",
                    downloaded_file_path = "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                    sha256 = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                    urls = [
                        "https://example.test/old.gz",
                    ],
                )
                """
            ),
            updated,
        )
        rusty_v8_module_bazel.check_module_bazel_text(updated, checksums, "146.4.0")

    def test_check_module_bazel_rejects_manifest_drift(self) -> None:
        module_bazel = textwrap.dedent(
            """\
            http_file(
                name = "rusty_v8_146_4_0_x86_64_unknown_linux_gnu_archive",
                downloaded_file_path = "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                sha256 = "1111111111111111111111111111111111111111111111111111111111111111",
                urls = [
                    "https://example.test/librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
                ],
            )
            """
        )
        checksums = {
            "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz": (
                "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            "orphan.gz": (
                "2222222222222222222222222222222222222222222222222222222222222222"
            ),
        }

        with self.assertRaisesRegex(
            rusty_v8_module_bazel.RustyV8ChecksumError,
            "manifest has orphan.gz",
        ):
            rusty_v8_module_bazel.check_module_bazel_text(
                module_bazel,
                checksums,
                "146.4.0",
            )


if __name__ == "__main__":
    unittest.main()
