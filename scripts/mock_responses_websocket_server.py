#!/usr/bin/env python3

import argparse
import asyncio
import datetime as dt
import json
import sys
from dataclasses import dataclass
from typing import Any

import websockets


HOST = "127.0.0.1"
DEFAULT_PORT = 8765
PATH = "/v1/responses"

CALL_ID = "shell-command-call"
FUNCTION_NAME = "shell_command"
FUNCTION_ARGS_JSON = json.dumps({"command": "echo websocket"}, separators=(",", ":"))

ASSISTANT_TEXT = "done"
SCENARIO_NORMAL = "normal"
SCENARIO_USAGE_LIMIT = "usage-limit"


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


def _event_usage_limit_error() -> dict[str, Any]:
    return {
        "type": "error",
        "status": 429,
        "error": {
            "type": "usage_limit_reached",
            "message": "The usage limit has been reached",
            "plan_type": "pro",
            "resets_at": 1704067242,
            "resets_in_seconds": 1234,
        },
        "headers": {
            "x-codex-primary-used-percent": "100.0",
            "x-codex-secondary-used-percent": "87.5",
            "x-codex-primary-over-secondary-limit-percent": "95.0",
            "x-codex-primary-window-minutes": "15",
            "x-codex-secondary-window-minutes": "60",
        },
    }


def _event_approaching_rate_limits() -> dict[str, Any]:
    return {
        "type": "codex.rate_limits",
        "plan_type": "plus",
        "rate_limits": {
            "allowed": True,
            "limit_reached": False,
            "primary": {
                "used_percent": 95,
                "window_minutes": 15,
                "reset_at": 1704067242,
            },
            "secondary": {
                "used_percent": 87.5,
                "window_minutes": 60,
                "reset_at": 1704067242,
            },
        },
        "code_review_rate_limits": None,
        "credits": None,
        "promo": None,
    }


def _dump_json(payload: Any) -> str:
    return json.dumps(payload, ensure_ascii=False, separators=(",", ":"))


def _print_request(prefix: str, payload: Any) -> None:
    pretty = json.dumps(payload, ensure_ascii=False, indent=2, sort_keys=True)
    sys.stdout.write(f"{prefix} {_utc_iso()}\n{pretty}\n")
    sys.stdout.flush()


@dataclass
class ScenarioState:
    scenario: str
    usage_limit_delay_seconds: float
    usage_limit_approach_first: bool
    approaching_rate_limits_emitted: bool = False
    usage_limit_emitted: bool = False
    success_count: int = 0


async def _handle_connection(
    websocket: Any,
    *,
    scenario_state: ScenarioState,
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

    if scenario_state.scenario == SCENARIO_NORMAL:
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
    else:
        request_count = 0
        while True:
            request_count += 1
            payload = await recv_json(f"req{request_count}")
            if payload.get("generate") is False:
                await send_event(_event_response_created(f"resp-warmup-{request_count}"))
                await send_event(_event_response_completed(f"resp-warmup-{request_count}"))
                continue

            if (
                scenario_state.usage_limit_approach_first
                and not scenario_state.approaching_rate_limits_emitted
            ):
                scenario_state.approaching_rate_limits_emitted = True
                await send_event(_event_approaching_rate_limits())
                await send_event(_event_response_created("resp-approaching-limits"))
                await send_event(_event_assistant_message("msg-approaching-limits", ASSISTANT_TEXT))
                await send_event(_event_response_completed("resp-approaching-limits"))
                continue

            if not scenario_state.usage_limit_emitted:
                scenario_state.usage_limit_emitted = True
                sys.stdout.write(
                    f"[conn] {_utc_iso()} waiting {scenario_state.usage_limit_delay_seconds:.1f}s before usage limit\n"
                )
                sys.stdout.flush()
                await asyncio.sleep(scenario_state.usage_limit_delay_seconds)
                await send_event(_event_usage_limit_error())
                continue

            scenario_state.success_count += 1
            response_id = f"resp-after-limit-{scenario_state.success_count}"
            message_id = f"msg-after-limit-{scenario_state.success_count}"
            await send_event(_event_response_created(response_id))
            await send_event(_event_assistant_message(message_id, ASSISTANT_TEXT))
            await send_event(_event_response_completed(response_id))

    sys.stdout.write(f"[conn] {_utc_iso()} closing\n")
    sys.stdout.flush()
    await websocket.close()


async def _serve(
    port: int,
    scenario: str,
    usage_limit_delay_seconds: float,
    usage_limit_approach_first: bool,
) -> int:
    scenario_state = ScenarioState(
        scenario=scenario,
        usage_limit_delay_seconds=usage_limit_delay_seconds,
        usage_limit_approach_first=usage_limit_approach_first,
    )

    async def handler(ws: Any) -> None:
        try:
            await _handle_connection(ws, scenario_state=scenario_state, expected_path=PATH)
        except websockets.exceptions.ConnectionClosed:
            return

    try:
        server = await websockets.serve(handler, HOST, port)
    except OSError as err:
        sys.stderr.write(f"[server] failed to bind ws://{HOST}:{port}: {err}\n")
        return 2
    bound_port = server.sockets[0].getsockname()[1]
    ws_uri = f"ws://{HOST}:{bound_port}"

    sys.stdout.write("[server] mock Responses WebSocket server running\n")
    if scenario == SCENARIO_USAGE_LIMIT:
        if usage_limit_approach_first:
            sys.stdout.write(
                f"[server] scenario={SCENARIO_USAGE_LIMIT} first real turn approaches limits; next real turn errors after {usage_limit_delay_seconds:.1f}s\n"
            )
        else:
            sys.stdout.write(
                f"[server] scenario={SCENARIO_USAGE_LIMIT} first real turn errors after {usage_limit_delay_seconds:.1f}s\n"
            )
    sys.stdout.write(f"""Add this to your config.toml:


[model_providers.localapi_ws]
base_url = "{ws_uri}/v1"
name = "localapi_ws"
wire_api = "responses"
env_key = "OPENAI_API_KEY_STAGING"
supports_websockets = true

[profiles.localapi_ws]
model = "gpt-5.2"
model_provider = "localapi_ws"
model_reasoning_effort = "medium"


start codex with `codex --profile localapi_ws`
""")
    if scenario == SCENARIO_USAGE_LIMIT:
        if usage_limit_approach_first:
            sys.stdout.write(
                """To exercise the approaching-limit then paused-queue flow:
1. Submit one prompt and dismiss the approaching-rate-limits prompt.
2. Submit a second prompt.
3. Before the delayed usage-limit error arrives, press Enter for a steer and Tab for a queued follow-up.
4. After the error, confirm both queued sections stay paused until you resume them.
"""
            )
        else:
            sys.stdout.write(
                """To exercise the paused queue state:
1. Submit one prompt.
2. Before the delayed usage-limit error arrives, type follow-ups and press Tab to queue them.
3. After the error, confirm queued sends stay paused until you press Enter and choose `Resume queued sends`.
"""
            )
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
    parser.add_argument(
        "--scenario",
        choices=[SCENARIO_NORMAL, SCENARIO_USAGE_LIMIT],
        default=SCENARIO_NORMAL,
        help=(
            "Behavior to emulate (default: normal).\n"
            "Use `usage-limit` to make the first real turn hit a usage limit."
        ),
    )
    parser.add_argument(
        "--usage-limit-delay-seconds",
        type=float,
        default=15.0,
        help=(
            "Delay before sending the usage-limit error in the `usage-limit` scenario "
            "(default: 15.0)."
        ),
    )
    parser.add_argument(
        "--usage-limit-approach-first",
        action="store_true",
        help=(
            "In the `usage-limit` scenario, complete one successful near-limit turn before "
            "the delayed hard-limit turn."
        ),
    )
    args = parser.parse_args()

    try:
        return asyncio.run(
            _serve(
                args.port,
                args.scenario,
                args.usage_limit_delay_seconds,
                args.usage_limit_approach_first,
            )
        )
    except KeyboardInterrupt:
        return 0


if __name__ == "__main__":
    raise SystemExit(main())
