import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import (
    assistant_text_from_turn,
    ensure_local_sdk_src,
    find_turn_by_id,
    runtime_config,
    temporary_sample_image_path,
)

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex, LocalImageInput, TextInput


async def main() -> None:
    with temporary_sample_image_path() as image_path:
        async with AsyncCodex(config=runtime_config()) as codex:
            thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})

            turn = await thread.turn(
                [
                    TextInput("Read this generated local image and summarize the colors/layout in 2 bullets."),
                    LocalImageInput(str(image_path.resolve())),
                ]
            )
            result = await turn.run()
            persisted = await thread.read(include_turns=True)
            persisted_turn = find_turn_by_id(persisted.thread.turns, result.id)

            print("Status:", result.status)
            print(assistant_text_from_turn(persisted_turn))


if __name__ == "__main__":
    asyncio.run(main())
