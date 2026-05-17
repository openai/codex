import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio

from openai_codex import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(
            model="gpt-5.4", config={"model_reasoning_effort": "high"}
        )
        turn = await thread.turn(TextInput("Give 3 bullets about SIMD."))
        result = await turn.run()

        print("thread_id:", thread.id)
        print("turn_id:", result.id)
        print("status:", result.status)
        if result.error is not None:
            print("error:", result.error)
        print("text:", result.final_response)
        print("items.count:", len(result.items))


if __name__ == "__main__":
    asyncio.run(main())
