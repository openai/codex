from __future__ import annotations

from codex import Codex, ItemCompletedEvent, TurnCompletedEvent


def main() -> None:
    client = Codex()
    thread = client.start_thread()

    stream = thread.run_streamed("Summarize repository health")
    for event in stream:
        match event:
            case ItemCompletedEvent(item=item):
                print(f"item[{item.type}]: {item}")
            case TurnCompletedEvent(usage=usage):
                print(
                    "usage: input=%s cached=%s output=%s"
                    % (usage.input_tokens, usage.cached_input_tokens, usage.output_tokens)
                )
            case _:
                print(event)


if __name__ == "__main__":
    main()
