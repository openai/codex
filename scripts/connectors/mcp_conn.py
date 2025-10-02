import json
import subprocess
from typing import Any


def call_tool_stdio(command: str, args: list[str], tool: str, payload: dict, timeout_sec: int = 1) -> dict[str, Any]:
    # Design-only stub: uses the mcp-client binary in codex-rs or a simple stdio client.
    # For now, this is a placeholder returning an augment-like response.
    return {
        "decision": "augment",
        "message": "(stub) hydrated context",
        "context_items": [{"_key": "lessons/1", "title": "stub", "scope": "default", "why": "stub", "scores": {"bm25": 0.0}}],
    }

