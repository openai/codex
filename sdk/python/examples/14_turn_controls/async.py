import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import (
    assistant_text_from_turn,
    ensure_local_sdk_src,
    runtime_config,
)

ensure_local_sdk_src()

import asyncio

from codex_app_server import AsyncCodex, TextInput


async def main() -> None:
    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
        steer_turn = await thread.turn(TextInput("Count from 1 to 40 with commas, then one summary sentence."))
        steer_result = "sent"
        try:
            _ = await steer_turn.steer(TextInput("Keep it brief and stop after 10 numbers."))
        except Exception as exc:
            steer_result = f"skipped {type(exc).__name__}"

        steer_event_count = 0
        steer_completed_status = "unknown"
        steer_completed_turn = None
        async for event in steer_turn.stream():
            steer_event_count += 1
            if event.method == "turn/completed":
                steer_completed_turn = event.payload.turn
                steer_completed_status = getattr(event.payload.turn.status, "value", str(event.payload.turn.status))

        steer_preview = assistant_text_from_turn(steer_completed_turn).strip() or "[no assistant text]"

        interrupt_turn = await thread.turn(TextInput("Count from 1 to 200 with commas, then one summary sentence."))
        interrupt_result = "sent"
        try:
            _ = await interrupt_turn.interrupt()
        except Exception as exc:
            interrupt_result = f"skipped {type(exc).__name__}"

        interrupt_event_count = 0
        interrupt_completed_status = "unknown"
        interrupt_completed_turn = None
        async for event in interrupt_turn.stream():
            interrupt_event_count += 1
            if event.method == "turn/completed":
                interrupt_completed_turn = event.payload.turn
                interrupt_completed_status = getattr(event.payload.turn.status, "value", str(event.payload.turn.status))

        interrupt_preview = assistant_text_from_turn(interrupt_completed_turn).strip() or "[no assistant text]"

        print("steer.result:", steer_result)
        print("steer.final.status:", steer_completed_status)
        print("steer.events.count:", steer_event_count)
        print("steer.assistant.preview:", steer_preview)
        print("interrupt.result:", interrupt_result)
        print("interrupt.final.status:", interrupt_completed_status)
        print("interrupt.events.count:", interrupt_event_count)
        print("interrupt.assistant.preview:", interrupt_preview)


if __name__ == "__main__":
    asyncio.run(main())
