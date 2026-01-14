#!/usr/bin/env python3

import argparse
import asyncio
import datetime as dt
import json
import sys
from typing import Any

import websockets


HOST = "127.0.0.1"
DEFAULT_PORT = 8765
PATH = "/v1/responses"

CALL_ID = "shell-command-call"
FUNCTION_NAME = "shell_command"
FUNCTION_ARGS_JSON = json.dumps({"command": "echo websocket"}, separators=(",", ":"))

ASSISTANT_TEXT = "done"


def _utc_iso() -> str:
    return dt.datetime.now(tz=dt.timezone.utc).isoformat(timespec="milliseconds")


def _default_usage() -> dict[str, Any]:
    return {
        "input_tokens": 0,
        "input_tokens_details": None,
        "output_tokens": 0,
        "output_tokens_details": None,
        "total_tokens": 0,
    }


def _event_response_created(response_id: str) -> dict[str, Any]:
    return {"type": "response.created", "response": {"id": response_id}}


def _event_response_done() -> dict[str, Any]:
    return {"type": "response.done", "response": {"usage": _default_usage()}}


def _event_response_completed(response_id: str) -> dict[str, Any]:
    return {"type": "response.completed", "response": {"id": response_id, "usage": _default_usage()}}


def _event_function_call(call_id: str, name: str, arguments_json: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {"type": "function_call", "call_id": call_id, "name": name, "arguments": arguments_json},
    }


def _event_assistant_message(message_id: str, text: str) -> dict[str, Any]:
    return {
        "type": "response.output_item.done",
        "item": {
            "type": "message",
            "role": "assistant",
            "id": message_id,
            "content": [{"type": "output_text", "text": text}],
        },
    }


def _dump_json(payload: Any) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))


def _print_request(prefix: str, payload: Any) -> None:
    pretty = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True)
    sys.stdout.write(f"{prefix} {_utc_iso()}\n{pretty}\n")
    sys.stdout.flush()

async def _handle_connection(
    websocket: Any,
    *,
    expected_path: str = PATH,
) -> None:
    # websockets v15 exposes the request path here.
    path = getattr(getattr(websocket, "request", None), "path", None)
    if path is None:
        # Older handler signatures could pass `path` separately; accept if unavailable.
        path = "(unknown)"

    sys.stdout.write(f"[conn] {_utc_iso()} connected path={path}\n")
    sys.stdout.flush()

    path_no_qs = path.split("?", 1)[0] if path != "(unknown)" else path
    if path_no_qs != "(unknown)" and path_no_qs != expected_path:
        sys.stdout.write(f"[conn] {_utc_iso()} rejecting unexpected path (expected {expected_path})\n")
        sys.stdout.flush()
        await websocket.close(code=1008, reason="unexpected websocket path")
        return

    async def recv_json(label: str) -> Any:
        msg = await websocket.recv()
        if isinstance(msg, bytes):
            payload = json.loads(msg.decode("utf-8"))
        else:
            payload = json.loads(msg)
        _print_request(f"[{label}] recv", payload)
        return payload

    async def send_event(ev: dict[str, Any]) -> None:
        sys.stdout.write(f"[conn] {_utc_iso()} send {_dump_json(ev)}\n")
        await websocket.send(_dump_json(ev))

    # Request 1: provoke a function call (mirrors `codex-rs/core/tests/suite/agent_websocket.rs`).
    await recv_json("req1")
    await send_event(_event_response_created("resp-1"))
    await send_event(_event_function_call(CALL_ID, FUNCTION_NAME, FUNCTION_ARGS_JSON))
    await send_event(_event_response_done())

    # Request 2: expect appended tool output; send final assistant message.
    await recv_json("req2")
    await send_event(_event_response_created("resp-2"))
    await send_event(_event_assistant_message("msg-1", ASSISTANT_TEXT))
    await send_event(_event_response_completed("resp-2"))

    sys.stdout.write(f"[conn] {_utc_iso()} closing\n")
    sys.stdout.flush()
    await websocket.close()


async def _serve(port: int) -> int:
    async def handler(ws: Any) -> None:
        try:
            await _handle_connection(ws, expected_path=PATH)
        except websockets.exceptions.ConnectionClosedOK:
            return

    try:
        server = await websockets.serve(handler, HOST, port)
    except OSError as err:
        sys.stderr.write(f"[server] failed to bind ws://{HOST}:{port}: {err}\n")
        return 2
    bound_port = server.sockets[0].getsockname()[1]
    ws_uri = f"ws://{HOST}:{bound_port}"

    sys.stdout.write("[server] mock Responses WebSocket server running\n")
    sys.stdout.write(f"""Add this to your config.toml:


[model_providers.localapi_ws]
base_url = "{ws_uri}/v1"
name = "localapi_ws"
wire_api = "responses_websocket"
env_key = "OPENAI_API_KEY_STAGING"

[profiles.localapi_ws]
model = "gpt-5.2"
model_provider = "localapi_ws"
model_reasoning_effort = "medium"


start codex with `codex --profile localapi_ws`
""")
    sys.stdout.flush()

    try:
        await asyncio.Future()
    finally:
        server.close()
        await server.wait_closed()
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(
        description=(
            "Mock a minimal Responses API WebSocket endpoint for the `test_codex` flow.\n"
            f"Binds to {HOST}:{DEFAULT_PORT} by default and logs incoming JSON requests to stdout."
        ),
        formatter_class=argparse.RawTextHelpFormatter,
    )
    parser.add_argument(
        "--port",
        type=int,
        default=DEFAULT_PORT,
        help=f"Bind port (default: {DEFAULT_PORT}; use 0 for random free port).",
    )
    args = parser.parse_args()

    try:
        return asyncio.run(_serve(args.port))
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
