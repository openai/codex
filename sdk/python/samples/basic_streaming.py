#!/usr/bin/env python3
from __future__ import annotations

from codex_sdk import Codex
from codex_sdk.types import ThreadEvent, ThreadItem

from helpers import codex_path_override


def handle_item_completed(item: ThreadItem) -> None:
    item_type = item.get("type")
    if item_type == "agent_message":
        print(f"Assistant: {item.get('text')}")
    elif item_type == "reasoning":
        print(f"Reasoning: {item.get('text')}")
    elif item_type == "command_execution":
        exit_code = item.get("exit_code")
        exit_text = f" Exit code {exit_code}." if exit_code is not None else ""
        print(f"Command {item.get('command')} {item.get('status')}.{exit_text}")
    elif item_type == "file_change":
        for change in item.get("changes", []):
            print(f"File {change.get('kind')} {change.get('path')}")


def handle_item_updated(item: ThreadItem) -> None:
    if item.get("type") == "todo_list":
        print("Todo:")
        for todo in item.get("items", []):
            mark = "x" if todo.get("completed") else " "
            print(f"\t {mark} {todo.get('text')}")


def handle_event(event: ThreadEvent) -> None:
    event_type = event.get("type")
    if event_type == "item.completed":
        handle_item_completed(event.get("item", {}))
    elif event_type in {"item.updated", "item.started"}:
        handle_item_updated(event.get("item", {}))
    elif event_type == "turn.completed":
        usage = event.get("usage", {})
        print(
            "Used {input} input tokens, {cached} cached input tokens, {output} output tokens.".format(
                input=usage.get("input_tokens"),
                cached=usage.get("cached_input_tokens"),
                output=usage.get("output_tokens"),
            )
        )
    elif event_type == "turn.failed":
        error = event.get("error", {})
        print(f"Turn failed: {error.get('message')}")


def main() -> None:
    codex = Codex(codex_path_override=codex_path_override())
    thread = codex.start_thread()

    try:
        while True:
            user_input = input("> ").strip()
            if not user_input:
                continue
            streamed = thread.run_streamed(user_input)
            for event in streamed.events:
                handle_event(event)
    except KeyboardInterrupt:
        return


if __name__ == "__main__":
    main()
