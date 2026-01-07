#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import sys
import time
import uuid


def main() -> int:
    args = sys.argv[1:]
    stdin_text = sys.stdin.read()

    log_path = os.environ.get("CODEX_FAKE_LOG")
    if log_path:
        with open(log_path, "a", encoding="utf-8") as handle:
            handle.write(
                json.dumps(
                    {
                        "args": args,
                        "env": dict(os.environ),
                        "stdin": stdin_text,
                    }
                )
                + "\n"
            )

    thread_id = _extract_thread_id(args) or os.environ.get("CODEX_FAKE_THREAD_ID") or _random_id()
    response_text = os.environ.get("CODEX_FAKE_RESPONSE", "Hi!")

    mode = os.environ.get("CODEX_FAKE_MODE", "basic")
    if mode == "infinite":
        _emit_event({"type": "thread.started", "thread_id": thread_id})
        _emit_event({"type": "turn.started"})
        counter = 0
        while True:
            counter += 1
            _emit_event(
                {
                    "type": "item.completed",
                    "item": {
                        "id": f"item_{counter}",
                        "type": "agent_message",
                        "text": f"{response_text} {counter}",
                    },
                }
            )
            time.sleep(0.01)
    else:
        _emit_event({"type": "thread.started", "thread_id": thread_id})
        _emit_event({"type": "turn.started"})
        _emit_event(
            {
                "type": "item.completed",
                "item": {"id": "item_0", "type": "agent_message", "text": response_text},
            }
        )
        _emit_event(
            {
                "type": "turn.completed",
                "usage": {"input_tokens": 42, "cached_input_tokens": 12, "output_tokens": 5},
            }
        )
    return 0


def _emit_event(event: dict) -> None:
    sys.stdout.write(json.dumps(event) + "\n")
    sys.stdout.flush()


def _extract_thread_id(args: list[str]) -> str | None:
    if "resume" in args:
        idx = args.index("resume")
        if idx + 1 < len(args):
            return args[idx + 1]
    return None


def _random_id() -> str:
    return f"thread_{uuid.uuid4().hex[:8]}"


if __name__ == "__main__":
    raise SystemExit(main())
