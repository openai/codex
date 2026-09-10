from __future__ import annotations

import json
from pathlib import Path
from typing import get_type_hints

import pytest

from openai_codex._runtime_requirements import CheckoutCapabilities
from openai_codex.client import CodexClient, _params_dict
from openai_codex.errors import CodexError
from openai_codex.generated.notification_registry import notification_turn_id
from openai_codex.generated.v2_all import (
    AbsolutePathBuf,
    AccountRateLimitsUpdatedNotification,
    AccountUpdatedNotification,
    AgentMessageDeltaNotification,
    ApplyPatchGuardianApprovalReviewAction,
    ApprovalsReviewer,
    AuthRecoveryNotification,
    CommandGuardianApprovalReviewAction,
    GetAccountResponse,
    PlanType,
    ReasoningEffort,
    ReasoningEffortOption,
    ThreadForkParams,
    ThreadListParams,
    ThreadQueueChangedNotification,
    ThreadResumeResponse,
    ThreadStartParams,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification,
    TurnStartParams,
    WarningNotification,
)
from openai_codex.models import InitializeResponse, JsonObject, Notification, UnknownNotification
from openai_codex.types import ThreadSource

ROOT = Path(__file__).resolve().parents[1]


@pytest.mark.parametrize(
    ("model", "fields"),
    [
        (
            CommandGuardianApprovalReviewAction,
            {"type": "command", "command": "pwd", "source": "shell"},
        ),
        (
            ApplyPatchGuardianApprovalReviewAction,
            {"type": "applyPatch", "files": [AbsolutePathBuf("/workspace/file")]},
        ),
    ],
)
def test_approval_review_paths_preserve_existing_wrappers(model, fields) -> None:
    action = model(cwd=AbsolutePathBuf("/workspace"), **fields)
    expected = {
        **fields,
        "cwd": "/workspace",
    }
    if "files" in expected:
        expected["files"] = ["/workspace/file"]
    assert action.model_dump(mode="json") == expected
    assert isinstance(action.cwd, AbsolutePathBuf)


def _initialized_client(
    monkeypatch: pytest.MonkeyPatch, metadata: JsonObject
) -> tuple[CodexClient, list[tuple[str, JsonObject | None]]]:
    client = CodexClient()
    requests: list[tuple[str, JsonObject | None]] = []

    def request_raw(method: str, params: JsonObject | None) -> JsonObject:
        requests.append((method, params))
        return metadata if method == "initialize" else {}

    monkeypatch.setattr(client, "_request_raw", request_raw)
    monkeypatch.setattr(client, "notify", lambda *_args: None)
    client.initialize()
    requests.clear()
    return client, requests


@pytest.mark.parametrize(
    ("method", "params"),
    [
        ("turn/start", {"input": [], "toolOutput": {"name": "delegate", "output": "Investigate"}}),
        ("turn/start", {"input": [], "turnTrigger": "automation"}),
        ("turn/start", {"input": [], "serviceTierForTurn": "default"}),
        ("thread/resume", {"threadId": "thread-1", "excludeTurns": False}),
        ("thread/fork", {"threadId": "thread-1", "excludeTurns": True}),
    ],
)
@pytest.mark.parametrize("version", ["0.147.0", "0.149.0", "0.151.0-alpha.6", "unknown", ""])
def test_new_options_reject_unsupported_runtime_before_sending(
    monkeypatch: pytest.MonkeyPatch, method: str, params: JsonObject, version: str
) -> None:
    client, requests = _initialized_client(monkeypatch, {"userAgent": f"codex-cli/{version}"})

    with pytest.raises(CodexError, match=r"Codex CLI 0\.151\.0 or newer"):
        client.request(method, params, response_model=InitializeResponse)

    assert requests == []


@pytest.mark.parametrize(
    "metadata",
    [
        {"userAgent": "codex-cli/0.151.0 (Linux)"},
        {"userAgent": "codex-cli 0.153.0"},
        {"userAgent": "codex-cli/0.154.0-alpha.1"},
        {"userAgent": "codex-cli/0.154.0-alpha.1.2"},
        {"userAgent": "codex-cli/0.151.0.post1"},
        {"userAgent": "unknown", "serverInfo": {"name": "codex", "version": "0.153.0"}},
    ],
)
def test_new_options_accept_supported_runtime_metadata(
    monkeypatch: pytest.MonkeyPatch, metadata: JsonObject
) -> None:
    client, requests = _initialized_client(monkeypatch, metadata)
    params = {"input": [], "toolOutput": {"name": "delegate", "output": "Investigate"}}

    client.request("turn/start", params, response_model=InitializeResponse)

    assert requests == [("turn/start", params)]


@pytest.mark.parametrize("supports_options", [True, False])
def test_unversioned_checkout_probes_and_caches_its_own_schema(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path, supports_options: bool
) -> None:
    client, requests = _initialized_client(monkeypatch, {"userAgent": "codex-cli/0.0.0"})
    command = ("checkout-codex", "--config", "key=value", "app-server")
    client._checkout_capabilities = CheckoutCapabilities(
        command, str(tmp_path), {"CUSTOM": "value"}
    )
    probes = []

    def generate_schema(args, **kwargs):
        probes.append((args[:-1], kwargs))
        output = Path(args[-1]) / "v2"
        output.mkdir()
        for name, fields in (
            ("TurnStartParams", ["turnTrigger", "serviceTierForTurn"]),
            ("ThreadResumeParams", ["excludeTurns"]),
            ("ThreadForkParams", ["excludeTurns"]),
        ):
            (output / f"{name}.json").write_text(
                json.dumps(
                    {"properties": {field: {} for field in fields} if supports_options else {}}
                )
            )

    monkeypatch.setattr("openai_codex._runtime_requirements.subprocess.run", generate_schema)
    for method, params in (
        ("turn/start", {"input": [], "turnTrigger": "automation"}),
        ("thread/resume", {"threadId": "thread-1", "excludeTurns": False}),
    ):
        if supports_options:
            client.request(method, params, response_model=InitializeResponse)
        else:
            with pytest.raises(CodexError, match="checkout does not support"):
                client.request(method, params, response_model=InitializeResponse)
    assert len(requests) == (2 if supports_options else 0)
    assert probes == [
        (
            [*command, "generate-json-schema", "--experimental", "--out"],
            {
                "cwd": str(tmp_path),
                "env": {"CUSTOM": "value"},
                "capture_output": True,
                "check": True,
                "timeout": 30,
            },
        )
    ]
    client.close()
    assert client._checkout_capabilities is None


def test_unversioned_custom_launch_requires_verifiable_capabilities(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, requests = _initialized_client(monkeypatch, {"userAgent": "codex-cli/0.0.0"})
    with pytest.raises(CodexError, match="Cannot verify an unversioned CLI"):
        client.request(
            "turn/start", {"turnTrigger": "automation"}, response_model=InitializeResponse
        )
    assert requests == []


@pytest.mark.parametrize("metadata", [{}, {"userAgent": "codex-cli/0.147.0"}])
def test_ordinary_requests_keep_working_on_old_or_unknown_runtime(
    monkeypatch: pytest.MonkeyPatch, metadata: JsonObject
) -> None:
    client, requests = _initialized_client(monkeypatch, metadata)
    params = {"input": [{"type": "text", "text": "Hello"}], "serviceTier": "default"}

    client.request("turn/start", params, response_model=InitializeResponse)
    client.request("thread/resume", {"threadId": "thread-1"}, response_model=InitializeResponse)
    client.request("thread/fork", {"threadId": "thread-1"}, response_model=InitializeResponse)

    assert requests == [
        ("turn/start", params),
        ("thread/resume", {"threadId": "thread-1"}),
        ("thread/fork", {"threadId": "thread-1"}),
    ]


def test_new_options_require_fresh_initialize_metadata_after_close(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    client, requests = _initialized_client(monkeypatch, {"userAgent": "codex-cli/0.153.0"})
    client.close()

    with pytest.raises(CodexError, match="reported version is 'unknown'"):
        client.request("thread/resume", {"excludeTurns": True}, response_model=InitializeResponse)

    assert requests == []


def test_generated_params_models_are_snake_case_and_dump_by_alias() -> None:
    params = ThreadListParams(search_term="needle", limit=5)

    assert "search_term" in ThreadListParams.model_fields
    dumped = _params_dict(params)
    assert dumped == {"searchTerm": "needle", "limit": 5}


def test_generated_v2_bundle_has_single_shared_plan_type_definition() -> None:
    source = (ROOT / "src" / "openai_codex" / "generated" / "v2_all.py").read_text()
    assert source.count("class PlanType(") == 1


def test_plan_type_accepts_business_prolite_from_newer_runtime() -> None:
    """New runtime plan values should remain typed when using a codex_bin override."""
    plan_type = "self_serve_business_prolite"
    response = GetAccountResponse.model_validate(
        {
            "account": {
                "type": "chatgpt",
                "email": "user@example.com",
                "planType": plan_type,
            },
            "requiresOpenaiAuth": True,
        }
    )
    assert response.account is not None
    assert response.account.root.plan_type.value == plan_type

    client = CodexClient()
    account_updated = client._coerce_notification(
        "account/updated",
        {"authMode": "chatgpt", "planType": plan_type},
    )
    assert isinstance(account_updated.payload, AccountUpdatedNotification)
    assert account_updated.payload.plan_type == PlanType(plan_type)

    rate_limits_updated = client._coerce_notification(
        "account/rateLimits/updated",
        {"rateLimits": {"planType": plan_type}},
    )
    assert isinstance(rate_limits_updated.payload, AccountRateLimitsUpdatedNotification)
    assert rate_limits_updated.payload.rate_limits.plan_type == PlanType(plan_type)


@pytest.mark.parametrize(
    ("effort", "wire_value"),
    [(ReasoningEffort.max, "max"), (ReasoningEffort.ultra, "ultra")],
)
def test_reasoning_effort_preserves_enum_constants_and_accepts_future_values(
    effort: ReasoningEffort, wire_value: str
) -> None:
    """Known effort members and new runtime values should share the enum-style API."""
    known_option = ReasoningEffortOption.model_validate(
        {"description": "Balanced", "reasoningEffort": "medium"}
    )
    future_option = ReasoningEffortOption.model_validate(
        {"description": "Future", "reasoningEffort": "future"}
    )
    turn_params = TurnStartParams(
        thread_id="thread-1",
        input=[],
        effort=effort,
    )

    assert {
        "known_member": ReasoningEffort.medium.value,
        "known_option": known_option.reasoning_effort.value,
        "future_option": future_option.reasoning_effort.value,
        "turn_effort": _params_dict(turn_params)["effort"],
    } == {
        "known_member": "medium",
        "known_option": "medium",
        "future_option": "future",
        "turn_effort": wire_value,
    }


def test_thread_source_preserves_enum_constants_and_accepts_future_values() -> None:
    """Known thread sources and new runtime values should share the enum-style API."""
    start_params = ThreadStartParams(thread_source=ThreadSource.user)
    fork_params = ThreadForkParams(
        thread_id="thread-1",
        thread_source=ThreadSource("future_source"),
    )

    assert {
        "known_member": ThreadSource.user.value,
        "subagent_member": ThreadSource.subagent.value,
        "memory_member": ThreadSource.memory_consolidation.value,
        "start_source": _params_dict(start_params)["threadSource"],
        "fork_source": _params_dict(fork_params)["threadSource"],
    } == {
        "known_member": "user",
        "subagent_member": "subagent",
        "memory_member": "memory_consolidation",
        "start_source": "user",
        "fork_source": "future_source",
    }


def test_thread_resume_response_accepts_auto_review_reviewer() -> None:
    """Generated response models should keep accepting the auto review enum value."""
    response = ThreadResumeResponse.model_validate(
        {
            "approvalPolicy": "on-request",
            "approvalsReviewer": "auto_review",
            "cwd": "/tmp",
            "model": "gpt-5",
            "modelProvider": "openai",
            "sandbox": {"type": "dangerFullAccess"},
            "thread": {
                "cliVersion": "1.0.0",
                "createdAt": 1,
                "cwd": "/tmp",
                "ephemeral": False,
                "id": "thread-1",
                "modelProvider": "openai",
                "preview": "",
                # The pinned runtime schema requires the session id on threads.
                "sessionId": "session-1",
                "source": "cli",
                "status": {"type": "idle"},
                "turns": [],
                "updatedAt": 1,
            },
        }
    )

    assert response.approvals_reviewer is ApprovalsReviewer.auto_review


def test_notifications_are_typed_with_canonical_v2_methods() -> None:
    client = CodexClient()
    event = client._coerce_notification(
        "thread/tokenUsage/updated",
        {
            "threadId": "thread-1",
            "turnId": "turn-1",
            "tokenUsage": {
                "last": {
                    "cachedInputTokens": 0,
                    "inputTokens": 1,
                    "outputTokens": 2,
                    "reasoningOutputTokens": 0,
                    "totalTokens": 3,
                },
                "total": {
                    "cachedInputTokens": 0,
                    "inputTokens": 1,
                    "outputTokens": 2,
                    "reasoningOutputTokens": 0,
                    "totalTokens": 3,
                },
            },
        },
    )

    assert event.method == "thread/tokenUsage/updated"
    assert isinstance(event.payload, ThreadTokenUsageUpdatedNotification)
    assert event.payload.turn_id == "turn-1"


def test_unknown_notifications_fall_back_to_unknown_payloads() -> None:
    client = CodexClient()
    event = client._coerce_notification(
        "unknown/notification",
        {
            "id": "evt-1",
            "conversationId": "thread-1",
            "msg": {"type": "turn_aborted"},
        },
    )

    assert event.method == "unknown/notification"
    assert isinstance(event.payload, UnknownNotification)
    assert event.payload.params["msg"] == {"type": "turn_aborted"}


@pytest.mark.parametrize(
    ("method", "params", "expected"),
    [
        (
            "modelProvider/authRecoveryCompleted",
            {
                "provider": "openai",
                "message": "Authentication recovered",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
            AuthRecoveryNotification(
                provider="openai",
                message="Authentication recovered",
                thread_id="thread-1",
                turn_id="turn-1",
            ),
        ),
        (
            "thread/queue/changed",
            {"threadId": "thread-1"},
            ThreadQueueChangedNotification(thread_id="thread-1"),
        ),
        ("warning", {"message": "heads up"}, WarningNotification(message="heads up")),
        (
            "future/notification",
            {"newField": "value"},
            UnknownNotification(params={"newField": "value"}),
        ),
    ],
)
def test_decoded_notifications_match_the_declared_payload_type(method, params, expected) -> None:
    event = CodexClient()._coerce_notification(method, params)

    assert event == Notification(method=method, payload=expected)
    assert isinstance(event.payload, get_type_hints(Notification)["payload"])


def test_invalid_notification_payload_falls_back_to_unknown() -> None:
    client = CodexClient()
    event = client._coerce_notification("thread/tokenUsage/updated", {"threadId": "missing"})

    assert event.method == "thread/tokenUsage/updated"
    assert isinstance(event.payload, UnknownNotification)


def test_generated_notification_turn_id_handles_known_payload_shapes() -> None:
    """Generated routing metadata should cover direct, nested, and unscoped payloads."""
    direct = AgentMessageDeltaNotification.model_validate(
        {
            "delta": "hello",
            "itemId": "item-1",
            "threadId": "thread-1",
            "turnId": "turn-1",
        }
    )
    nested = TurnCompletedNotification.model_validate(
        {
            "threadId": "thread-1",
            "turn": {"id": "turn-2", "items": [], "status": "completed"},
        }
    )
    unscoped = WarningNotification(message="heads up")

    assert [
        notification_turn_id(direct),
        notification_turn_id(nested),
        notification_turn_id(unscoped),
    ] == ["turn-1", "turn-2", None]


def test_turn_notification_router_demuxes_registered_turns() -> None:
    """The router should deliver out-of-order turn events to the matching queues."""
    client = CodexClient()
    client.register_turn_notifications("turn-1")
    client.register_turn_notifications("turn-2")

    client._router.route_notification(
        client._coerce_notification(
            "item/agentMessage/delta",
            {
                "delta": "two",
                "itemId": "item-2",
                "threadId": "thread-1",
                "turnId": "turn-2",
            },
        )
    )
    client._router.route_notification(
        client._coerce_notification(
            "item/agentMessage/delta",
            {
                "delta": "one",
                "itemId": "item-1",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
        )
    )

    first = client.next_turn_notification("turn-1")
    second = client.next_turn_notification("turn-2")

    assert isinstance(first.payload, AgentMessageDeltaNotification)
    assert isinstance(second.payload, AgentMessageDeltaNotification)
    assert [
        (first.method, first.payload.delta),
        (second.method, second.payload.delta),
    ] == [
        ("item/agentMessage/delta", "one"),
        ("item/agentMessage/delta", "two"),
    ]


def test_goal_notification_router_routes_by_thread_id() -> None:
    """A goal operation should receive turn notifications across physical turn ids."""
    client = CodexClient()
    state = client.register_goal_operation("thread-1")

    client._router.route_notification(
        client._coerce_notification(
            "item/agentMessage/delta",
            {
                "delta": "continued",
                "itemId": "item-1",
                "threadId": "thread-1",
                "turnId": "turn-2",
            },
        )
    )

    event = client.next_goal_notification(state)

    assert isinstance(event.payload, AgentMessageDeltaNotification)
    assert (event.method, event.payload.delta) == (
        "item/agentMessage/delta",
        "continued",
    )


def test_client_reader_routes_interleaved_turn_notifications_by_turn_id() -> None:
    """Reader-loop routing should preserve order within each interleaved turn stream."""
    client = CodexClient()
    client.register_turn_notifications("turn-1")
    client.register_turn_notifications("turn-2")

    messages: list[dict[str, object]] = [
        {
            "method": "item/agentMessage/delta",
            "params": {
                "delta": "one-a",
                "itemId": "item-1",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
        },
        {
            "method": "item/agentMessage/delta",
            "params": {
                "delta": "two-a",
                "itemId": "item-2",
                "threadId": "thread-1",
                "turnId": "turn-2",
            },
        },
        {
            "method": "item/agentMessage/delta",
            "params": {
                "delta": "one-b",
                "itemId": "item-3",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
        },
        {
            "method": "item/agentMessage/delta",
            "params": {
                "delta": "two-b",
                "itemId": "item-4",
                "threadId": "thread-1",
                "turnId": "turn-2",
            },
        },
    ]

    def fake_read_message() -> dict[str, object]:
        """Feed the reader loop a realistic interleaved stdout sequence."""
        if messages:
            return messages.pop(0)
        raise EOFError

    client._read_message = fake_read_message  # type: ignore[method-assign]
    client._reader_loop()

    first_turn_events = [
        client.next_turn_notification("turn-1"),
        client.next_turn_notification("turn-1"),
    ]
    second_turn_events = [
        client.next_turn_notification("turn-2"),
        client.next_turn_notification("turn-2"),
    ]

    first_turn_deltas = [
        event.payload.delta
        for event in first_turn_events
        if isinstance(event.payload, AgentMessageDeltaNotification)
    ]
    second_turn_deltas = [
        event.payload.delta
        for event in second_turn_events
        if isinstance(event.payload, AgentMessageDeltaNotification)
    ]
    assert (first_turn_deltas, second_turn_deltas) == (
        ["one-a", "one-b"],
        ["two-a", "two-b"],
    )


def test_turn_notification_router_starts_at_explicit_registration() -> None:
    """Explicit registration receives events from when the caller attaches."""
    client = CodexClient()
    client.register_turn_notifications("turn-1")
    client._router.route_notification(
        client._coerce_notification(
            "item/agentMessage/delta",
            {
                "delta": "early",
                "itemId": "item-1",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
        )
    )

    event = client.next_turn_notification("turn-1")

    assert isinstance(event.payload, AgentMessageDeltaNotification)
    assert (event.method, event.payload.delta) == (
        "item/agentMessage/delta",
        "early",
    )


def test_turn_notification_router_clears_unregistered_turn_when_completed() -> None:
    """A completed unregistered turn should not leave a pending queue behind."""
    client = CodexClient()
    client._router.route_notification(
        client._coerce_notification(
            "item/agentMessage/delta",
            {
                "delta": "early",
                "itemId": "item-1",
                "threadId": "thread-1",
                "turnId": "turn-1",
            },
        )
    )
    client._router.route_notification(
        client._coerce_notification(
            "turn/completed",
            {
                "threadId": "thread-1",
                "turn": {"id": "turn-1", "items": [], "status": "completed"},
            },
        )
    )

    assert client._router._turn_states == {}


def test_turn_notification_router_routes_unknown_turn_notifications() -> None:
    """Unknown notifications should still route when their raw params carry a turn id."""
    client = CodexClient()
    client.register_turn_notifications("turn-1")
    client.register_turn_notifications("turn-2")

    client._router.route_notification(
        Notification(
            method="unknown/direct",
            payload=UnknownNotification(params={"turnId": "turn-1"}),
        )
    )
    client._router.route_notification(
        Notification(
            method="unknown/nested",
            payload=UnknownNotification(params={"turn": {"id": "turn-2"}}),
        )
    )

    first = client.next_turn_notification("turn-1")
    second = client.next_turn_notification("turn-2")

    assert [first.method, second.method] == ["unknown/direct", "unknown/nested"]
