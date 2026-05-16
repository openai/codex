from __future__ import annotations

import asyncio
import json
import shlex

import pytest

from app_server_harness import (
    AppServerHarness,
    ev_completed,
    ev_function_call,
    ev_response_created,
    sse,
)
from app_server_helpers import response_approval_policy, response_sandbox_type

from openai_codex import ApprovalMode, AsyncCodex, Codex
from openai_codex.generated.v2_all import (
    AskForApprovalValue,
    DangerFullAccessSandboxPolicy,
    ReadOnlySandboxPolicy,
    SandboxMode,
    SandboxPolicy,
    ThreadResumeParams,
)

DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE = DangerFullAccessSandboxPolicy(type="dangerFullAccess").type


def test_thread_resume_inherits_deny_all_approval_mode(tmp_path) -> None:
    """Resuming a thread should preserve its stored approval mode."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("source seeded", response_id="resume-mode")

        with Codex(config=harness.app_server_config()) as codex:
            source = codex.thread_start(approval_mode=ApprovalMode.deny_all)
            result = source.run("seed the source rollout")
            resumed = codex.thread_resume(source.id)
            resumed_state = codex._client.thread_resume(  # noqa: SLF001
                resumed.id,
                ThreadResumeParams(thread_id=resumed.id),
            )

    assert {
        "final_response": result.final_response,
        "resumed_policy": response_approval_policy(resumed_state),
    } == {
        "final_response": "source seeded",
        "resumed_policy": AskForApprovalValue.never.value,
    }


def test_thread_fork_inherits_deny_all_approval_mode(tmp_path) -> None:
    """Forking without an override should preserve the source approval mode."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("source seeded", response_id="fork-mode")

        with Codex(config=harness.app_server_config()) as codex:
            source = codex.thread_start(approval_mode=ApprovalMode.deny_all)
            result = source.run("seed the source rollout")
            forked = codex.thread_fork(source.id)
            forked_state = codex._client.thread_resume(  # noqa: SLF001
                forked.id,
                ThreadResumeParams(thread_id=forked.id),
            )

    assert {
        "final_response": result.final_response,
        "forked_is_distinct": forked.id != source.id,
        "forked_policy": response_approval_policy(forked_state),
    } == {
        "final_response": "source seeded",
        "forked_is_distinct": True,
        "forked_policy": AskForApprovalValue.never.value,
    }


def test_thread_fork_can_override_approval_mode(tmp_path) -> None:
    """Forking with an explicit approval mode should send an override."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "source seeded",
            response_id="fork-override-mode",
        )

        with Codex(config=harness.app_server_config()) as codex:
            source = codex.thread_start(approval_mode=ApprovalMode.deny_all)
            result = source.run("seed the source rollout")
            forked = codex.thread_fork(
                source.id,
                approval_mode=ApprovalMode.auto_review,
            )
            forked_state = codex._client.thread_resume(  # noqa: SLF001
                forked.id,
                ThreadResumeParams(thread_id=forked.id),
            )

    assert {
        "final_response": result.final_response,
        "forked_policy": response_approval_policy(forked_state),
    } == {
        "final_response": "source seeded",
        "forked_policy": AskForApprovalValue.on_request.value,
    }


def test_dangerous_bypass_thread_lifecycle_persists_thread_settings(
    tmp_path,
) -> None:
    """Thread lifecycle operations should preserve the explicit bypass preset."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "bypass seeded",
            response_id="bypass-thread",
        )

        with Codex(config=harness.app_server_config()) as codex:
            source = codex.thread_start(
                approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
            )
            result = source.run("seed the bypass thread")
            started_state = codex._client.thread_resume(  # noqa: SLF001
                source.id,
                ThreadResumeParams(thread_id=source.id),
            )
            resumed = codex.thread_resume(source.id)
            resumed_state = codex._client.thread_resume(  # noqa: SLF001
                resumed.id,
                ThreadResumeParams(thread_id=resumed.id),
            )
            forked = codex.thread_fork(source.id)
            forked_state = codex._client.thread_resume(  # noqa: SLF001
                forked.id,
                ThreadResumeParams(thread_id=forked.id),
            )

    assert {
        "final_response": result.final_response,
        "forked_is_distinct": forked.id != source.id,
        "started": (
            response_approval_policy(started_state),
            response_sandbox_type(started_state),
        ),
        "resumed": (
            response_approval_policy(resumed_state),
            response_sandbox_type(resumed_state),
        ),
        "forked": (
            response_approval_policy(forked_state),
            response_sandbox_type(forked_state),
        ),
    } == {
        "final_response": "bypass seeded",
        "forked_is_distinct": True,
        "started": (
            AskForApprovalValue.never.value,
            DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE,
        ),
        "resumed": (
            AskForApprovalValue.never.value,
            DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE,
        ),
        "forked": (
            AskForApprovalValue.never.value,
            DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE,
        ),
    }


def test_turn_dangerous_bypass_persists_thread_settings(tmp_path) -> None:
    """Turn-level bypass should persist approvals disabled and sandbox bypassed."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message(
            "turn bypass",
            response_id="bypass-turn",
        )

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start(approval_mode=ApprovalMode.auto_review)
            result = thread.run(
                "bypass this turn",
                approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
            )
            after_turn = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )

    assert {
        "final_response": result.final_response,
        "thread_settings": (
            response_approval_policy(after_turn),
            response_sandbox_type(after_turn),
        ),
    } == {
        "final_response": "turn bypass",
        "thread_settings": (
            AskForApprovalValue.never.value,
            DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE,
        ),
    }


def test_async_turn_dangerous_bypass_persists_thread_settings(tmp_path) -> None:
    """Async turn-level bypass should persist the same app-server settings."""

    async def scenario() -> None:
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "async turn bypass",
                response_id="async-bypass-turn",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start(approval_mode=ApprovalMode.auto_review)
                result = await thread.run(
                    "bypass this async turn",
                    approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
                )
                after_turn = await codex._client.thread_resume(  # noqa: SLF001
                    thread.id,
                    ThreadResumeParams(thread_id=thread.id),
                )

        assert {
            "final_response": result.final_response,
            "thread_settings": (
                response_approval_policy(after_turn),
                response_sandbox_type(after_turn),
            ),
        } == {
            "final_response": "async turn bypass",
            "thread_settings": (
                AskForApprovalValue.never.value,
                DANGER_FULL_ACCESS_SANDBOX_POLICY_TYPE,
            ),
        }

    asyncio.run(scenario())


def test_outside_workspace_write_rejected_for_deny_all_and_allowed_for_bypass(
    tmp_path,
) -> None:
    """Dangerous bypass should be the mode that permits outside-workspace writes."""
    rejected_path = tmp_path / "deny-all-outside-write.txt"
    allowed_path = tmp_path / "dangerous-outside-write.txt"

    with AppServerHarness(tmp_path) as harness:
        rejected_args = json.dumps(
            {
                "command": (
                    f"printf %s rejected > {shlex.quote(str(rejected_path))}"
                ),
                "login": False,
                "timeout_ms": 1_000,
            }
        )
        dangerous_args = json.dumps(
            {
                "command": (
                    f"printf %s dangerous > {shlex.quote(str(allowed_path))}"
                ),
                "login": False,
                "timeout_ms": 1_000,
            }
        )
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("deny-all-write"),
                    ev_function_call(
                        "deny-all-outside-write",
                        "shell_command",
                        rejected_args,
                    ),
                    ev_completed("deny-all-write"),
                ]
            )
        )
        harness.responses.enqueue_assistant_message(
            "deny-all shell completed",
            response_id="deny-all-final",
        )
        harness.responses.enqueue_sse(
            sse(
                [
                    ev_response_created("dangerous-write"),
                    ev_function_call(
                        "dangerous-outside-write",
                        "shell_command",
                        dangerous_args,
                    ),
                    ev_completed("dangerous-write"),
                ]
            )
        )
        harness.responses.enqueue_assistant_message(
            "dangerous shell completed",
            response_id="dangerous-final",
        )

        with Codex(config=harness.app_server_config()) as codex:
            denied_thread = codex.thread_start(approval_mode=ApprovalMode.deny_all)
            denied_result = denied_thread.run("write outside the workspace")

            bypass_thread = codex.thread_start(
                approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
            )
            bypass_result = bypass_thread.run("write outside the workspace")

    assert {
        "denied_final_response": denied_result.final_response,
        "denied_path_exists": rejected_path.exists(),
        "bypass_final_response": bypass_result.final_response,
        "bypass_file_contents": allowed_path.read_text(),
    } == {
        "denied_final_response": "deny-all shell completed",
        "denied_path_exists": False,
        "bypass_final_response": "dangerous shell completed",
        "bypass_file_contents": "dangerous",
    }


def test_dangerous_bypass_rejects_explicit_sandbox_conflicts_before_state_changes(
    tmp_path,
) -> None:
    """Conflicting bypass presets should fail before mutating app-server state."""
    with AppServerHarness(tmp_path) as harness:
        with Codex(config=harness.app_server_config()) as codex:
            with pytest.raises(ValueError, match="combined with sandbox"):
                codex.thread_start(
                    approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
                    sandbox=SandboxMode.read_only,
                )

            threads_after_invalid_start = codex.thread_list(archived=False)
            thread = codex.thread_start()

            with pytest.raises(ValueError, match="combined with sandbox_policy"):
                thread.run(
                    "this should never reach app-server",
                    approval_mode=ApprovalMode.dangerously_bypass_approvals_and_sandbox,
                    sandbox_policy=SandboxPolicy(root=ReadOnlySandboxPolicy(type="readOnly")),
                )

            thread_state = thread.read(include_turns=True)

    assert {
        "threads_after_invalid_start": [
            existing.id for existing in threads_after_invalid_start.data
        ],
        "turns_after_invalid_run": thread_state.thread.turns,
    } == {
        "threads_after_invalid_start": [],
        "turns_after_invalid_run": [],
    }


def test_turn_approval_mode_persists_until_next_turn(tmp_path) -> None:
    """A turn-level approval override should apply to later omitted-arg turns."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("turn override", response_id="turn-mode-1")
        harness.responses.enqueue_assistant_message("turn inherited", response_id="turn-mode-2")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start()
            first_result = thread.run(
                "deny this and later turns",
                approval_mode=ApprovalMode.deny_all,
            )
            after_turn_override = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )
            second_result = thread.run("inherit previous approval mode")
            after_omitted_turn = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )

    assert {
        "after_turn_override": response_approval_policy(after_turn_override),
        "after_omitted_turn": response_approval_policy(after_omitted_turn),
        "final_responses": [
            first_result.final_response,
            second_result.final_response,
        ],
    } == {
        "after_turn_override": AskForApprovalValue.never.value,
        "after_omitted_turn": AskForApprovalValue.never.value,
        "final_responses": ["turn override", "turn inherited"],
    }


def test_thread_run_approval_mode_persists_until_explicit_override(tmp_path) -> None:
    """Omitted run approval mode should not rewrite the thread's stored setting."""
    with AppServerHarness(tmp_path) as harness:
        harness.responses.enqueue_assistant_message("locked down", response_id="approval-1")
        harness.responses.enqueue_assistant_message("reviewable", response_id="approval-2")

        with Codex(config=harness.app_server_config()) as codex:
            thread = codex.thread_start(approval_mode=ApprovalMode.deny_all)

            first_result = thread.run("keep approvals denied")
            after_default_run = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )
            second_result = thread.run(
                "allow auto review now",
                approval_mode=ApprovalMode.auto_review,
            )
            after_override_run = codex._client.thread_resume(  # noqa: SLF001
                thread.id,
                ThreadResumeParams(thread_id=thread.id),
            )

    assert {
        "after_default_policy": response_approval_policy(after_default_run),
        "after_override_policy": response_approval_policy(after_override_run),
        "final_responses": [
            first_result.final_response,
            second_result.final_response,
        ],
    } == {
        "after_default_policy": AskForApprovalValue.never.value,
        "after_override_policy": AskForApprovalValue.on_request.value,
        "final_responses": ["locked down", "reviewable"],
    }


def test_async_thread_run_approval_mode_persists_until_explicit_override(
    tmp_path,
) -> None:
    """Async omitted run approval mode should leave stored settings alone."""

    async def scenario() -> None:
        """Use the async client to verify persisted app-server approval state."""
        with AppServerHarness(tmp_path) as harness:
            harness.responses.enqueue_assistant_message(
                "async locked down",
                response_id="async-approval-1",
            )
            harness.responses.enqueue_assistant_message(
                "async reviewable",
                response_id="async-approval-2",
            )

            async with AsyncCodex(config=harness.app_server_config()) as codex:
                thread = await codex.thread_start(approval_mode=ApprovalMode.deny_all)
                first_result = await thread.run("keep async approvals denied")
                after_default_run = await codex._client.thread_resume(  # noqa: SLF001
                    thread.id,
                    ThreadResumeParams(thread_id=thread.id),
                )
                second_result = await thread.run(
                    "allow async auto review now",
                    approval_mode=ApprovalMode.auto_review,
                )
                after_override_run = await codex._client.thread_resume(  # noqa: SLF001
                    thread.id,
                    ThreadResumeParams(thread_id=thread.id),
                )

        assert {
            "after_default_policy": response_approval_policy(after_default_run),
            "after_override_policy": response_approval_policy(after_override_run),
            "final_responses": [
                first_result.final_response,
                second_result.final_response,
            ],
        } == {
            "after_default_policy": AskForApprovalValue.never.value,
            "after_override_policy": AskForApprovalValue.on_request.value,
            "final_responses": ["async locked down", "async reviewable"],
        }

    asyncio.run(scenario())
