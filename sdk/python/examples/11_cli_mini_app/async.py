import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio

from openai_codex import (
    AsyncCodex,
    TextInput,
)
from openai_codex.types import (
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification,
)


def _format_usage(usage: object) -> str:
    last = usage.last
    total = usage.total
    return (
        "usage>\n"
        f"  last: input={last.input_tokens} output={last.output_tokens} reasoning={last.reasoning_output_tokens} total={last.total_tokens} cached={last.cached_input_tokens}\n"
        f"  total: input={total.input_tokens} output={total.output_tokens} reasoning={total.reasoning_output_tokens} total={total.total_tokens} cached={total.cached_input_tokens}"
    )


async def main() -> None:
    print("Codex async mini CLI. Type /exit to quit.")

    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(
            model="gpt-5.4", config={"model_reasoning_effort": "high"}
        )
        print("Thread:", thread.id)

        while True:
            try:
                user_input = (await asyncio.to_thread(input, "you> ")).strip()
            except EOFError:
                break

            if not user_input:
                continue
            if user_input in {"/exit", "/quit"}:
                break

            turn = await thread.turn(TextInput(user_input))
            usage = None
            status = None
            error = None

            print("assistant> ", end="", flush=True)
            async for event in turn.stream():
                payload = event.payload
                if event.method == "item/agentMessage/delta":
                    delta = payload.delta
                    if delta:
                        print(delta, end="", flush=True)
                    continue
                if isinstance(payload, ThreadTokenUsageUpdatedNotification):
                    usage = payload.token_usage
                    continue
                if isinstance(payload, TurnCompletedNotification):
                    status = payload.turn.status
                    error = payload.turn.error

            print()
            if status is None:
                raise RuntimeError("stream ended without turn/completed")
            if usage is None:
                raise RuntimeError("stream ended without token usage")

            status_text = status.value
            print(f"assistant.status> {status_text}")
            if status_text == "failed":
                print("assistant.error>", error)

            print(_format_usage(usage))


if __name__ == "__main__":
    asyncio.run(main())
