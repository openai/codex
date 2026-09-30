import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

from npm_alpha_tag import npm_alpha_tag
from releases import is_valid_release_version, should_update_version


class VersionComparisonTest(unittest.TestCase):
    def test_validation_accepts_alpha_hotfix(self) -> None:
        self.assertTrue(is_valid_release_version("0.123.0-alpha.5.2"))

    def test_validation_rejects_extra_component(self) -> None:
        self.assertFalse(is_valid_release_version("0.123.0-alpha.5.2.3"))

    def test_next_alpha_after_public_release(self) -> None:
        self.assertTrue(should_update_version("0.124.0-alpha.1", "0.123.0"))

    def test_public_release_after_next_alpha(self) -> None:
        self.assertFalse(should_update_version("0.123.0", "0.124.0-alpha.1"))

    def test_hotfix_for_an_older_release_line(self) -> None:
        self.assertFalse(should_update_version("0.100.0-alpha.1.2", "0.123.0-alpha.5"))

    def test_hotfix_for_an_older_alpha(self) -> None:
        self.assertFalse(should_update_version("0.123.0-alpha.2.3", "0.123.0-alpha.10"))

    def test_hotfix_for_the_current_alpha(self) -> None:
        self.assertTrue(should_update_version("0.123.0-alpha.5.2", "0.123.0-alpha.5"))

    def test_hotfix_numbers_compare_numerically(self) -> None:
        self.assertTrue(
            should_update_version("0.123.0-alpha.5.10", "0.123.0-alpha.5.2")
        )

    def test_numbered_alpha_after_bare_alpha(self) -> None:
        self.assertTrue(should_update_version("0.123.0-alpha.1", "0.123.0-alpha"))

    def test_beta_after_alpha(self) -> None:
        self.assertTrue(should_update_version("0.123.0-beta", "0.123.0-alpha.10"))

    def test_public_release_after_beta(self) -> None:
        self.assertTrue(should_update_version("0.123.0", "0.123.0-beta.2"))

    def test_equal_version(self) -> None:
        self.assertFalse(should_update_version("0.123.0-alpha.5", "0.123.0-alpha.5"))

    def test_missing_current_version(self) -> None:
        self.assertTrue(should_update_version("0.123.0-alpha.5", ""))

    def test_invalid_current_version(self) -> None:
        self.assertTrue(should_update_version("0.123.0-alpha.5", "0.123"))

    def test_invalid_release_version(self) -> None:
        self.assertRaises(ValueError, should_update_version, "0.123", "")


class NpmAlphaTagTest(unittest.TestCase):
    def test_published_tags_across_packages_and_platforms(self) -> None:
        # Each package/tag reads its own registry value, even after a partially
        # published release. Also exercise a missing pointer and dotted hotfix.
        cases = [
            (
                "@openai/codex",
                "alpha",
                "0.158.0-alpha.10",
                "0.157.0-alpha.11.1",
                "release-0.157.0-alpha.11.1-alpha",
            ),
            (
                "@openai/codex",
                "alpha-linux-x64",
                "0.158.0-alpha.10-linux-x64",
                "0.157.0-alpha.11.1",
                "release-0.157.0-alpha.11.1-alpha-linux-x64",
            ),
            (
                "@openai/codex",
                "alpha-win32-arm64",
                "0.158.0-alpha.9-win32-arm64",
                "0.158.0-alpha.10",
                "alpha-win32-arm64",
            ),
            (
                "@openai/codex-sdk",
                "alpha",
                "0.158.0-alpha.10",
                "0.158.0-alpha.10.1",
                "alpha",
            ),
            (
                "@openai/codex-responses-api-proxy",
                "alpha",
                "0.159.0-alpha.1",
                "0.158.0-alpha.10.1",
                "release-0.158.0-alpha.10.1-alpha",
            ),
            ("@openai/codex-sdk", "alpha", None, "0.158.0-alpha.10", "alpha"),
        ]
        with tempfile.TemporaryDirectory() as tmpdir:
            tarball = Path(tmpdir) / "package.tgz"
            for package, tag, current, version, expected in cases:
                with self.subTest(package=package, tag=tag, version=version):
                    content = json.dumps({"name": package}).encode()
                    with tarfile.open(tarball, "w:gz") as archive:
                        info = tarfile.TarInfo("package/package.json")
                        info.size = len(content)
                        archive.addfile(info, io.BytesIO(content))
                    tags = {tag: current} if current is not None else {}
                    result = subprocess.CompletedProcess([], 0, json.dumps(tags))
                    with patch(
                        "npm_alpha_tag.subprocess.run", return_value=result
                    ) as run:
                        self.assertEqual(npm_alpha_tag(tarball, version, tag), expected)
                    run.assert_called_once_with(
                        [
                            "npm",
                            "view",
                            package,
                            "dist-tags",
                            "--json",
                            "--prefer-online",
                        ],
                        check=True,
                        capture_output=True,
                        text=True,
                    )

    def test_registry_failure_prevents_publish(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tarball = Path(tmpdir) / "package.tgz"
            content = b'{"name": "@openai/codex"}'
            with tarfile.open(tarball, "w:gz") as archive:
                info = tarfile.TarInfo("package/package.json")
                info.size = len(content)
                archive.addfile(info, io.BytesIO(content))
            with patch(
                "npm_alpha_tag.subprocess.run",
                side_effect=subprocess.CalledProcessError(1, "npm"),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    npm_alpha_tag(tarball, "0.158.0-alpha.10", "alpha")


if __name__ == "__main__":
    unittest.main()
