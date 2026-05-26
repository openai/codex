#!/usr/bin/env python3

from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.archive import resolve_zstd_command


class ResolveZstdCommandTest(unittest.TestCase):
    def test_prefers_zstd_from_path(self) -> None:
        def which(name: str) -> str | None:
            return {"zstd": "/usr/bin/zstd", "dotslash": "/usr/bin/dotslash"}.get(name)

        self.assertEqual(resolve_zstd_command(which=which), ["/usr/bin/zstd"])

    def test_falls_back_to_dotslash_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            manifest = Path(temp_dir) / "zstd"
            manifest.write_text("#!/usr/bin/env dotslash\n{}\n", encoding="utf-8")

            def which(name: str) -> str | None:
                return {"dotslash": "/usr/bin/dotslash"}.get(name)

            self.assertEqual(
                resolve_zstd_command(dotslash_manifest=manifest, which=which),
                ["/usr/bin/dotslash", str(manifest)],
            )

    def test_errors_when_no_zstd_or_dotslash_manifest_is_available(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            missing_manifest = Path(temp_dir) / "zstd"

            with self.assertRaisesRegex(RuntimeError, "zstd is required"):
                resolve_zstd_command(
                    dotslash_manifest=missing_manifest,
                    which=lambda _name: None,
                )


if __name__ == "__main__":
    unittest.main()
