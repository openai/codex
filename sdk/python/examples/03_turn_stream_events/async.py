import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Count from 1 to 200 with commas, then one summary sentence."))

        # Best effort controls: models can finish quickly, so races are expected.
        try:
            _ = await turn.steer(TextInput("Keep it brief and stop after 20 numbers."))
            print("steer: sent")
        except Exception as exc:
            print("steer: skipped", type(exc).__name__)

        try:
            _ = await turn.interrupt()
            print("interrupt: sent")
        except Exception as exc:
            print("interrupt: skipped", type(exc).__name__)

        event_count = 0
        async for event in turn.stream():
            event_count += 1
            print(event.method, event.payload)

        print("events.count:", event_count)


if __name__ == "__main__":
    asyncio.run(main())
