#!/usr/bin/env python3

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.targets import TARGET_SPECS
from codex_package.v8 import archive_name
from rusty_v8_artifacts import RustyV8ArtifactManifest


class RustyV8ReleaseTagTest(unittest.TestCase):
    def test_release_tag_comes_from_artifact_manifest(self) -> None:
        self.assertEqual(
            RustyV8ArtifactManifest.load().artifact_identity,
            "rusty-v8-v149.2.0-v8-14.9.207.35-recipe-1",
        )

    def test_archive_name_supports_unix_and_msvc_targets(self) -> None:
        self.assertEqual(
            archive_name(TARGET_SPECS["x86_64-unknown-linux-gnu"]),
            "librusty_v8_release_x86_64-unknown-linux-gnu.a.gz",
        )
        self.assertEqual(
            archive_name(TARGET_SPECS["aarch64-pc-windows-msvc"]),
            "rusty_v8_release_aarch64-pc-windows-msvc.lib.gz",
        )


if __name__ == "__main__":
    unittest.main()
