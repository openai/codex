from __future__ import annotations

import asyncio

from app_server_harness import AppServerHarness
from openai_codex import ApprovalMode, AsyncCodex, Codex
from openai_codex.generated.v2_all import AskForApprovalValue, ThreadResumeParams
from app_server_helpers import response_approval_policy


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
