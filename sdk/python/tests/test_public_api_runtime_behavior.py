from __future__ import annotations

import asyncio
from pathlib import Path
from types import SimpleNamespace
from typing import Any
from unittest.mock import AsyncMock, Mock

import pytest

import openai_codex.api as public_api_module
from openai_codex.api import (
    ApprovalMode,
    AsyncCodex,
    Codex,
    ExternalMessage,
    Sandbox,
    TextInput,
)
from openai_codex.client import _params_dict
from openai_codex.generated.v2_all import TurnCompletedNotification, TurnStartParams
from openai_codex.models import InitializeResponse, Notification

ROOT = Path(__file__).resolve().parents[1]


def _approval_settings(params: list[Any]) -> list[dict[str, object]]:
    """Return serialized approval settings from captured Pydantic params."""
    return [
        {
            key: value
            for key, value in param.model_dump(
                by_alias=True,
                exclude_none=True,
                mode="json",
            ).items()
            if key in {"approvalPolicy", "approvalsReviewer"}
        }
        for param in params
    ]


def test_codex_init_failure_closes_client(monkeypatch: pytest.MonkeyPatch) -> None:
    closed: list[bool] = []

    class FakeClient:
        def __init__(self, config=None) -> None:  # noqa: ANN001,ARG002
            self._closed = False

        def start(self) -> None:
            return None

        def initialize(self) -> InitializeResponse:
            return InitializeResponse.model_validate({})

        def close(self) -> None:
            self._closed = True
            closed.append(True)

    monkeypatch.setattr(public_api_module, "CodexClient", FakeClient)

    with pytest.raises(RuntimeError, match="missing required metadata"):
        Codex()

    assert closed == [True]


def test_async_codex_init_failure_closes_client() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()
        close_calls = 0

        async def fake_start() -> None:
            return None

        async def fake_initialize() -> InitializeResponse:
            return InitializeResponse.model_validate({})

        async def fake_close() -> None:
            nonlocal close_calls
            close_calls += 1

        codex._client.start = fake_start  # type: ignore[method-assign]
        codex._client.initialize = fake_initialize  # type: ignore[method-assign]
        codex._client.close = fake_close  # type: ignore[method-assign]

        with pytest.raises(RuntimeError, match="missing required metadata"):
            await codex.models()

        assert close_calls == 1
        assert codex._initialized is False
        assert codex._init is None

    asyncio.run(scenario())


def test_async_codex_initializes_only_once_under_concurrency() -> None:
    async def scenario() -> None:
        codex = AsyncCodex()
        start_calls = 0
        initialize_calls = 0
        ready = asyncio.Event()

        async def fake_start() -> None:
            nonlocal start_calls
            start_calls += 1

        async def fake_initialize() -> InitializeResponse:
            nonlocal initialize_calls
            initialize_calls += 1
            ready.set()
            await asyncio.sleep(0.02)
            return InitializeResponse.model_validate(
                {
                    "userAgent": "codex-cli/1.2.3",
                    "serverInfo": {"name": "codex-cli", "version": "1.2.3"},
                }
            )

        async def fake_model_list(include_hidden: bool = False):  # noqa: ANN202,ARG001
            await ready.wait()
            return object()

        codex._client.start = fake_start  # type: ignore[method-assign]
        codex._client.initialize = fake_initialize  # type: ignore[method-assign]
        codex._client.model_list = fake_model_list  # type: ignore[method-assign]

        await asyncio.gather(codex.models(), codex.models())

        assert start_calls == 1
        assert initialize_calls == 1

    asyncio.run(scenario())


@pytest.mark.parametrize("api_type", [Codex, AsyncCodex])
@pytest.mark.parametrize(
    ("options", "expected"),
    [
        ({}, {}),
        ({"include_turns": None}, {}),
        ({"include_turns": True}, {"excludeTurns": False}),
        ({"include_turns": False}, {"excludeTurns": True}),
    ],
)
def test_include_turns_preserves_omission_and_inverts_explicit_values(
    api_type, options, expected
) -> None:
    async def scenario() -> None:
        async_api = api_type is AsyncCodex
        rpc = AsyncMock if async_api else Mock
        thread_response = SimpleNamespace(thread=SimpleNamespace(id="thread-2"))
        client = SimpleNamespace(
            thread_resume=rpc(return_value=thread_response),
            thread_fork=rpc(return_value=thread_response),
        )
        codex = api_type.__new__(api_type)
        codex._client = client
        codex._initialized = True

        for method in ("thread_resume", "thread_fork"):
            thread = getattr(codex, method)("thread-1", **options)
            if async_api:
                thread = await thread
            assert thread.id == "thread-2"
            assert _params_dict(getattr(client, method).call_args.args[1]) == {
                "threadId": "thread-1",
                **expected,
            }

    asyncio.run(scenario())


@pytest.mark.parametrize("api_type", [Codex, AsyncCodex])
@pytest.mark.parametrize("method", ["run", "turn"])
@pytest.mark.parametrize(
    "content", [None, "External update", [{"type": "input_text", "text": "External update"}]]
)
def test_turn_inputs_and_options_reach_the_client(api_type, method, content) -> None:
    """User and external inputs preserve distinct wire representations for every entry point."""

    async def scenario() -> None:
        async_api = api_type is AsyncCodex
        rpc = AsyncMock if async_api else Mock
        completed = Notification(
            method="turn/completed",
            payload=TurnCompletedNotification.model_validate(
                {
                    "threadId": "thread-1",
                    "turn": {"id": "turn-1", "items": [], "status": "completed"},
                }
            ),
        )
        subscription = SimpleNamespace(next=Mock(return_value=completed), close=Mock())
        client = SimpleNamespace(
            _start_turn=rpc(
                return_value=(SimpleNamespace(turn=SimpleNamespace(id="turn-1")), subscription)
            ),
        )
        codex = api_type.__new__(api_type)
        codex._client = client
        codex._initialized = True
        thread = (
            public_api_module.AsyncThread(codex, "thread-1")
            if async_api
            else public_api_module.Thread(client, "thread-1")
        )
        input = (
            "Continue."
            if content is None
            else ExternalMessage(tool_name="notifications", namespace="slack", content=content)
        )
        turn = getattr(thread, method)(
            input,
            service_tier="priority",
            turn_service_tier="default",
            source="automation",
        )
        if async_api:
            turn = await turn
        assert turn.id == "turn-1"
        expected_input = (
            [{"type": "text", "text": "Continue.", "text_elements": []}] if content is None else []
        )
        expected_tool_output = (
            {}
            if content is None
            else {
                "toolOutput": {"name": "notifications", "namespace": "slack", "output": content},
            }
        )
        assert _params_dict(client._start_turn.call_args.kwargs["params"]) == {
            "threadId": "thread-1",
            "input": expected_input,
            "serviceTier": "priority",
            "serviceTierForTurn": "default",
            "turnTrigger": "automation",
            **expected_tool_output,
        }

    asyncio.run(scenario())


@pytest.mark.parametrize("api_type", [Codex, AsyncCodex])
def test_external_messages_cannot_be_mixed_with_user_input_or_sent_as_user_steering(
    api_type,
) -> None:
    async def scenario() -> None:
        async_api = api_type is AsyncCodex
        rpc = AsyncMock if async_api else Mock
        client = SimpleNamespace(
            _start_turn=rpc(), turn_steer=rpc(), _subscribe_turn_notifications=Mock()
        )
        codex = api_type.__new__(api_type)
        codex._client = client
        codex._initialized = True
        thread = (
            public_api_module.AsyncThread(codex, "thread-1")
            if async_api
            else public_api_module.Thread(client, "thread-1")
        )
        handle = (
            public_api_module.AsyncTurnHandle(codex, "thread-1", "turn-1")
            if async_api
            else public_api_module.TurnHandle(client, "thread-1", "turn-1")
        )
        external = ExternalMessage(tool_name="notifications", content="Untrusted update")
        for operation, input in (
            (thread.turn, [TextInput("User request"), external]),
            (handle.steer, external),
        ):
            with pytest.raises(TypeError):
                result = operation(input)
                if async_api:
                    await result
        with pytest.raises(ValueError, match="tool_name"):
            result = thread.turn(ExternalMessage(tool_name="  ", content="Untrusted update"))
            if async_api:
                await result
        client._start_turn.assert_not_called()
        client.turn_steer.assert_not_called()

    asyncio.run(scenario())


def _approval_mode_turn_params(approval_mode: ApprovalMode) -> TurnStartParams:
    """Build real generated turn params from one public approval mode."""
    approval_policy, approvals_reviewer = public_api_module._approval_mode_settings(approval_mode)
    return TurnStartParams(
        thread_id="thread-1",
        input=[],
        approval_policy=approval_policy,
        approvals_reviewer=approvals_reviewer,
    )


def test_approval_modes_serialize_to_expected_start_params() -> None:
    """ApprovalMode should map to the app-server params sent for new work."""
    assert {
        mode.value: _approval_settings([_approval_mode_turn_params(mode)])[0]
        for mode in ApprovalMode
    } == {
        "deny_all": {"approvalPolicy": "never"},
        "auto_review": {
            "approvalPolicy": "on-request",
            "approvalsReviewer": "auto_review",
        },
    }


def test_unknown_approval_mode_is_rejected() -> None:
    """Invalid approval modes should fail before params are constructed."""
    with pytest.raises(ValueError, match="deny_all, auto_review"):
        public_api_module._approval_mode_settings("allow_all")  # type: ignore[arg-type]


def test_sandbox_presets_serialize_for_threads_and_turns() -> None:
    """One public sandbox enum should map to both stable wire representations."""
    assert {
        sandbox.name: public_api_module._sandbox_mode(sandbox).value for sandbox in Sandbox
    } == {
        "read_only": "read-only",
        "workspace_write": "workspace-write",
        "full_access": "danger-full-access",
    }
    assert {
        sandbox.name: public_api_module._sandbox_policy(sandbox).model_dump(
            by_alias=True,
            mode="json",
        )
        for sandbox in Sandbox
    } == {
        "read_only": {"networkAccess": False, "type": "readOnly"},
        "workspace_write": {
            "excludeSlashTmp": False,
            "excludeTmpdirEnvVar": False,
            "networkAccess": False,
            "type": "workspaceWrite",
            "writableRoots": [],
        },
        "full_access": {"type": "dangerFullAccess"},
    }


def test_raw_sandbox_strings_are_rejected() -> None:
    """Callers should use the discoverable enum rather than memorizing values."""
    with pytest.raises(ValueError, match="Sandbox\\.workspace_write"):
        public_api_module._sandbox_mode("workspace")  # type: ignore[arg-type]


def test_retry_examples_compare_status_with_enum() -> None:
    for path in (
        ROOT / "examples" / "10_error_handling_and_retry" / "sync.py",
        ROOT / "examples" / "10_error_handling_and_retry" / "async.py",
    ):
        source = path.read_text()
        assert '== "failed"' not in source
        assert "TurnStatus.failed" in source
