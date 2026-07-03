import os
import subprocess
import sys
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from unittest.mock import call
from unittest.mock import patch

import pipeline

PIPELINE_SCRIPT = Path(__file__).with_name("pipeline.py").resolve()
CANDIDATE_AUTHOR_DATE = "2026-01-02T03:04:05+00:00"


@dataclass(frozen=True)
class RepositoryFixture:
    repo: Path
    candidate: str
    later_candidate: str
    ready_parent: str


class PipelineIntegrationTest(unittest.TestCase):
    def test_prepares_and_publishes_exactly_one_ready_commit(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            run_pipeline(fixture.repo, runner_temp, "prepare")

            metadata = (runner_temp / "publish-metadata.json").read_text(
                encoding="utf-8"
            )
            self.assertIn(fixture.candidate, metadata)
            self.assertNotIn(fixture.later_candidate, metadata)
            (runner_temp / "public-commit-message.md").write_text(
                "Describe the public change\n\nPublic details.\n", encoding="utf-8"
            )
            run_pipeline(fixture.repo, runner_temp, "publish")

            fetch_ready(fixture.repo)
            ready = git_output(
                fixture.repo, "rev-parse", f"origin/{pipeline.READY_BRANCH}"
            )
            self.assertEqual(
                git_output(fixture.repo, "rev-parse", f"{ready}^"),
                fixture.ready_parent,
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:public.txt"),
                "candidate one",
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:{pipeline.STATE_FILE}"),
                fixture.candidate,
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:codex-rs/Cargo.lock"),
                cargo_lockfile().strip(),
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:MODULE.bazel.lock"),
                bazel_lockfile().strip(),
            )
            self.assertFalse(git_path_exists(fixture.repo, ready, pipeline.GITHUB_DIR))
            self.assertEqual(
                git_output(fixture.repo, "show", "--no-patch", "--format=%B", ready),
                "Describe the public change\n\nPublic details.",
            )
            self.assertEqual(
                git_output(
                    fixture.repo,
                    "show",
                    "--no-patch",
                    "--format=%an%x00%ae%x00%aI",
                    ready,
                ),
                git_output(
                    fixture.repo,
                    "show",
                    "--no-patch",
                    "--format=%an%x00%ae%x00%aI",
                    fixture.candidate,
                ),
            )

    def test_publish_rejects_ready_branch_movement(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()
            run_pipeline(fixture.repo, runner_temp, "prepare")
            (runner_temp / "public-commit-message.md").write_text(
                "Describe the public change\n", encoding="utf-8"
            )

            git(fixture.repo, "checkout", pipeline.READY_BRANCH)
            (fixture.repo / "concurrent.txt").write_text("moved\n", encoding="utf-8")
            commit_all(fixture.repo, "Advance ready concurrently")
            git(fixture.repo, "push", "origin", pipeline.READY_BRANCH)

            result = run_pipeline(fixture.repo, runner_temp, "publish", check=False)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("advanced from", result.stderr)

    def test_prepare_rejects_second_parent_state(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            repo, second_parent, candidate_head = create_second_parent_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            result = run_pipeline(repo, runner_temp, "prepare", check=False)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                f"{second_parent} is not on the first-parent chain of {candidate_head}",
                result.stderr,
            )

    def test_prepare_writes_public_only_message_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(
                root,
                candidate_content="x" * 20_000,
                additional_candidate_files={"nested/[second].txt": "second file\n"},
            )
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            run_pipeline(fixture.repo, runner_temp, "prepare")

            self.assertTrue(
                (runner_temp / "message-input/message_policy.py").is_file()
            )
            message_input = runner_temp / "message-input"
            target = (message_input / pipeline.MESSAGE_TARGET_FILE).read_text(
                encoding="utf-8"
            ).strip()
            workspace = root / "message-workspace"
            subprocess.run(
                [
                    "git",
                    "init",
                    "--initial-branch=main",
                    workspace.as_posix(),
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            git(workspace, "config", "user.name", "Public History")
            git(workspace, "config", "user.email", "public@example.com")
            (workspace / "public-history.txt").write_text(
                "public history\n", encoding="utf-8"
            )
            public_history_commit = commit_all(workspace, "Public history")
            git(
                workspace,
                "fetch",
                (message_input / pipeline.MESSAGE_WORKSPACE_BUNDLE).as_posix(),
                f"refs/heads/{pipeline.MESSAGE_WORKSPACE_BRANCH}:"
                f"refs/heads/{pipeline.MESSAGE_WORKSPACE_BRANCH}",
            )
            git(workspace, "checkout", pipeline.MESSAGE_WORKSPACE_BRANCH)

            self.assertEqual(
                git_output(workspace, "rev-parse", "HEAD"),
                target,
            )
            self.assertEqual(
                git_output(workspace, "rev-list", "--count", target),
                "2",
            )
            self.assertEqual(
                git_output(
                    workspace,
                    "show",
                    f"{public_history_commit}:public-history.txt",
                ),
                "public history",
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}^:public.txt"),
                "baseline",
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:public.txt"),
                "x" * 20_000,
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:nested/[second].txt"),
                "second file",
            )
            self.assertFalse(git_path_exists(workspace, target, pipeline.GITHUB_DIR))
            self.assertFalse(git_path_exists(workspace, target, pipeline.STATE_FILE))
            self.assertEqual(
                git_output(workspace, "show", "--no-patch", "--format=%s", target),
                pipeline.MESSAGE_PLACEHOLDER_SUBJECT,
            )
            self.assertEqual(
                git_output(
                    workspace,
                    "show",
                    "--no-patch",
                    "--format=%an%x00%ae%x00%aI",
                    target,
                ),
                git_output(
                    fixture.repo,
                    "show",
                    "--no-patch",
                    "--format=%an%x00%ae%x00%aI",
                    fixture.candidate,
                ),
            )

    def test_validate_message_reads_generated_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            repo = root / "repo"
            repo.mkdir()
            (repo / "public-history").mkdir()
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()
            message_file = runner_temp / "public-commit-message.md"
            message_file.write_text(
                "Describe the public change\n\nPublic details.\n\n",
                encoding="utf-8",
            )

            run_pipeline(repo, runner_temp, "validate-message")

            self.assertEqual(
                message_file.read_text(encoding="utf-8"),
                "Describe the public change\n\nPublic details.\n",
            )

    def test_validate_message_rejects_missing_generated_file(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            repo = root / "repo"
            repo.mkdir()
            (repo / "public-history").mkdir()
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            result = run_pipeline(
                repo,
                runner_temp,
                "validate-message",
                env={"CODEX_OUTPUT": "Do not accept this fallback"},
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("public-commit-message.md", result.stderr)

    def test_manual_message_override_publishes_without_model_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()
            github_output = runner_temp / "github-output"
            override_env = {
                "USE_PUBLIC_MESSAGE_OVERRIDE": "true",
                "EXPECTED_CANDIDATE_REVISION": fixture.candidate,
                "PUBLIC_SUBJECT": "Describe the reviewed public change",
                "PUBLIC_BODY": "Explain the reviewed public motivation.",
                "GITHUB_OUTPUT": github_output.as_posix(),
            }

            run_pipeline(
                fixture.repo,
                runner_temp,
                "prepare",
                env=override_env,
            )
            self.assertFalse((runner_temp / "message-input").exists())
            self.assertEqual(
                github_output.read_text(encoding="utf-8"),
                "has_change=true\nmessage_override=true\n",
            )
            run_pipeline(
                fixture.repo,
                runner_temp,
                "validate-message",
                env=override_env,
            )
            run_pipeline(fixture.repo, runner_temp, "publish")

            fetch_ready(fixture.repo)
            ready = git_output(
                fixture.repo, "rev-parse", f"origin/{pipeline.READY_BRANCH}"
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:{pipeline.STATE_FILE}"),
                fixture.candidate,
            )
            self.assertEqual(
                git_output(fixture.repo, "show", "--no-patch", "--format=%B", ready),
                "Describe the reviewed public change\n\n"
                "Explain the reviewed public motivation.",
            )

    def test_manual_message_override_rejects_wrong_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            result = run_pipeline(
                fixture.repo,
                runner_temp,
                "prepare",
                env={
                    "USE_PUBLIC_MESSAGE_OVERRIDE": "true",
                    "EXPECTED_CANDIDATE_REVISION": fixture.later_candidate,
                    "PUBLIC_SUBJECT": "Describe the wrong public change",
                },
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                f"expected candidate {fixture.later_candidate}", result.stderr
            )
            self.assertIn(
                f"next pending candidate is {fixture.candidate}", result.stderr
            )

    def test_manual_message_override_requires_explicit_opt_in(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            result = run_pipeline(
                fixture.repo,
                runner_temp,
                "prepare",
                env={
                    "EXPECTED_CANDIDATE_REVISION": fixture.candidate,
                    "PUBLIC_SUBJECT": "Describe the reviewed public change",
                },
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("Set USE_PUBLIC_MESSAGE_OVERRIDE=true", result.stderr)

    def test_manual_message_override_uses_public_message_validator(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            result = run_pipeline(
                fixture.repo,
                runner_temp,
                "validate-message",
                env={
                    "USE_PUBLIC_MESSAGE_OVERRIDE": "true",
                    "EXPECTED_CANDIDATE_REVISION": fixture.candidate,
                    "PUBLIC_SUBJECT": "Describe the reviewed public change",
                    "PUBLIC_BODY": "See https://openai.slack.com/archives/private",
                },
                check=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("a Slack URL", result.stderr)


class PipelineLockfileTest(unittest.TestCase):
    def test_reconcile_bazel_lockfile_updates_then_verifies(self) -> None:
        candidate_tree = Path("/candidate")

        with patch.object(pipeline, "run") as run:
            pipeline.reconcile_bazel_lockfile(candidate_tree)

        self.assertEqual(
            run.call_args_list,
            [
                call(
                    ["bazel", "mod", "deps", "--lockfile_mode=update"],
                    cwd=candidate_tree,
                    capture=True,
                ),
                call(
                    ["bazel", "mod", "deps", "--lockfile_mode=error"],
                    cwd=candidate_tree,
                    capture=True,
                ),
            ],
        )


def create_repository_fixture(
    root: Path,
    *,
    candidate_content: str = "candidate one\n",
    additional_candidate_files: dict[str, str] | None = None,
) -> RepositoryFixture:
    repo, baseline = create_base_repository(root)

    git(repo, "checkout", "-b", pipeline.READY_BRANCH, baseline)
    state_file = repo / pipeline.STATE_FILE
    state_file.parent.mkdir(parents=True, exist_ok=True)
    state_file.write_text(f"{baseline}\n", encoding="utf-8")
    ready_parent = commit_all(repo, "Seed ready state")
    git(repo, "push", "origin", pipeline.READY_BRANCH)

    git(repo, "checkout", "-b", pipeline.CANDIDATE_BRANCH, baseline)
    support = repo / ".github/codex-internal-to-public"
    support.mkdir(parents=True)
    (support / "prompt.md").write_text("internal support\n", encoding="utf-8")
    workflow = repo / ".github/workflows/codex-internal-to-public-staging.yml"
    workflow.parent.mkdir(parents=True)
    workflow.write_text("name: Internal staging\n", encoding="utf-8")
    public_github_file = repo / ".github/workflows/public-ci.yml"
    public_github_file.write_text("name: Public CI\n", encoding="utf-8")
    (repo / pipeline.BAZEL_LOCKFILE).write_text(
        "internal candidate lockfile\n", encoding="utf-8"
    )
    (repo / "public.txt").write_text(candidate_content, encoding="utf-8")
    for relative_path, content in (additional_candidate_files or {}).items():
        path = repo / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    candidate = commit_all(
        repo,
        "Candidate one",
        author="Candidate Author <candidate@example.com>",
        date=CANDIDATE_AUTHOR_DATE,
    )
    (repo / "public.txt").write_text("candidate two\n", encoding="utf-8")
    later_candidate = commit_all(repo, "Candidate two")
    git(repo, "push", "origin", pipeline.CANDIDATE_BRANCH)
    git(repo, "checkout", "main")
    return RepositoryFixture(
        repo=repo,
        candidate=candidate,
        later_candidate=later_candidate,
        ready_parent=ready_parent,
    )


def create_second_parent_fixture(root: Path) -> tuple[Path, str, str]:
    repo, baseline = create_base_repository(root)
    git(repo, "checkout", "-b", "side", baseline)
    (repo / "side.txt").write_text("side\n", encoding="utf-8")
    second_parent = commit_all(repo, "Side commit")

    git(repo, "checkout", "-b", pipeline.CANDIDATE_BRANCH, baseline)
    (repo / "public.txt").write_text("first parent\n", encoding="utf-8")
    commit_all(repo, "First-parent commit")
    git(repo, "merge", "--no-ff", "side", "-m", "Merge side")
    candidate_head = git_output(repo, "rev-parse", "HEAD")
    git(repo, "push", "origin", pipeline.CANDIDATE_BRANCH)

    git(repo, "checkout", "-b", pipeline.READY_BRANCH, baseline)
    state_file = repo / pipeline.STATE_FILE
    state_file.parent.mkdir(parents=True, exist_ok=True)
    state_file.write_text(f"{second_parent}\n", encoding="utf-8")
    commit_all(repo, "Seed invalid ready state")
    git(repo, "push", "origin", pipeline.READY_BRANCH)
    git(repo, "checkout", "main")
    return repo, second_parent, candidate_head


def create_base_repository(root: Path) -> tuple[Path, str]:
    remote = root / "origin.git"
    subprocess.run(
        ["git", "init", "--bare", remote.as_posix()],
        check=True,
        capture_output=True,
        text=True,
    )
    repo = root / "repo"
    subprocess.run(
        ["git", "init", "--initial-branch=main", repo.as_posix()],
        check=True,
        capture_output=True,
        text=True,
    )
    git(repo, "config", "user.name", "Pipeline Test")
    git(repo, "config", "user.email", "pipeline@example.com")
    git(repo, "remote", "add", "origin", remote.as_posix())
    manifest = repo / pipeline.CARGO_MANIFEST
    manifest.parent.mkdir(parents=True)
    manifest.write_text(
        '[package]\nname = "public-fixture"\nversion = "0.1.0"\nedition = "2024"\n',
        encoding="utf-8",
    )
    source = manifest.parent / "src/lib.rs"
    source.parent.mkdir()
    source.write_text("pub fn fixture() {}\n", encoding="utf-8")
    (repo / pipeline.CARGO_LOCKFILE).write_text(cargo_lockfile(), encoding="utf-8")
    (repo / pipeline.BAZEL_LOCKFILE).write_text(bazel_lockfile(), encoding="utf-8")
    (repo / "public.txt").write_text("baseline\n", encoding="utf-8")
    return repo, commit_all(repo, "Public baseline")


def cargo_lockfile() -> str:
    return (
        "# This file is automatically @generated by Cargo.\n"
        "# It is not intended for manual editing.\n"
        "version = 4\n\n"
        "[[package]]\n"
        'name = "public-fixture"\n'
        'version = "0.1.0"\n'
    )


def bazel_lockfile() -> str:
    return '{"lockFileVersion": 21}\n'


def run_pipeline(
    repo: Path,
    runner_temp: Path,
    command: str,
    *,
    env: dict[str, str] | None = None,
    check: bool = True,
) -> subprocess.CompletedProcess[str]:
    fake_bin = runner_temp / "fake-bin"
    fake_bin.mkdir(exist_ok=True)
    fake_bazel = fake_bin / "bazel"
    fake_bazel.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    fake_bazel.chmod(0o755)
    return subprocess.run(
        [sys.executable, PIPELINE_SCRIPT.as_posix(), command],
        cwd=repo,
        env={
            **os.environ,
            "PATH": f"{fake_bin}:{os.environ['PATH']}",
            "RUNNER_TEMP": runner_temp.as_posix(),
            **(env or {}),
        },
        check=check,
        capture_output=True,
        text=True,
    )


def fetch_ready(repo: Path) -> None:
    git(
        repo,
        "fetch",
        "origin",
        f"+refs/heads/{pipeline.READY_BRANCH}:refs/remotes/origin/{pipeline.READY_BRANCH}",
    )


def git_path_exists(repo: Path, revision: str, path: Path) -> bool:
    return (
        git(repo, "cat-file", "-e", f"{revision}:{path}", check=False).returncode == 0
    )


def commit_all(
    repo: Path,
    message: str,
    *,
    author: str | None = None,
    date: str | None = None,
) -> str:
    git(repo, "add", "--all")
    args = ["commit", "-m", message]
    if author:
        args.extend(["--author", author])
    if date:
        args.extend(["--date", date])
    git(repo, *args)
    return git_output(repo, "rev-parse", "HEAD")


def git_output(repo: Path, *args: str) -> str:
    return git(repo, *args).stdout.strip()


def git(repo: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", *args],
        cwd=repo,
        check=check,
        capture_output=True,
        text=True,
    )


if __name__ == "__main__":
    unittest.main()
