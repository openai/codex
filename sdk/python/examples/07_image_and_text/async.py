import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex, ImageInput, TextInput

REMOTE_IMAGE_URL = "https://raw.githubusercontent.com/github/explore/main/topics/python/python.png"


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(
            [
                TextInput("What is in this image? Give 3 bullets."),
                ImageInput(REMOTE_IMAGE_URL),
            ]
        )
        result = await turn.run()

        print("Status:", result.status)
        print(result.text)


if __name__ == "__main__":
    asyncio.run(main())
