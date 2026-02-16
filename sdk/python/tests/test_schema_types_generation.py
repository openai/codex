from codex_app_server.schema_types import (
    ThreadStartResponse,
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
