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
            prepare_repo = shallow_clone(fixture.repo, root / "prepare-repo")

            run_pipeline(prepare_repo, runner_temp, "prepare")

            metadata = (runner_temp / "publish-metadata.json").read_text(
                encoding="utf-8"
            )
            self.assertIn(fixture.candidate, metadata)
            self.assertNotIn(fixture.later_candidate, metadata)
            (runner_temp / "public-commit-message.md").write_text(
                "Describe the public change\n\nPublic details.\n", encoding="utf-8"
            )
            publish_repo = shallow_clone(fixture.repo, root / "publish-repo")
            run_pipeline(publish_repo, runner_temp, "publish")

            fetch_ready(fixture.repo)
            ready = git_output(
                fixture.repo, "rev-parse", f"origin/{pipeline.READY_BRANCH}"
            )
            self.assertEqual(
                git_output(fixture.repo, "rev-parse", f"{ready}^"),
                fixture.ready_parent,
            )
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:public/projected.txt"),
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
            self.assertEqual(
                git_output(fixture.repo, "show", f"{ready}:MODULE.bazel"),
                'module(name = "candidate")',
            )
            self.assertEqual(
                git_output(
                    fixture.repo,
                    "show",
                    f"{ready}:public/.vscode/settings.json",
                ),
                '{"fixture": true}',
            )
            self.assertFalse(git_path_exists(fixture.repo, ready, pipeline.GITHUB_DIR))
            self.assertTrue(
                git_path_exists(
                    fixture.repo,
                    ready,
                    Path("public/.github/workflows/public-ci.yml"),
                )
            )
            self.assertEqual(
                git_output(
                    fixture.repo,
                    "diff",
                    "--name-only",
                    fixture.ready_parent,
                    ready,
                    "--",
                    pipeline.GITHUB_DIR.as_posix(),
                ),
                "",
            )
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

    def test_state_only_candidate_uses_automatic_message(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(
                root,
                candidate_content="baseline\n",
                bazel_module_content='module(name = "baseline")\n',
                public_workflow_content="name: Public CI\n",
            )
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()
            github_output = runner_temp / "github-output"

            run_pipeline(
                fixture.repo,
                runner_temp,
                "prepare",
                env={"GITHUB_OUTPUT": github_output.as_posix()},
            )

            self.assertFalse((runner_temp / "message-input").exists())
            self.assertEqual(
                (runner_temp / "public-commit-message.md").read_text(
                    encoding="utf-8"
                ),
                f"{pipeline.STATE_ONLY_MESSAGE_SUBJECT}\n",
            )
            self.assertEqual(
                github_output.read_text(encoding="utf-8"),
                "has_change=true\n"
                "message_override=false\n"
                "automatic_message=true\n",
            )

            run_pipeline(fixture.repo, runner_temp, "validate-message")
            run_pipeline(fixture.repo, runner_temp, "publish")

            fetch_ready(fixture.repo)
            ready = git_output(
                fixture.repo, "rev-parse", f"origin/{pipeline.READY_BRANCH}"
            )
            self.assertEqual(
                git_output(
                    fixture.repo,
                    "diff",
                    "--name-only",
                    fixture.ready_parent,
                    ready,
                ),
                pipeline.STATE_FILE.as_posix(),
            )
            self.assertEqual(
                git_output(fixture.repo, "show", "--no-patch", "--format=%s", ready),
                pipeline.STATE_ONLY_MESSAGE_SUBJECT,
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

            result = run_pipeline(
                fixture.repo, runner_temp, "publish", check=False
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("advanced from", result.stderr)

    def test_publish_rejects_root_github_in_ready_parent(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(root)
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            git(fixture.repo, "checkout", pipeline.READY_BRANCH)
            workflow = fixture.repo / ".github/workflows/unexpected.yml"
            workflow.parent.mkdir(parents=True)
            workflow.write_text("name: Unexpected\n", encoding="utf-8")
            commit_all(fixture.repo, "Corrupt ready tree")
            git(fixture.repo, "push", "origin", pipeline.READY_BRANCH)

            run_pipeline(fixture.repo, runner_temp, "prepare")
            (runner_temp / "public-commit-message.md").write_text(
                "Describe the public change\n", encoding="utf-8"
            )
            result = run_pipeline(fixture.repo, runner_temp, "publish", check=False)

            self.assertNotEqual(result.returncode, 0)
            self.assertIn(
                "ready branch unexpectedly contains a root .github directory",
                result.stderr,
            )

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
                additional_candidate_files={
                    "public/nested/[second].txt": "second file\n",
                    "private.txt": "not exported\n",
                },
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
                git_output(workspace, "show", f"{target}^:projected.txt"),
                "baseline",
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:projected.txt"),
                "x" * 20_000,
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:nested/[second].txt"),
                "second file",
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}^:.vscode/settings.json"),
                '{"fixture": true}',
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:.vscode/settings.json"),
                '{"fixture": true}',
            )
            self.assertEqual(
                git_output(
                    workspace,
                    "show",
                    f"{target}:.github/workflows/public-ci.yml",
                ),
                "name: Projected public CI",
            )
            self.assertFalse(git_path_exists(workspace, target, pipeline.PUBLIC_DIR))
            self.assertFalse(
                git_path_exists(workspace, target, Path("private.txt"))
            )
            self.assertFalse(git_path_exists(workspace, target, pipeline.STATE_FILE))
            self.assertEqual(
                git_output(workspace, "show", f"{target}:{pipeline.BAZEL_LOCKFILE}"),
                bazel_lockfile().strip(),
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}^:{pipeline.BAZEL_MODULE}"),
                'module(name = "baseline")',
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:{pipeline.BAZEL_MODULE}"),
                'module(name = "candidate")',
            )
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

    def test_prepare_includes_license_only_change_in_message_workspace(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            fixture = create_repository_fixture(
                root,
                candidate_content=None,
                bazel_module_content=None,
                public_workflow_content=None,
                additional_candidate_files={"LICENSE": "candidate license\n"},
            )
            runner_temp = root / "runner-temp"
            runner_temp.mkdir()

            run_pipeline(fixture.repo, runner_temp, "prepare")

            message_input = runner_temp / "message-input"
            target = (
                (message_input / pipeline.MESSAGE_TARGET_FILE)
                .read_text(encoding="utf-8")
                .strip()
            )
            workspace = root / "message-workspace"
            subprocess.run(
                ["git", "init", "--initial-branch=main", workspace.as_posix()],
                check=True,
                capture_output=True,
                text=True,
            )
            git(
                workspace,
                "fetch",
                (message_input / pipeline.MESSAGE_WORKSPACE_BUNDLE).as_posix(),
                f"refs/heads/{pipeline.MESSAGE_WORKSPACE_BRANCH}:"
                f"refs/heads/{pipeline.MESSAGE_WORKSPACE_BRANCH}",
            )
            git(workspace, "checkout", pipeline.MESSAGE_WORKSPACE_BRANCH)

            self.assertEqual(
                git_output(workspace, "diff", "--name-only", f"{target}^", target),
                pipeline.LICENSE_FILE.as_posix(),
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}^:{pipeline.LICENSE_FILE}"),
                "baseline license",
            )
            self.assertEqual(
                git_output(workspace, "show", f"{target}:{pipeline.LICENSE_FILE}"),
                "candidate license",
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
                "has_change=true\n"
                "message_override=true\n"
                "automatic_message=false\n",
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
    def test_restores_cargo_lock_from_source_and_bazel_lock_from_ready(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            candidate_tree = Path(temp_dir)
            (candidate_tree / pipeline.CARGO_LOCKFILE).parent.mkdir(parents=True)

            with patch.object(
                pipeline,
                "output",
                side_effect=["source cargo lock", "ready bazel lock"],
            ) as output:
                pipeline.restore_lockfile_baselines(
                    "source-revision", "ready-revision", candidate_tree
                )

            self.assertEqual(
                (candidate_tree / pipeline.CARGO_LOCKFILE).read_text(
                    encoding="utf-8"
                ),
                "source cargo lock\n",
            )
            self.assertEqual(
                (candidate_tree / pipeline.BAZEL_LOCKFILE).read_text(
                    encoding="utf-8"
                ),
                "ready bazel lock\n",
            )
            self.assertEqual(
                output.call_args_list,
                [
                    call(
                        [
                            "git",
                            "show",
                            "source-revision:codex-rs/Cargo.lock",
                        ]
                    ),
                    call(
                        [
                            "git",
                            "show",
                            "ready-revision:MODULE.bazel.lock",
                        ]
                    ),
                ],
            )

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


class PipelineProjectionTest(unittest.TestCase):
    def test_public_overlay_merges_with_and_overrides_verbatim_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temp_dir:
            root = Path(temp_dir)
            transport = root / "transport"
            (transport / "codex-rs").mkdir(parents=True)
            (transport / "codex-rs/retained.rs").write_text(
                "retained\n", encoding="utf-8"
            )
            (transport / "codex-rs/overridden.rs").write_text(
                "verbatim\n", encoding="utf-8"
            )
            (transport / pipeline.BAZEL_MODULE).write_text(
                "verbatim module\n", encoding="utf-8"
            )
            (transport / "public/codex-rs").mkdir(parents=True)
            (transport / "public/codex-rs/overridden.rs").write_text(
                "overlay\n", encoding="utf-8"
            )
            (transport / "public/codex-rs/added.rs").write_text(
                "added\n", encoding="utf-8"
            )
            (transport / "public" / pipeline.BAZEL_MODULE).write_text(
                "overlay module\n", encoding="utf-8"
            )

            effective_public = root / "effective-public"
            pipeline.project_effective_public_tree(transport, effective_public)

            self.assertEqual(
                {
                    path.relative_to(effective_public).as_posix(): path.read_text(
                        encoding="utf-8"
                    )
                    for path in effective_public.rglob("*")
                    if path.is_file()
                },
                {
                    "MODULE.bazel": "overlay module\n",
                    "codex-rs/added.rs": "added\n",
                    "codex-rs/overridden.rs": "overlay\n",
                    "codex-rs/retained.rs": "retained\n",
                },
            )


def create_repository_fixture(
    root: Path,
    *,
    candidate_content: str | None = "candidate one\n",
    bazel_module_content: str | None = 'module(name = "candidate")\n',
    public_workflow_content: str | None = "name: Projected public CI\n",
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
    public_github_file = repo / "public/.github/workflows/public-ci.yml"
    public_github_file.parent.mkdir(parents=True, exist_ok=True)
    if public_workflow_content is not None:
        public_github_file.write_text(public_workflow_content, encoding="utf-8")
    (repo / pipeline.BAZEL_LOCKFILE).write_text(
        "internal candidate lockfile\n", encoding="utf-8"
    )
    if bazel_module_content is not None:
        (repo / pipeline.BAZEL_MODULE).write_text(
            bazel_module_content, encoding="utf-8"
        )
    if candidate_content is not None:
        (repo / "public/projected.txt").write_text(
            candidate_content, encoding="utf-8"
        )
    for relative_path, content in (additional_candidate_files or {}).items():
        path = repo / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")
    candidate = commit_all(
        repo,
        f"Candidate one\n\nGitOrigin-RevId: {baseline}",
        author="Candidate Author <candidate@example.com>",
        date=CANDIDATE_AUTHOR_DATE,
    )
    (repo / "public/projected.txt").write_text(
        "candidate two\n", encoding="utf-8"
    )
    later_candidate = commit_all(
        repo, f"Candidate two\n\nGitOrigin-RevId: {baseline}"
    )
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
    (repo / pipeline.BAZEL_MODULE).write_text(
        'module(name = "baseline")\n', encoding="utf-8"
    )
    (repo / pipeline.BAZEL_LOCKFILE).write_text(bazel_lockfile(), encoding="utf-8")
    (repo / pipeline.LICENSE_FILE).write_text("baseline license\n", encoding="utf-8")
    public_file = repo / "public/projected.txt"
    public_file.parent.mkdir()
    public_file.write_text("baseline\n", encoding="utf-8")
    public_workflow = repo / "public/.github/workflows/public-ci.yml"
    public_workflow.parent.mkdir(parents=True)
    public_workflow.write_text("name: Public CI\n", encoding="utf-8")
    public_gitignore = repo / "public/.gitignore"
    public_gitignore.write_text(".vscode/\n", encoding="utf-8")
    public_editor_settings = repo / "public/.vscode/settings.json"
    public_editor_settings.parent.mkdir(parents=True)
    public_editor_settings.write_text('{"fixture": true}\n', encoding="utf-8")
    git(repo, "add", "--force", "public/.vscode/settings.json")
    baseline = commit_all(repo, "Public baseline")
    git(repo, "push", "-u", "origin", "main")
    return repo, baseline


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


def shallow_clone(source_repo: Path, destination: Path) -> Path:
    remote = Path(git_output(source_repo, "remote", "get-url", "origin")).resolve()
    subprocess.run(
        [
            "git",
            "clone",
            "--branch",
            pipeline.CANDIDATE_BRANCH,
            "--depth=1",
            remote.as_uri(),
            destination.as_posix(),
        ],
        check=True,
        capture_output=True,
        text=True,
    )
    is_shallow = git_output(destination, "rev-parse", "--is-shallow-repository")
    if is_shallow != "true":
        raise RuntimeError(f"expected a shallow test clone, got {is_shallow}")
    return destination


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
