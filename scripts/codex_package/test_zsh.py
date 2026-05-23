#!/usr/bin/env python3

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.dotslash import artifact_for_target
from codex_package.targets import TARGET_SPECS
from codex_package.zsh import ZSH_MANIFEST


class ZshManifestTest(unittest.TestCase):
    def test_manifest_covers_unix_release_targets(self) -> None:
        release_targets = [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "aarch64-unknown-linux-musl",
            "x86_64-unknown-linux-musl",
        ]

        self.assertEqual(
            {
                target: artifact_for_target(
                    TARGET_SPECS[target],
                    ZSH_MANIFEST,
                    artifact_label="codex-zsh",
                )
                .url.rsplit("/", 1)[-1]
                for target in release_targets
            },
            {
                target: f"codex-zsh-{target}.tar.gz"
                for target in release_targets
            },
        )


if __name__ == "__main__":
    unittest.main()
