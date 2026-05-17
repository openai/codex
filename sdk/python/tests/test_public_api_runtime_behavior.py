from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any

import pytest

import openai_codex.api as public_api_module
from openai_codex.api import (
    ApprovalMode,
    AsyncCodex,
    Codex,
)
from openai_codex.generated.v2_all import TurnStartParams
from openai_codex.models import InitializeResponse

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

    monkeypatch.setattr(public_api_module, "AppServerClient", FakeClient)

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


def test_retry_examples_compare_status_with_enum() -> None:
    for path in (
        ROOT / "examples" / "10_error_handling_and_retry" / "sync.py",
        ROOT / "examples" / "10_error_handling_and_retry" / "async.py",
    ):
        source = path.read_text()
        assert '== "failed"' not in source
        assert "TurnStatus.failed" in source
