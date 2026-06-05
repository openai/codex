#!/usr/bin/env python3

import hashlib
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.targets import TARGET_SPECS
from codex_package.v8 import fetch_codex_v8_artifacts


class RustyV8ArtifactsTest(unittest.TestCase):
    def test_fetches_pointer_compression_sandbox_artifacts(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            payload = b"artifact"
            digest = hashlib.sha256(payload).hexdigest()

            def download_file(url: str, dest: Path) -> None:
                if dest.suffix == ".sha256":
                    target = "x86_64-unknown-linux-musl"
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    dest.write_text(
                        "\n".join(
                            [
                                f"{digest}  librusty_v8_ptrcomp_sandbox_release_{target}.a.gz",
                                f"{digest}  src_binding_ptrcomp_sandbox_release_{target}.rs",
                            ]
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                else:
                    dest.write_bytes(payload)

            with patch("codex_package.v8.download_file", side_effect=download_file):
                artifacts = fetch_codex_v8_artifacts(
                    TARGET_SPECS["x86_64-unknown-linux-musl"],
                    version="147.4.0",
                    cache_root=Path(temp_dir),
                )

            self.assertEqual(
                artifacts.archive.name,
                "librusty_v8_ptrcomp_sandbox_release_x86_64-unknown-linux-musl.a.gz",
            )
            self.assertEqual(
                artifacts.binding.name,
                "src_binding_ptrcomp_sandbox_release_x86_64-unknown-linux-musl.rs",
            )


if __name__ == "__main__":
    unittest.main()
