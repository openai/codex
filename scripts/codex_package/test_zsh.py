#!/usr/bin/env python3

import hashlib
import json
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.targets import TARGET_SPECS
from codex_package.zsh import resolve_zsh_bin


class ResolveZshBinTest(unittest.TestCase):
    def test_uses_manifest_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            archive = root / "codex-zsh.tar.gz"
            source = root / "zsh"
            source.write_bytes(b"standalone zsh")
            with tarfile.open(archive, "w:gz") as tar:
                tar.add(source, arcname="codex-zsh/bin/zsh")

            manifest = root / "codex-zsh"
            manifest.write_text(
                json.dumps(
                    {
                        "platforms": {
                            "linux-x86_64": {
                                "size": archive.stat().st_size,
                                "hash": "sha256",
                                "digest": hashlib.sha256(
                                    archive.read_bytes()
                                ).hexdigest(),
                                "format": "tar.gz",
                                "path": "codex-zsh/bin/zsh",
                                "providers": [{"url": archive.as_uri()}],
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )

            with patch(
                "codex_package.dotslash.default_cache_root",
                return_value=root / "cache",
            ):
                zsh_bin = resolve_zsh_bin(
                    TARGET_SPECS["x86_64-unknown-linux-musl"], manifest
                )

            self.assertIsNotNone(zsh_bin)
            assert zsh_bin is not None
            self.assertEqual(zsh_bin.read_bytes(), b"standalone zsh")


if __name__ == "__main__":
    unittest.main()
