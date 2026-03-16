import sys
from pathlib import Path

_EXAMPLES_ROOT = Path(__file__).resolve().parents[1]
if str(_EXAMPLES_ROOT) not in sys.path:
    sys.path.insert(0, str(_EXAMPLES_ROOT))

from _bootstrap import ensure_local_sdk_src, runtime_config

ensure_local_sdk_src()

import asyncio

from codex_app_server import (
    AsyncCodex,
    TextInput,
    ThreadTokenUsageUpdatedNotification,
    TurnCompletedNotification,
)


def _status_value(status: object | None) -> str:
    return str(getattr(status, "value", status))


def _format_usage(usage: object | None) -> str:
    if usage is None:
        return "usage> (none)"

    last = getattr(usage, "last", None)
    total = getattr(usage, "total", None)
    if last is None or total is None:
        return f"usage> {usage}"

    return (
        "usage>\n"
        f"  last: input={last.inputTokens} output={last.outputTokens} reasoning={last.reasoningOutputTokens} total={last.totalTokens} cached={last.cachedInputTokens}\n"
        f"  total: input={total.inputTokens} output={total.outputTokens} reasoning={total.reasoningOutputTokens} total={total.totalTokens} cached={total.cachedInputTokens}"
    )


async def main() -> None:
    print("Codex async mini CLI. Type /exit to quit.")

    async with AsyncCodex(config=runtime_config()) as codex:
        thread = await codex.thread_start(model="gpt-5.4", config={"model_reasoning_effort": "high"})
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
            printed_delta = False

            print("assistant> ", end="", flush=True)
            async for event in turn.stream():
                payload = event.payload
                if event.method == "item/agentMessage/delta":
                    delta = getattr(payload, "delta", "")
                    if delta:
                        print(delta, end="", flush=True)
                        printed_delta = True
                    continue
                if isinstance(payload, ThreadTokenUsageUpdatedNotification):
                    usage = payload.token_usage
                    continue
                if isinstance(payload, TurnCompletedNotification):
                    status = payload.turn.status
                    error = payload.turn.error

            if printed_delta:
                print()
            else:
                print("[no text]")

            status_text = _status_value(status)
            print(f"assistant.status> {status_text}")
            if status_text == "failed":
                print("assistant.error>", error)

            print(_format_usage(usage))


if __name__ == "__main__":
    asyncio.run(main())
