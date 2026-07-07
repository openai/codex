import subprocess
import tempfile
import unittest
from pathlib import Path

from v8_canary_changes import changed_files
from v8_canary_changes import canary_required
from v8_canary_changes import merge_base
from v8_canary_changes import resolved_v8_version
from v8_canary_changes import windows_source_required


class V8CanaryChangesTest(unittest.TestCase):
    def test_resolved_v8_version(self) -> None:
        cargo_lock = b"""\
[[package]]
name = "other"
version = "1.0.0"

[[package]]
name = "v8"
version = "149.2.0"
"""

        self.assertEqual(resolved_v8_version(cargo_lock), "149.2.0")

    def test_unrelated_cargo_manifest_change_does_not_require_source_build(
        self,
    ) -> None:
        self.assertFalse(
            windows_source_required(
                {"codex-rs/Cargo.toml"},
                "149.2.0",
                "149.2.0",
            )
        )

    def test_v8_version_change_requires_source_build(self) -> None:
        self.assertTrue(windows_source_required(set(), "149.2.0", "150.0.0"))

    def test_module_helper_change_requires_source_build(self) -> None:
        self.assertTrue(
            windows_source_required(
                {".github/scripts/rusty_v8_module_bazel.py"},
                "149.2.0",
                "149.2.0",
            )
        )

    def test_setup_ci_change_requires_canary_and_source_build(self) -> None:
        changed_files = {".github/actions/setup-ci/action.yml"}

        self.assertTrue(canary_required(changed_files, "149.2.0", "149.2.0"))
        self.assertTrue(
            windows_source_required(changed_files, "149.2.0", "149.2.0")
        )

    def test_manual_dispatch_requires_source_build(self) -> None:
        self.assertTrue(
            windows_source_required(
                set(),
                "149.2.0",
                "149.2.0",
                force=True,
            )
        )

    def test_changed_files_excludes_changes_made_only_on_base_branch(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            self.run_git(root, "init", "--initial-branch=main")
            self.run_git(root, "config", "user.name", "Test User")
            self.run_git(root, "config", "user.email", "test@example.com")

            self.write_and_commit(root, "initial", "initial.txt")
            common = self.run_git(root, "rev-parse", "HEAD")
            self.run_git(root, "switch", "-c", "feature")
            self.run_git(root, "switch", "main")
            self.write_and_commit(root, "base-only", "base-only.txt")
            base = self.run_git(root, "rev-parse", "HEAD")

            self.run_git(root, "switch", "feature")
            self.write_and_commit(root, "feature-only", "feature-only.txt")
            head = self.run_git(root, "rev-parse", "HEAD")

            self.assertEqual(
                changed_files(base, head, root=root),
                {"feature-only.txt"},
            )
            self.assertEqual(merge_base(base, head, root=root), common)

    def write_and_commit(self, root: Path, contents: str, path: str) -> None:
        (root / path).write_text(contents)
        self.run_git(root, "add", path)
        self.run_git(root, "commit", "-m", contents)

    def run_git(self, root: Path, *args: str) -> str:
        return subprocess.check_output(
            ["git", *args],
            cwd=root,
            stderr=subprocess.PIPE,
            text=True,
        ).strip()


if __name__ == "__main__":
    unittest.main()
