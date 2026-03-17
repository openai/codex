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
)

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        turn = await thread.turn(TextInput("Explain SIMD in 3 short bullets."))

        event_count = 0
        saw_started = False
        saw_delta = False
        completed_status = "unknown"

        async for event in turn.stream():
            event_count += 1
            if event.method == "turn/started":
                saw_started = True
                print("stream.started")
                continue
            if event.method == "item/agentMessage/delta":
                delta = getattr(event.payload, "delta", "")
                if delta:
                    if not saw_delta:
                        print("assistant> ", end="", flush=True)
                    print(delta, end="", flush=True)
                    saw_delta = True
                continue
            if event.method == "turn/completed":
                completed_status = getattr(event.payload.turn.status, "value", str(event.payload.turn.status))

        if saw_delta:
            print()
        else:
            persisted = await thread.read(include_turns=True)
            persisted_turn = find_turn_by_id(persisted.thread.turns, turn.id)
            final_text = assistant_text_from_turn(persisted_turn).strip() or "[no assistant text]"
            print("assistant>", final_text)

        print("stream.started.seen:", saw_started)
        print("stream.completed:", completed_status)
        print("events.count:", event_count)


if __name__ == "__main__":
    asyncio.run(main())
