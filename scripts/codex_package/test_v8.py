#!/usr/bin/env python3

from pathlib import Path
import sys
import tempfile
import textwrap
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from codex_package.v8 import resolved_v8_source_version
from codex_package.v8 import rusty_v8_release_tag


class RustyV8ReleaseTagTest(unittest.TestCase):
    def test_release_tag_combines_crate_and_source_versions(self) -> None:
        self.assertEqual(
            rusty_v8_release_tag(
                crate_version="149.2.0",
                source_version="14.9.207.35",
            ),
            "rusty-v8-v149.2.0-v8-14.9.207.35",
        )

    def test_resolved_v8_source_version_reads_archive_override(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            module_bazel = Path(temp_dir) / "MODULE.bazel"
            module_bazel.write_text(
                textwrap.dedent(
                    """\
                    archive_override(
                        module_name = "v8",
                        urls = ["https://github.com/v8/v8/archive/refs/tags/14.9.207.35.tar.gz"],
                    )
                    """
                )
            )

            self.assertEqual(
                resolved_v8_source_version(module_bazel),
                "14.9.207.35",
            )


if __name__ == "__main__":
    unittest.main()
