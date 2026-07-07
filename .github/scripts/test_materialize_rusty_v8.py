#!/usr/bin/env python3

from __future__ import annotations

import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path


SCRIPTS_ROOT = Path(__file__).resolve().parents[2] / "scripts"
sys.path.insert(0, str(SCRIPTS_ROOT))

import materialize_rusty_v8
from rusty_v8_artifacts import RustyV8ArtifactManifest


class RustyV8ArtifactManifestTest(unittest.TestCase):
    def test_loads_complete_artifact_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            patch = root / "third_party/v8/patches/recipe-2/fix.patch"
            patch.parent.mkdir(parents=True)
            patch.write_text("patch", encoding="utf-8")
            manifest_path = root / "artifacts.toml"
            manifest_path.write_text(
                textwrap.dedent(
                    """\
                    schema_version = 1
                    wrapper_version = "149.2.0"
                    wrapper_v8_version = "14.9.207.2"
                    v8_version = "14.9.207.35"
                    v8_source_commit = "933ce636c562cd54d68e7f7c93ab5cdffd685fca"
                    patch_recipe = 2
                    artifact_identity = "rusty-v8-v149.2.0-v8-14.9.207.35-recipe-2"
                    patches = ["third_party/v8/patches/recipe-2/fix.patch"]
                    """
                ),
                encoding="utf-8",
            )

            manifest = RustyV8ArtifactManifest.load(
                manifest_path,
                repo_root=root,
            )

            self.assertEqual(
                manifest,
                RustyV8ArtifactManifest(
                    schema_version=1,
                    wrapper_version="149.2.0",
                    wrapper_v8_version="14.9.207.2",
                    v8_version="14.9.207.35",
                    v8_source_commit="933ce636c562cd54d68e7f7c93ab5cdffd685fca",
                    patch_recipe=2,
                    artifact_identity=(
                        "rusty-v8-v149.2.0-v8-14.9.207.35-recipe-2"
                    ),
                    patches=(
                        Path("third_party/v8/patches/recipe-2/fix.patch"),
                    ),
                ),
            )

    def test_rejects_identity_drift(self) -> None:
        manifest = RustyV8ArtifactManifest(
            schema_version=1,
            wrapper_version="149.2.0",
            wrapper_v8_version="14.9.207.2",
            v8_version="14.9.207.35",
            v8_source_commit="933ce636c562cd54d68e7f7c93ab5cdffd685fca",
            patch_recipe=2,
            artifact_identity="wrong",
            patches=(Path("third_party/v8/patches/recipe-2/fix.patch"),),
        )

        with self.assertRaisesRegex(ValueError, "does not match"):
            manifest.validate(Path("/does/not/matter"))

    def test_rejects_independent_engine_branch_change(self) -> None:
        manifest = RustyV8ArtifactManifest(
            schema_version=1,
            wrapper_version="149.2.0",
            wrapper_v8_version="14.9.207.2",
            v8_version="14.9.208.1",
            v8_source_commit="933ce636c562cd54d68e7f7c93ab5cdffd685fca",
            patch_recipe=2,
            artifact_identity="rusty-v8-v149.2.0-v8-14.9.208.1-recipe-2",
            patches=(Path("third_party/v8/patches/recipe-2/fix.patch"),),
        )

        with self.assertRaisesRegex(
            ValueError,
            "must stay on wrapper patch line",
        ):
            manifest.validate(Path("/does/not/matter"))


class MaterializeRustyV8Test(unittest.TestCase):
    def test_materializes_pinned_wrapper_engine_and_patch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            wrapper = root / "wrapper"
            v8 = root / "v8"
            checkout = root / "checkout"
            self.init_repository(wrapper)
            (wrapper / "Cargo.toml").write_text(
                '[package]\nname = "v8"\nversion = "149.2.0"\n',
                encoding="utf-8",
            )
            (wrapper / "README.md").write_text(
                "V8 Version: 14.9.207.2\n",
                encoding="utf-8",
            )
            (wrapper / ".gitmodules").write_text("", encoding="utf-8")
            (wrapper / "tools").mkdir()
            self.commit_all(wrapper, "wrapper")
            self.git(wrapper, "tag", "v149.2.0")

            self.init_repository(v8)
            version_header = v8 / "include" / "v8-version.h"
            version_header.parent.mkdir(parents=True)
            version_header.write_text(
                textwrap.dedent(
                    """\
                    #define V8_MAJOR_VERSION 14
                    #define V8_MINOR_VERSION 9
                    #define V8_BUILD_NUMBER 207
                    #define V8_PATCH_LEVEL 35
                    """
                ),
                encoding="utf-8",
            )
            target = v8 / "target.txt"
            target.write_text("before\n", encoding="utf-8")
            v8_commit = self.commit_all(v8, "engine")

            recipe = root / "third_party/v8/patches/recipe-1"
            recipe.mkdir(parents=True)
            target.write_text("after\n", encoding="utf-8")
            patch = recipe / "change.patch"
            patch.write_text(
                self.git(v8, "diff", capture_output=True) + "\n",
                encoding="utf-8",
            )
            self.git(v8, "restore", "target.txt")

            manifest = RustyV8ArtifactManifest(
                schema_version=1,
                wrapper_version="149.2.0",
                wrapper_v8_version="14.9.207.2",
                v8_version="14.9.207.35",
                v8_source_commit=v8_commit,
                patch_recipe=1,
                artifact_identity=(
                    "rusty-v8-v149.2.0-v8-14.9.207.35-recipe-1"
                ),
                patches=(
                    Path("third_party/v8/patches/recipe-1/change.patch"),
                ),
            )
            manifest.validate(root)

            materialize_rusty_v8.materialize(
                checkout,
                manifest,
                wrapper_repository=str(wrapper),
                v8_repository=str(v8),
                repo_root=root,
                sync_dependencies=False,
            )

            self.assertEqual(
                (checkout / "v8" / "target.txt").read_text(encoding="utf-8"),
                "after\n",
            )
            self.assertEqual(
                self.git(
                    checkout / "v8",
                    "rev-parse",
                    "HEAD",
                    capture_output=True,
                ),
                v8_commit,
            )

    def init_repository(self, root: Path) -> None:
        root.mkdir()
        self.git(root, "init", "--initial-branch=main")
        self.git(root, "config", "user.name", "Codex Test")
        self.git(root, "config", "user.email", "codex@example.com")

    def commit_all(self, root: Path, message: str) -> str:
        self.git(root, "add", ".")
        self.git(root, "commit", "-m", message)
        return self.git(root, "rev-parse", "HEAD", capture_output=True)

    def git(
        self,
        root: Path,
        *args: str,
        capture_output: bool = False,
    ) -> str:
        completed = subprocess.run(
            ["git", *args],
            cwd=root,
            check=True,
            text=True,
            capture_output=capture_output,
        )
        return completed.stdout.strip() if capture_output else ""


if __name__ == "__main__":
    unittest.main()
