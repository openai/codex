from __future__ import annotations

from pathlib import Path
from typing import Any

from codex_app_server.client import AppServerClient, _params_dict
from codex_app_server.generated.v2_all import (
    ApprovalsReviewer,
    ThreadListParams,
    ThreadResumeResponse,
    ThreadTokenUsageUpdatedNotification,
)
from codex_app_server.models import UnknownNotification

ROOT = Path(__file__).resolve().parents[1]


def test_thread_set_name_and_compact_use_current_rpc_methods() -> None:
    client = AppServerClient()
    calls: list[tuple[str, dict[str, Any] | None]] = []

    def fake_request(method: str, params, *, response_model):  # type: ignore[no-untyped-def]
        calls.append((method, params))
        return response_model.model_validate({})

    client.request = fake_request  # type: ignore[method-assign]

    client.thread_set_name("thread-1", "sdk-name")
    client.thread_compact("thread-1")

    assert calls[0][0] == "thread/name/set"
    assert calls[1][0] == "thread/compact/start"


def test_generated_params_models_are_snake_case_and_dump_by_alias() -> None:
    params = ThreadListParams(search_term="needle", limit=5)

    assert "search_term" in ThreadListParams.model_fields
    dumped = _params_dict(params)
    assert dumped == {"searchTerm": "needle", "limit": 5}


def test_generated_v2_bundle_has_single_shared_plan_type_definition() -> None:
    source = (ROOT / "src" / "codex_app_server" / "generated" / "v2_all.py").read_text()
    assert source.count("class PlanType(") == 1


def test_thread_resume_response_accepts_auto_review_reviewer() -> None:
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
                "source": "cli",
                "status": {"type": "idle"},
                "turns": [],
                "updatedAt": 1,
            },
        }
    )

    assert response.approvals_reviewer is ApprovalsReviewer.auto_review


def test_notifications_are_typed_with_canonical_v2_methods() -> None:
    client = AppServerClient()
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
    client = AppServerClient()
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


def test_invalid_notification_payload_falls_back_to_unknown() -> None:
    client = AppServerClient()
    event = client._coerce_notification(
        "thread/tokenUsage/updated", {"threadId": "missing"}
    )

    assert event.method == "thread/tokenUsage/updated"
    assert isinstance(event.payload, UnknownNotification)


def test_turn_notification_router_demuxes_registered_turns() -> None:
    client = AppServerClient()
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

    assert [
        (first.method, getattr(first.payload, "delta", None)),
        (second.method, getattr(second.payload, "delta", None)),
    ] == [
        ("item/agentMessage/delta", "one"),
        ("item/agentMessage/delta", "two"),
    ]


def test_turn_notification_router_buffers_events_before_registration() -> None:
    client = AppServerClient()
    client._router.route_notification(
        client._coerce_notification(
            "turn/completed",
            {
                "threadId": "thread-1",
                "turn": {"id": "turn-1", "items": [], "status": "completed"},
            },
        )
    )

    client.register_turn_notifications("turn-1")
    event = client.next_turn_notification("turn-1")

    assert (
        event.method,
        getattr(getattr(event.payload, "turn", None), "id", None),
    ) == (
        "turn/completed",
        "turn-1",
    )
