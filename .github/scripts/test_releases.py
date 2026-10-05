import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch

from npm_alpha_tag import npm_alpha_tag
import publish_r2_release as r2
from releases import is_valid_release_version, should_make_latest, should_update_version


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
    def write_package(self, tarball: Path, package: str) -> None:
        content = json.dumps({"name": package}).encode()
        with tarfile.open(tarball, "w:gz") as archive:
            info = tarfile.TarInfo("package/package.json")
            info.size = len(content)
            archive.addfile(info, io.BytesIO(content))

    def test_published_tags_across_packages_and_platforms(self) -> None:
        # Each package/tag reads its own registry value, even after a partially
        # published release. Also exercise a missing pointer and dotted hotfix.
        cases = [
            (
                "@openai/codex-sdk",
                "latest",
                "0.100.0",
                "0.99.9",
                "release-0.99.9-latest",
            ),
            (
                "@openai/codex",
                "linux-x64",
                "0.99.9-linux-x64",
                "0.100.0",
                "linux-x64",
            ),
            ("@openai/codex-responses-api-proxy", "latest", None, "0.100.0", "latest"),
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
            (
                "@openai/codex",
                "alpha-linux-x64",
                "broken-linux-x64",
                "0.158.0-alpha.10",
                "alpha-linux-x64",
            ),
        ]
        with tempfile.TemporaryDirectory() as tmpdir:
            tarball = Path(tmpdir) / "package.tgz"
            for package, tag, current, version, expected in cases:
                with self.subTest(package=package, tag=tag, version=version):
                    self.write_package(tarball, package)
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

    def test_stable_tags_reject_malformed_and_prerelease_versions(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tarball = Path(tmpdir) / "package.tgz"
            self.write_package(tarball, "@openai/codex")
            for tag, current in (
                ("latest", "broken"),
                ("linux-x64", "0.100.0-alpha.1-linux-x64"),
            ):
                with self.subTest(tag=tag, current=current):
                    result = subprocess.CompletedProcess(
                        [], 0, json.dumps({tag: current})
                    )
                    with patch("npm_alpha_tag.subprocess.run", return_value=result):
                        with self.assertRaises(ValueError):
                            npm_alpha_tag(tarball, "0.99.9", tag)

    def test_registry_failure_prevents_publish(self) -> None:
        with tempfile.TemporaryDirectory() as tmpdir:
            tarball = Path(tmpdir) / "package.tgz"
            self.write_package(tarball, "@openai/codex")
            with patch(
                "npm_alpha_tag.subprocess.run",
                side_effect=subprocess.CalledProcessError(1, "npm"),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    npm_alpha_tag(tarball, "0.158.0-alpha.10", "alpha")


class StablePublicationTests(unittest.TestCase):
    def test_github_latest_orders_numerically_and_allows_equal_reruns(self):
        latest = {"tag_name": "rust-v0.100.0", "draft": False, "prerelease": False}
        self.assertFalse(should_make_latest("0.99.9", latest))
        self.assertTrue(should_make_latest("0.100.0", latest))
        self.assertTrue(should_make_latest("0.101.0", latest))

    def test_github_latest_rejects_unpublished_or_unreadable_metadata(self):
        for latest in (
            {},
            {"tag_name": "rust-vunknown", "draft": False, "prerelease": False},
            {"tag_name": "rust-v0.100.0", "draft": True, "prerelease": False},
            {"tag_name": "rust-v0.100.0", "draft": False, "prerelease": True},
        ):
            with self.subTest(latest=latest):
                with self.assertRaises(ValueError):
                    should_make_latest("0.99.9", latest)

    def test_r2_checks_its_channel_and_only_repairs_unreadable_prereleases(self):
        with patch.object(r2, "run_command") as command:
            command.return_value = '{"tag_name":"rust-v0.100.0"}'
            self.assertFalse(r2.should_update_channel("endpoint", "0.99.9", "latest"))
            self.assertTrue(r2.should_update_channel("endpoint", "0.100.0", "latest"))
            self.assertIn(
                "s3://releases/codex/channels/latest", command.call_args.args[0]
            )
            command.return_value = "invalid"
            with self.assertRaises(r2.PublishError):
                r2.should_update_channel("endpoint", "0.99.9", "latest")
            self.assertTrue(
                r2.should_update_channel("endpoint", "0.99.9-alpha.1", "prerelease")
            )


if __name__ == "__main__":
    unittest.main()
