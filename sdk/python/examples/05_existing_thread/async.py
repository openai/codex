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
        original = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

        first_turn = await original.turn(TextInput("Tell me one fact about Saturn."))
        first = await first_turn.run()
        print("Created thread:", first.thread_id)

        resumed = await codex.thread_resume(first.thread_id)
        second_turn = await resumed.turn(TextInput("Continue with one more fact."))
        second = await second_turn.run()
        print(second.text)


if __name__ == "__main__":
    asyncio.run(main())
