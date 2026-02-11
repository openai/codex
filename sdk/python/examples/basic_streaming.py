import asyncio

from openai_codex_sdk import Codex, ItemCompletedEvent, TurnCompletedEvent


async def main() -> None:
    codex = Codex()
    thread = codex.start_thread()

    while True:
        prompt = input("> ").strip()
        if not prompt:
            continue

        streamed = await thread.run_streamed(prompt)
        async for event in streamed.events:
            if isinstance(event, ItemCompletedEvent):
                print("item:", event.item)
            elif isinstance(event, TurnCompletedEvent):
                print("usage:", event.usage)


if __name__ == "__main__":
    asyncio.run(main())
