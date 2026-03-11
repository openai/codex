import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex


async def main() -> None:
    async with AsyncCodex() as codex:
        print("metadata:", codex.metadata)

        models = await codex.models(include_hidden=True)
        print("models.count:", len(models.data))
        if models.data:
            print("first model id:", models.data[0].id)


if __name__ == "__main__":
    asyncio.run(main())
