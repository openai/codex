from __future__ import annotations

import asyncio
from pathlib import Path

import pytest

from codex_sdk.asyncio import AsyncCodex, AbortController
from codex_sdk.errors import AbortError
from .utils import fake_codex_path


@pytest.fixture
def log_path(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    path = tmp_path / "async-log.jsonl"
    monkeypatch.setenv("CODEX_FAKE_LOG", str(path))
    monkeypatch.setenv("CODEX_FAKE_MODE", "basic")
    return path


@pytest.mark.asyncio
async def test_async_run(log_path: Path) -> None:
    client = AsyncCodex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    result = await thread.run("Hello")

    assert result.final_response == "Hi!"
    assert result.items == [{"id": "item_0", "type": "agent_message", "text": "Hi!"}]
    assert thread.id is not None


@pytest.mark.asyncio
async def test_async_run_streamed(log_path: Path) -> None:
    client = AsyncCodex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()
    streamed = await thread.run_streamed("Hello")

    events = []
    async for event in streamed.events:
        events.append(event)

    assert events[0]["type"] == "thread.started"
    assert events[2]["item"]["text"] == "Hi!"


@pytest.mark.asyncio
async def test_async_abort(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> None:
    controller = AbortController()

    monkeypatch.setenv("CODEX_FAKE_LOG", str(tmp_path / "async-abort.jsonl"))
    monkeypatch.setenv("CODEX_FAKE_MODE", "infinite")

    client = AsyncCodex(codex_path_override=fake_codex_path(), base_url="http://test", api_key="test")
    thread = client.start_thread()

    async def abort_soon() -> None:
        await asyncio.sleep(0.05)
        controller.abort("stop")

    asyncio.create_task(abort_soon())

    streamed = await thread.run_streamed("Hello", signal=controller.signal)

    with pytest.raises(AbortError):
        async for _event in streamed.events:
            await asyncio.sleep(0.01)
