from __future__ import annotations

import asyncio
import time

from codex_app_server.async_client import AsyncAppServerClient


def test_async_client_allows_parallel_transport_calls() -> None:
    async def scenario() -> int:
        client = AsyncAppServerClient()
        active = 0
        max_active = 0

        def fake_model_list(include_hidden: bool = False) -> bool:
            nonlocal active, max_active
            active += 1
            max_active = max(max_active, active)
            time.sleep(0.05)
            active -= 1
            return include_hidden

        client._sync.model_list = fake_model_list  # type: ignore[method-assign]
        await asyncio.gather(client.model_list(), client.model_list())
        return max_active

    assert asyncio.run(scenario()) == 2


def test_async_stream_text_is_incremental_and_allows_parallel_calls() -> None:
    async def scenario() -> tuple[str, list[str], bool]:
        client = AsyncAppServerClient()

        def fake_stream_text(thread_id: str, text: str, params=None):  # type: ignore[no-untyped-def]
            yield "first"
            time.sleep(0.03)
            yield "second"
            yield "third"

        def fake_model_list(include_hidden: bool = False) -> str:
            return "done"

        client._sync.stream_text = fake_stream_text  # type: ignore[method-assign]
        client._sync.model_list = fake_model_list  # type: ignore[method-assign]

        stream = client.stream_text("thread-1", "hello")
        first = await anext(stream)

        completed_before_stream_done = False
        competing_call = asyncio.create_task(client.model_list())
        await asyncio.sleep(0.01)
        completed_before_stream_done = competing_call.done()

        remaining: list[str] = []
        async for item in stream:
            remaining.append(item)

        await competing_call
        return first, remaining, completed_before_stream_done

    first, remaining, completed = asyncio.run(scenario())
    assert first == "first"
    assert remaining == ["second", "third"]
    assert completed
