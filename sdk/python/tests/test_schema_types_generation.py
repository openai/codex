from codex_app_server.schema_types import (
    ItemCompletedNotificationPayload,
    ItemStartedNotificationPayload,
    ThreadForkResponse,
    ThreadNameUpdatedNotificationPayload,
    ThreadStartResponse,
    ThreadTokenUsageUpdatedNotificationPayload,
    TurnStartResponse,
    ThreadListResponse,
    TurnCompletedNotificationPayload,
)


def test_schema_generated_models_from_dict():
    t = ThreadStartResponse.from_dict({"thread": {"id": "thr_1"}})
    assert t.thread.id == "thr_1"

    turn = TurnStartResponse.from_dict({"turn": {"id": "turn_1", "status": "inProgress"}})
    assert turn.turn.id == "turn_1"

    listed = ThreadListResponse.from_dict({"data": [{"id": "thr_1"}], "nextCursor": None})
    assert listed.data[0].id == "thr_1"

    done = TurnCompletedNotificationPayload.from_dict(
        {"threadId": "thr_1", "turn": {"id": "turn_1", "status": "completed"}}
    )
    assert done.turn.status == "completed"

    forked = ThreadForkResponse.from_dict({"thread": {"id": "thr_2"}})
    assert forked.thread.id == "thr_2"

    started_evt = ItemStartedNotificationPayload.from_dict(
        {"threadId": "thr_1", "turnId": "turn_1", "item": {"id": "i1", "type": "agentMessage"}}
    )
    completed_evt = ItemCompletedNotificationPayload.from_dict(
        {"threadId": "thr_1", "turnId": "turn_1", "item": {"id": "i1", "type": "agentMessage"}}
    )
    assert started_evt.turnId == completed_evt.turnId == "turn_1"

    renamed = ThreadNameUpdatedNotificationPayload.from_dict({"threadId": "thr_1", "threadName": "renamed"})
    assert renamed.threadName == "renamed"

    usage = ThreadTokenUsageUpdatedNotificationPayload.from_dict(
        {
            "threadId": "thr_1",
            "turnId": "turn_1",
            "tokenUsage": {
                "last": {
                    "cachedInputTokens": 1,
                    "inputTokens": 2,
                    "outputTokens": 3,
                    "reasoningOutputTokens": 4,
                    "totalTokens": 10,
                },
                "total": {
                    "cachedInputTokens": 1,
                    "inputTokens": 2,
                    "outputTokens": 3,
                    "reasoningOutputTokens": 4,
                    "totalTokens": 10,
                },
                "modelContextWindow": 200000,
            },
        }
    )
    assert usage.tokenUsage["modelContextWindow"] == 200000
