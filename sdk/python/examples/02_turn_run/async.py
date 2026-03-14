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
        turn = await thread.turn(TextInput("Give 3 bullets about SIMD."))
        result = await turn.run()
        persisted = await thread.read(include_turns=True)
        persisted_turn = next(
            (turn for turn in persisted.thread.turns or [] if turn.id == result.turn_id),
            None,
        )

        print("thread_id:", result.thread_id)
        print("turn_id:", result.turn_id)
        print("status:", result.status)
        if result.error is not None:
            print("error:", result.error)
        print("text:", result.text)
        print(
            "persisted.items.count:",
            0 if persisted_turn is None else len(persisted_turn.items or []),
        )
        if result.usage is None:
            raise RuntimeError("missing usage for completed turn")
        print("usage.thread_id:", result.usage.thread_id)
        print("usage.turn_id:", result.usage.turn_id)


if __name__ == "__main__":
    asyncio.run(main())
