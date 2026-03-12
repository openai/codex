import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio
import random
from collections.abc import Awaitable, Callable
from typing import TypeVar

from codex_app_server import (
    AsyncCodex,
    JsonRpcError,
    ServerBusyError,
    TextInput,
    TurnStatus,
    is_retryable_error,
)

ResultT = TypeVar("ResultT")


async def retry_on_overload_async(
    op: Callable[[], Awaitable[ResultT]],
    *,
    max_attempts: int = 3,
    initial_delay_s: float = 0.25,
    max_delay_s: float = 2.0,
    jitter_ratio: float = 0.2,
) -> ResultT:
    if max_attempts < 1:
        raise ValueError("max_attempts must be >= 1")

    delay = initial_delay_s
    attempt = 0
    while True:
        attempt += 1
        try:
            return await op()
        except Exception as exc:  # noqa: BLE001
            if attempt >= max_attempts or not is_retryable_error(exc):
                raise
            jitter = delay * jitter_ratio
            sleep_for = min(max_delay_s, delay) + random.uniform(-jitter, jitter)
            if sleep_for > 0:
                await asyncio.sleep(sleep_for)
            delay = min(max_delay_s, delay * 2)


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5", config={"model_reasoning_effort": "high"})

        try:
            result = await retry_on_overload_async(
                _run_turn(thread, "Summarize retry best practices in 3 bullets."),
                max_attempts=3,
                initial_delay_s=0.25,
                max_delay_s=2.0,
            )
        except ServerBusyError as exc:
            print("Server overloaded after retries:", exc.message)
            print("Text:")
            return
        except JsonRpcError as exc:
            print(f"JSON-RPC error {exc.code}: {exc.message}")
            print("Text:")
            return

        if result.status == TurnStatus.failed:
            print("Turn failed:", result.error)

        print("Text:", result.text)


def _run_turn(thread, prompt: str):
    async def _inner():
        turn = await thread.turn(TextInput(prompt))
        return await turn.run()

    return _inner


if __name__ == "__main__":
    asyncio.run(main())
