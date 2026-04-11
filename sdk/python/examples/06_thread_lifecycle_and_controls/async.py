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
        first = await (await thread.turn(TextInput("One sentence about structured planning."))).run()
        second = await (await thread.turn(TextInput("Now restate it for a junior engineer."))).run()

        reopened = await codex.thread_resume(thread.id)
        listing_active = await codex.thread_list(limit=20, archived=False)
        reading = await reopened.read(include_turns=True)

        _ = await reopened.set_name("sdk-lifecycle-demo")
        _ = await codex.thread_archive(reopened.id)
        listing_archived = await codex.thread_list(limit=20, archived=True)
        unarchived = await codex.thread_unarchive(reopened.id)

        resumed_info = "n/a"
        try:
            resumed = await codex.thread_resume(
                unarchived.id,
                model="gpt-5.4",
                config={"model_reasoning_effort": "high"},
            )
            resumed_result = await (await resumed.turn(TextInput("Continue in one short sentence."))).run()
            resumed_info = f"{resumed_result.id} {resumed_result.status}"
        except Exception as exc:
            resumed_info = f"skipped({type(exc).__name__})"

        forked_info = "n/a"
        try:
            forked = await codex.thread_fork(unarchived.id, model="gpt-5.4")
            forked_result = await (await forked.turn(TextInput("Take a different angle in one short sentence."))).run()
            forked_info = f"{forked_result.id} {forked_result.status}"
        except Exception as exc:
            forked_info = f"skipped({type(exc).__name__})"

        compact_info = "sent"
        try:
            _ = await unarchived.compact()
        except Exception as exc:
            compact_info = f"skipped({type(exc).__name__})"

        print("Lifecycle OK:", thread.id)
        print("first:", first.id, first.status)
        print("second:", second.id, second.status)
        print("read.turns:", len(reading.thread.turns or []))
        print("list.active:", len(listing_active.data))
        print("list.archived:", len(listing_archived.data))
        print("resumed:", resumed_info)
        print("forked:", forked_info)
        print("compact:", compact_info)


if __name__ == "__main__":
    asyncio.run(main())
