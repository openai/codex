import io
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import textwrap
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


class DotSlashReleaseConfigTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        github_dir = Path(__file__).resolve().parent.parent
        cls.configs = {
            filename: json.loads((github_dir / filename).read_text())["outputs"]
            for filename in (
                "dotslash-config.json",
                "dotslash-argument-comment-lint-config.json",
                "dotslash-provisioned-config.json",
            )
        }
        # Names from the published rust-v0.167.0-alpha.2 release, which has no
        # Bazel assets: https://github.com/openai/codex/releases/tag/rust-v0.167.0-alpha.2
        cls.assets = set(
            (github_dir / "scripts/fixtures/codex-0.167.0-alpha.2-assets.txt")
            .read_text()
            .splitlines()
        )

    def missing_assets(self, assets):
        return sorted(
            (filename, output_name, platform)
            for filename, outputs in self.configs.items()
            for output_name, output in outputs.items()
            for platform, config in output["platforms"].items()
            if not any(re.match(config["regex"], asset) for asset in assets)
        )

    def test_cargo_release_covers_all_configured_outputs(self):
        self.assertEqual(
            {filename: set(outputs) for filename, outputs in self.configs.items()},
            {
                "dotslash-config.json": {
                    "codex",
                    "codex-app-server",
                    "codex-responses-api-proxy",
                    "bwrap",
                    "codex-command-runner",
                    "codex-windows-sandbox-setup",
                    "codex-windows-sandbox-service",
                },
                "dotslash-argument-comment-lint-config.json": {"argument-comment-lint"},
                "dotslash-provisioned-config.json": {"codex-provisioned"},
            },
        )
        self.assertFalse(any("-bazel" in asset for asset in self.assets))
        self.assertEqual(self.missing_assets(self.assets), [])

    def test_missing_cargo_asset_is_detected(self):
        missing_asset = "codex-package-aarch64-apple-darwin.tar.zst"
        self.assertIn(missing_asset, self.assets)
        self.assertEqual(
            self.missing_assets(self.assets - {missing_asset}),
            [("dotslash-config.json", "codex", "macos-aarch64")],
        )

    def test_missing_provisioned_cargo_asset_is_detected(self):
        for arch in ("aarch64", "x86_64"):
            with self.subTest(arch=arch):
                missing_asset = f"codex-provisioned-package-{arch}-apple-darwin.tar.gz"
                self.assertIn(missing_asset, self.assets)
                self.assertEqual(
                    self.missing_assets(self.assets - {missing_asset}),
                    [
                        (
                            "dotslash-provisioned-config.json",
                            "codex-provisioned",
                            f"macos-{arch}",
                        )
                    ],
                )


class StablePublicationTests(unittest.TestCase):
    def test_github_release_upload_and_cleanup(self):
        bash = shutil.which("bash")
        if bash is None:
            self.skipTest("Release workflow requires Bash 4.4 or newer")
        version = subprocess.check_output(
            [bash, "-c", 'echo "${BASH_VERSINFO[0]} ${BASH_VERSINFO[1]}"'],
            text=True,
        )
        if tuple(map(int, version.split())) < (4, 4):
            self.skipTest("Release workflow requires Bash 4.4 or newer")

        script = Path(__file__).resolve().with_name("publish-github-release.sh")
        with tempfile.TemporaryDirectory() as tmpdir:
            root = Path(tmpdir)
            gh = root / "gh"
            gh.write_text(
                textwrap.dedent("""\
                    #!/usr/bin/env bash
                    set -euo pipefail
                    case "$1 $2" in
                      'api graphql')
                        if compgen -G 'release-*' >/dev/null; then echo true; else echo false; fi ;;
                      'api --silent')
                        [[ "$3" == "repos/test/codex/git/ref/tags/$GITHUB_REF_NAME" ]]
                        [[ -f "${3##*/}.tag" ]] || exit 5 ;;
                      'api --method')
                        case "$3 $4" in
                          'POST repos/test/codex/releases')
                            printf '%s\\n' "$@" > "release-$CREATE_ID"
                            echo "$CREATE_ID" ;;
                          'DELETE repos/test/codex/releases/'*)
                            echo "DELETE $4" >> operations
                            rm -- "release-${4##*/}" ;;
                          *) exit 99 ;;
                        esac ;;
                      'release upload')
                        echo upload >> operations
                        [[ "$3" == "$GITHUB_REF_NAME" && -f "$4" ]]
                        if [[ "$FAIL_AT" == replace ]]; then
                          mv "release-$CREATE_ID" release-999
                          exit 7
                        fi
                        [[ "$FAIL_AT" != upload ]] || exit 7 ;;
                      'release edit')
                        echo edit >> operations
                        [[ "$FAIL_AT" != edit ]] || exit 8 ;;
                      *) exit 99 ;;
                    esac
                    """)
            )
            gh.chmod(0o755)
            alpha = "rust-v0.100.0-alpha.1"
            delete = "DELETE repos/test/codex/releases"
            # Repeated tags reuse the preceding release state for a retry.
            for tag, fail_at, create_id, status, surviving_id, operations in (
                (alpha, "upload", 101, 7, None, ["upload", f"{delete}/101"]),
                (alpha, "", 102, 0, 102, ["upload", "edit"]),
                (alpha, "upload", 103, 7, 102, ["upload"]),
                ("rust-v0.100.0-alpha.2", "edit", 104, 8, 104, ["upload", "edit"]),
                ("rust-v0.100.0", "upload", 105, 7, 105, ["upload"]),
                (
                    "rust-v0.100.0-alpha.3",
                    "replace",
                    106,
                    7,
                    999,
                    ["upload", f"{delete}/106"],
                ),
                ("rust-v0.100.0-alpha.4", "missing_tag", 107, 5, None, []),
            ):
                with self.subTest(tag=tag, fail_at=fail_at):
                    directory = root / tag
                    tag_file = directory / f"{tag}.tag"
                    if not directory.exists():
                        (directory / "dist").mkdir(parents=True)
                        (directory / "dist/package.tar.gz").write_bytes(b"package")
                        (directory / "notes.md").write_text("release notes\n")
                        if fail_at != "missing_tag":
                            tag_file.touch()
                    operation_log = directory / "operations"
                    operation_log.write_text("")
                    prerelease = "-alpha." in tag
                    result = subprocess.run(
                        [bash, str(script)],
                        cwd=directory,
                        env={
                            **os.environ,
                            "PATH": f"{root}{os.pathsep}{os.environ['PATH']}",
                            "GITHUB_REPOSITORY": "test/codex",
                            "GITHUB_REPOSITORY_OWNER": "test",
                            "GITHUB_REF_NAME": tag,
                            "RELEASE_NAME": tag.removeprefix("rust-v"),
                            "RELEASE_NOTES": "notes.md",
                            "MAKE_LATEST": str(not prerelease).lower(),
                            "PRERELEASE": str(prerelease).lower(),
                            "FAIL_AT": fail_at,
                            "CREATE_ID": str(create_id),
                        },
                        capture_output=True,
                        text=True,
                    )
                    self.assertEqual(result.returncode, status, result.stderr)
                    self.assertEqual(tag_file.exists(), fail_at != "missing_tag")
                    self.assertEqual(operation_log.read_text().splitlines(), operations)
                    releases = list(directory.glob("release-*"))
                    self.assertEqual(
                        [p.name for p in releases],
                        [f"release-{surviving_id}"] if surviving_id else [],
                    )
                    if surviving_id:
                        fields = releases[0].read_text().splitlines()
                        self.assertIn(f"tag_name={tag}", fields)
                        self.assertIn(f"draft={str(not prerelease).lower()}", fields)
                        self.assertIn(f"prerelease={str(prerelease).lower()}", fields)

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
