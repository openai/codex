import asyncio
import gc
import threading
from concurrent.futures import ThreadPoolExecutor
from itertools import chain

import pytest

from openai_codex import AsyncCodex
from openai_codex._run import _collect_turn_result
from openai_codex.api import AsyncThread, AsyncTurnHandle, Thread, TurnHandle
from openai_codex.async_client import AsyncCodexClient
from openai_codex.client import CodexClient
from openai_codex.errors import TransportClosedError


def turn_events(client, *, status="completed"):
    scope = {"threadId": "thread-1", "turnId": "turn-1"}
    usage = {
        "inputTokens": 2,
        "cachedInputTokens": 0,
        "outputTokens": 3,
        "reasoningOutputTokens": 0,
        "totalTokens": 5,
    }
    return [
        client._coerce_notification(
            "item/completed",
            {
                **scope,
                "completedAtMs": 1,
                "item": {
                    "id": "message",
                    "type": "agentMessage",
                    "text": "done",
                    "phase": "final_answer",
                },
            },
        ),
        client._coerce_notification(
            "thread/tokenUsage/updated", {**scope, "tokenUsage": {"last": usage, "total": usage}}
        ),
        client._coerce_notification(
            "turn/completed",
            {
                "threadId": "thread-1",
                "turn": {
                    "id": "turn-1",
                    "items": [],
                    "status": status,
                    "error": {"message": "model failed"} if status == "failed" else None,
                },
            },
        ),
    ]


@pytest.mark.parametrize("consumed", [False, True])
def test_late_join_starts_with_future_events(consumed):
    client = CodexClient()
    original = TurnHandle(client, "thread-1", "turn-1")
    events = turn_events(client)
    client._router.route_notification(events[0])
    stream = original.stream()
    previous = [next(stream)] if consumed else []
    joined = TurnHandle(client, "thread-1", "turn-1")
    client._router.route_notification(events[1])
    for event in events[2:]:
        client._router.route_notification(event)

    first_result = _collect_turn_result(chain(previous, stream), turn_id="turn-1")
    joined_result = joined.run()
    assert first_result.final_response == "done"
    assert joined_result.final_response is None
    assert joined_result.usage == first_result.usage
    assert client._router._turn_states == {}


def test_consumed_deltas_are_released_while_turn_is_active():
    client = CodexClient()
    subscription = client._subscribe_turn_notifications("turn-1")
    state = client._router._turn_states["turn-1"]
    for index in range(1000):
        event = client._coerce_notification(
            "item/agentMessage/delta",
            {
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "message",
                "delta": str(index),
            },
        )
        client._router.route_notification(event)
        assert subscription.next() == event
    assert state.events == {}
    assert not state.completed
    subscription.close()


def test_slow_subscriber_keeps_unread_deltas_until_it_consumes_them():
    client = CodexClient()
    fast = client._subscribe_turn_notifications("turn-1")
    slow = client._subscribe_turn_notifications("turn-1")
    event = client._coerce_notification(
        "item/agentMessage/delta",
        {"threadId": "thread-1", "turnId": "turn-1", "itemId": "message", "delta": "hello"},
    )
    client._router.route_notification(event)
    assert fast.next() == event
    assert slow.next() == event
    assert client._router._turn_states["turn-1"].events == {}
    fast.close()
    slow.close()


@pytest.mark.parametrize("async_api", [False, True])
@pytest.mark.parametrize("low_level", [False, True])
@pytest.mark.parametrize("completed", [False, True])
def test_events_or_failure_before_turn_start_returns(monkeypatch, async_api, low_level, completed):
    codex = AsyncCodex()
    codex._initialized = True
    client = codex._client._sync if async_api else CodexClient()

    def request_raw(method, params):
        assert method == "turn/start"
        for event in turn_events(client) if completed else []:
            client._router.route_notification(event)
        client._router.fail_all(TransportClosedError("transport failed"))
        return {"turn": {"id": "turn-1", "status": "inProgress", "items": []}}

    monkeypatch.setattr(client, "_request_raw", request_raw)
    public = codex._client if async_api else client
    thread = AsyncThread(codex, "thread-1") if async_api else Thread(client, "thread-1")

    async def value(call):
        return await call if async_api else call

    async def scenario():
        if low_level:
            started = await value(public.turn_start("thread-1", "hello"))
            assert (
                await value(public.wait_for_turn_completed(started.turn.id))
            ).turn.id == "turn-1"
        else:
            handle = await value(thread.turn("hello"))
            assert (await value(handle.run())).final_response == "done"

    if completed:
        asyncio.run(scenario())
    else:
        with pytest.raises(TransportClosedError, match="transport failed"):
            asyncio.run(scenario())
    assert client._router._turn_states == {}


def test_pending_join_starts_at_request_while_original_handle_finishes():
    client = CodexClient()
    original = TurnHandle(client, "thread-1", "turn-1")
    events = turn_events(client)
    unknown = [client._coerce_notification(name, {"turnId": "turn-1"}) for name in ("old", "live")]
    client._router.route_notification(unknown[0])
    assert original._subscription.next() is unknown[0]

    with client._router.pending_turn("thread-1") as cursors:
        client._router.route_notification(unknown[1])
        assert original._subscription.next() is unknown[1]
        client._router.route_notification(events[0])
        client._router.route_notification(events[1])
        manual = TurnHandle(client, "thread-1", "turn-1")
        client._router.route_notification(events[2])
        result = original.run()
        assert manual.run().usage is None
        subscription = client._router.prepare_turn("turn-1", "thread-1", cursors, for_handle=True)
    joined_handle = TurnHandle(client, "thread-1", "turn-1", _subscription=subscription)
    assert joined_handle._subscription.next() is unknown[1]
    joined = joined_handle.run()
    assert result.final_response == joined.final_response == "done"
    assert joined.usage.last.total_tokens == 5
    assert client._router._turn_states == {}


def test_failed_start_releases_early_completed_state():
    client = CodexClient()
    with pytest.raises(ValueError, match="request failed"):
        with client._router.pending_turn("thread-1"):
            for event in turn_events(client):
                client._router.route_notification(event)
            raise ValueError("request failed")
    assert client._router._turn_states == {}
    assert client._router._pending_turn_requests == {}


def test_closing_one_stream_leaves_other_subscriber_intact():
    client = CodexClient()
    original = TurnHandle(client, "thread-1", "turn-1")
    joined = TurnHandle(client, "thread-1", "turn-1")
    events = turn_events(client)
    client._router.route_notification(events[0])
    stream = original.stream()
    next(stream)
    stream.close()
    for event in events[1:]:
        client._router.route_notification(event)
    assert joined.run().final_response == "done"
    assert client._router._turn_states == {}


@pytest.mark.parametrize("failure", ["transport", "model"])
def test_both_handles_observe_failure_and_release_state(failure):
    client = CodexClient()
    handles = [TurnHandle(client, "thread-1", "turn-1") for _ in range(2)]
    if failure == "transport":
        client._router.fail_all(TransportClosedError("transport failed"))
    else:
        for event in turn_events(client, status="failed"):
            client._router.route_notification(event)
    for handle in handles:
        with pytest.raises((TransportClosedError, RuntimeError), match=f"{failure} failed"):
            handle.run()
    assert client._router._turn_states == {}


def test_abandoned_handle_releases_completed_history():
    client = CodexClient()
    handle = TurnHandle(client, "thread-1", "turn-1")
    for event in turn_events(client):
        client._router.route_notification(event)
    with client._router._lock:
        del handle
        gc.collect()
    assert client._router._turn_states == {}


def test_cancelled_async_consumer_leaves_other_handle_intact():
    async def scenario():
        codex = AsyncCodex()
        codex._initialized = True
        client = codex._client._sync
        original = AsyncTurnHandle(codex, "thread-1", "turn-1")
        joined = AsyncTurnHandle(codex, "thread-1", "turn-1")
        task = asyncio.create_task(original.run())
        await asyncio.sleep(0)  # Let the stream enter its wait before cancelling it.
        task.cancel()
        with pytest.raises(asyncio.CancelledError):
            await task
        for event in turn_events(client):
            client._router.route_notification(event)
        assert (await asyncio.wait_for(joined.run(), timeout=2)).final_response == "done"
        assert client._router._turn_states == {}

    asyncio.run(scenario())


def test_cancelled_turn_start_releases_result_after_response(monkeypatch):
    async def scenario():
        client = AsyncCodexClient()
        entered = threading.Event()
        respond = threading.Event()
        released = threading.Event()
        subscribe = client._sync._router.prepare_turn

        def request_raw(method, params):
            entered.set()
            assert respond.wait(timeout=2)
            for event in turn_events(client._sync):
                client._sync._router.route_notification(event)
            return {"turn": {"id": "turn-1", "items": [], "status": "completed"}}

        def observe_release(*args, **kwargs):
            subscription = subscribe(*args, **kwargs)
            close = subscription.close

            def close_and_signal():
                close()
                released.set()

            subscription.close = close_and_signal
            return subscription

        monkeypatch.setattr(client._sync, "_request_raw", request_raw)
        monkeypatch.setattr(client._sync._router, "prepare_turn", observe_release)
        task = asyncio.create_task(client.turn_start("thread-1", "hello"))
        try:
            assert await asyncio.to_thread(entered.wait, 2)
            task.cancel()
            with pytest.raises(asyncio.CancelledError):
                await task
        finally:
            respond.set()
        assert await asyncio.to_thread(released.wait, 2)
        assert client._sync._router._turn_states == {}

    asyncio.run(scenario())


def test_cancelled_queued_start_does_not_send_a_request(monkeypatch):
    with ThreadPoolExecutor(max_workers=1) as executor:
        monkeypatch.setattr("openai_codex.async_client._TURN_START_EXECUTOR", executor)
        release_worker = threading.Event()
        worker_started = threading.Event()
        request_sent = threading.Event()

        def occupy_worker():
            worker_started.set()
            assert release_worker.wait(timeout=5)

        occupied = executor.submit(occupy_worker)
        assert worker_started.wait(timeout=5)
        client = AsyncCodexClient()
        monkeypatch.setattr(client._sync, "_start_turn", lambda *args, **kwargs: request_sent.set())

        async def scenario():
            task = asyncio.create_task(client.turn_start("thread-1", "hello"))
            await asyncio.sleep(0)
            task.cancel()
            with pytest.raises(asyncio.CancelledError):
                await task

        try:
            asyncio.run(scenario())
        finally:
            release_worker.set()
        occupied.result(timeout=5)
        executor.submit(lambda: None).result(timeout=5)
        assert not request_sent.is_set()


def test_low_level_start_keeps_implicit_registration_and_explicit_unregister(monkeypatch):
    client = CodexClient()

    def request_raw(method, params):
        for event in turn_events(client):
            client._router.route_notification(event)
        return {"turn": {"id": "turn-1", "status": "completed", "items": []}}

    monkeypatch.setattr(client, "_request_raw", request_raw)
    started = client.turn_start("thread-1", "hello")
    registered = client._router._turn_notifications[started.turn.id]
    assert client.turn_start("thread-1", "again").turn.id == started.turn.id
    assert client._router._turn_notifications[started.turn.id] is registered
    assert client.next_turn_notification(started.turn.id) == turn_events(client)[0]
    client.unregister_turn_notifications(started.turn.id)
    with pytest.raises(RuntimeError, match="not registered"):
        client.next_turn_notification(started.turn.id)
    assert client._router._turn_states == {}
