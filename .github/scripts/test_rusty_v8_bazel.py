#!/usr/bin/env python3

from __future__ import annotations

import textwrap
import unittest
from pathlib import Path
from unittest.mock import Mock
from unittest.mock import patch

import rusty_v8_bazel
import rusty_v8_module_bazel


class RustyV8BazelTest(unittest.TestCase):
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
                Path("v8.lib"),
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

    @patch("rusty_v8_bazel.ensure_bazel_output_files")
    @patch("rusty_v8_bazel.subprocess.run")
    def test_host_runnable_bazel_output_file_selects_runnable_candidate(
        self,
        run: Mock,
        ensure_outputs: Mock,
    ) -> None:
        amd64_tool = Path("/tmp/llvm-amd64/bin/llvm-ar")
        arm64_tool = Path("/tmp/llvm-arm64/bin/llvm-ar")
        ensure_outputs.return_value = [amd64_tool, arm64_tool]
        run.side_effect = [
            OSError("Exec format error"),
            Mock(returncode=0),
        ]

        self.assertEqual(
            arm64_tool,
            rusty_v8_bazel.host_runnable_bazel_output_file(
                "linux_arm64_musl",
                "@llvm//tools:llvm-ar",
                "opt",
            ),
        )

    @patch("rusty_v8_bazel.ensure_bazel_output_files")
    @patch("rusty_v8_bazel.subprocess.run")
    def test_host_runnable_bazel_output_file_rejects_ambiguous_candidates(
        self,
        run: Mock,
        ensure_outputs: Mock,
    ) -> None:
        amd64_tool = Path("/tmp/llvm-amd64/bin/llvm-ar")
        arm64_tool = Path("/tmp/llvm-arm64/bin/llvm-ar")
        ensure_outputs.return_value = [amd64_tool, arm64_tool]
        run.side_effect = [
            Mock(returncode=0),
            Mock(returncode=0),
        ]

        with self.assertRaisesRegex(
            SystemExit,
            "expected exactly one host-runnable output",
        ):
            rusty_v8_bazel.host_runnable_bazel_output_file(
                "linux_arm64_musl",
                "@llvm//tools:llvm-ar",
                "opt",
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
